use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr, Schema, Statement};
use std::path::Path;
use std::time::Duration;

const SLOW_QUERY_THRESHOLD_MS: u64 = 100;

/// Extracts the filesystem path from a SQLite URL for directory creation.
///
/// sqlx URL conventions: `sqlite::memory:` (no file), `sqlite://rel/path.db`
/// (relative), `sqlite:///abs/path.db` (absolute), `sqlite:plain.db` (relative).
/// Returns None for in-memory databases and non-path URLs.
fn sqlite_url_path(database_url: &str) -> Option<String> {
    let rest = database_url.strip_prefix("sqlite:")?;
    let rest = rest.split('?').next().unwrap_or(rest);
    if rest.is_empty() || rest == ":memory:" {
        return None;
    }
    // "///abs/path" → "/abs/path"; "//rel/path" → "rel/path"; "/x" or "x" → "x".
    if let Some(abs) = rest.strip_prefix("///") {
        return Some(format!("/{abs}"));
    }
    let rel = rest.strip_prefix("//").unwrap_or(rest);
    let rel = rel.strip_prefix('/').unwrap_or(rel);
    if rel.is_empty() {
        None
    } else {
        Some(rel.to_string())
    }
}

async fn ensure_sqlite_dir(database_url: &str) -> Result<(), std::io::Error> {
    if let Some(path) = sqlite_url_path(database_url)
        && let Some(parent) = Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            tracing::error!(
                "Failed to create database directory '{}': {}",
                parent.display(),
                e
            );
            e
        })?;
    }
    Ok(())
}

pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    ensure_sqlite_dir(database_url)
        .await
        .map_err(|e| DbErr::Custom(format!("Failed to create database directory: {e}")))?;

    let mut opt = ConnectOptions::new(database_url.to_owned());

    opt.max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(8))
        .acquire_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(60))
        .max_lifetime(Duration::from_secs(3600))
        .sqlx_logging(true)
        .sqlx_slow_statements_logging_settings(
            tracing::log::LevelFilter::Warn,
            Duration::from_millis(SLOW_QUERY_THRESHOLD_MS),
        );

    if database_url.starts_with("sqlite:") {
        use sea_orm::sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};

        opt.map_sqlx_sqlite_opts(|opts: SqliteConnectOptions| {
            // WAL 下事务内「先读后写」存在写升级竞态：DEFERRED 事务首语句是读，
            // 读快照期间若其他连接提交过，首次写升级会立即报 SQLITE_BUSY_SNAPSHOT，
            // busy_timeout 对此无效。需要多语句事务时把写放在首位，或把读移出事务
            // （见 `cron::log_repository::prune_old_runs` 的取数顺序）；回归测试必须
            // 用文件库——内存库没有 WAL 语义。
            opts.journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal)
                // SQLite 写操作是串行的，过长的 busy_timeout 会掩盖锁竞争。
                .busy_timeout(Duration::from_secs(5))
                .foreign_keys(true)
                // -64000 为 KiB 单位，约 62.5 MiB/连接页缓存，提升读性能。
                .pragma("cache_size", "-64000")
                // 临时表/排序全部走内存。
                .pragma("temp_store", "2")
                // 限制 WAL/回滚日志文件大小不超过 64 MB。
                .pragma("journal_size_limit", "67108864")
                // WAL 自动检查点阈值（页数），默认即 1000，显式声明便于维护。
                .pragma("wal_autocheckpoint", "1000")
                // 内存映射 I/O，读多场景可降低系统调用开销。
                .pragma("mmap_size", "268435456")
        });
    }

    let db = Database::connect(opt).await?;

    let changed = migrate(&db).await?;
    if changed {
        use sea_orm::ConnectionTrait;
        db.execute_unprepared("ANALYZE;").await?;
    }

    Ok(db)
}

pub(crate) async fn migrate(db: &DatabaseConnection) -> Result<bool, DbErr> {
    use crate::entity::{cron_job, cron_job_log, cron_job_run, note, session, setting, user};
    use sea_orm::ConnectionTrait;

    let backend = db.get_database_backend();

    let mut stmt = Schema::new(backend).create_table_from_entity(setting::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(user::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(session::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(cron_job::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(cron_job_run::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(cron_job_log::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    let mut stmt = Schema::new(backend).create_table_from_entity(note::Entity);
    stmt.if_not_exists();
    db.execute(&stmt).await?;

    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
    )
    .await?;

    let mut changed = false;

    // Migration 1: 会话过期时间查询/清理索引。
    changed |= ensure_migration(
        db,
        1,
        &["CREATE INDEX IF NOT EXISTS idx_session_expires_at ON session (expires_at)"],
    )
    .await?;

    // Migration 2: 定时任务的分组与软删字段（历史库兜底；新库建表已带这两列）。
    changed |= ensure_columns(
        db,
        2,
        &[
            (
                "cron_jobs",
                "group",
                "ALTER TABLE cron_jobs ADD COLUMN \"group\" TEXT NOT NULL DEFAULT 'other'",
            ),
            (
                "cron_jobs",
                "is_deleted",
                "ALTER TABLE cron_jobs ADD COLUMN \"is_deleted\" BOOLEAN NOT NULL DEFAULT 0",
            ),
        ],
    )
    .await?;

    // Migration 3 originally created a redundant non-unique index on `name`.
    // It is now a placeholder so existing databases skip it; migration 4 drops
    // the index because `name` already has a unique constraint.
    changed |= ensure_migration(db, 3, &["SELECT 1"]).await?;

    changed |= ensure_migration(db, 4, &["DROP INDEX IF EXISTS idx_cron_jobs_name"]).await?;

    // Migration 5: 定时任务执行日志（runs + logs）的查询索引。
    changed |= ensure_migration(
        db,
        5,
        &[
            "CREATE INDEX IF NOT EXISTS idx_cron_job_runs_job_name ON cron_job_runs (job_name)",
            "CREATE INDEX IF NOT EXISTS idx_cron_job_logs_run_id ON cron_job_logs (run_id)",
        ],
    )
    .await?;

    // Migration 6: cron_job_logs 覆盖索引 (run_id, seq) —— 单 run 日志查询按
    // run_id 过滤 + seq 排序，复合索引直接覆盖；左前缀同时替代原单列
    // idx_cron_job_logs_run_id，故删除后者减少写放大。
    changed |= ensure_migration(
        db,
        6,
        &[
            "CREATE INDEX IF NOT EXISTS idx_cron_job_logs_run_seq ON cron_job_logs (run_id, seq)",
            "DROP INDEX IF EXISTS idx_cron_job_logs_run_id",
        ],
    )
    .await?;

    tracing::info!("Database tables migrated");

    Ok(changed)
}

async fn column_exists(db: &DatabaseConnection, table: &str, column: &str) -> Result<bool, DbErr> {
    use sea_orm::ConnectionTrait;

    let rows = db
        .query_all_raw(Statement::from_string(
            db.get_database_backend(),
            format!("PRAGMA table_info({table})"),
        ))
        .await?;
    for row in rows {
        if let Ok(name) = row.try_get::<String>("", "name")
            && name == column
        {
            return Ok(true);
        }
    }
    Ok(false)
}

// Non-idempotent ALTER TABLE statements are acceptable here because the
// in-transaction version guard turns a concurrent second migrator into a
// fail-fast error (DDL rolls back with the transaction; the DB is never left
// half-migrated) rather than silently double-applying. It does NOT serialize
// concurrent first-start migrations -- deployments run a single container at a
// time. The schema_migrations table is created before any versioned migration
// runs.

/// 缺失则补列的迁移样板：对 `(table, column)` 逐列检查，缺列时收集其 ADD 语句，
/// 最后以单次 `ensure_migration(version, stmts)` 执行（版本守卫保证同一版本只跑
/// 一次）。无缺列时仍写入版本记录（`SELECT 1`），幂等且不吞后续迁移。
async fn ensure_columns(
    db: &DatabaseConnection,
    version: i32,
    columns: &[(&str, &str, &str)],
) -> Result<bool, DbErr> {
    let mut statements: Vec<&str> = Vec::new();
    for (table, column, add_ddl) in columns {
        if !column_exists(db, table, column).await? {
            statements.push(add_ddl);
        }
    }
    if statements.is_empty() {
        ensure_migration(db, version, &["SELECT 1"]).await
    } else {
        ensure_migration(db, version, &statements).await
    }
}

async fn ensure_migration(
    db: &DatabaseConnection,
    version: i32,
    statements: &[&str],
) -> Result<bool, DbErr> {
    use sea_orm::{ConnectionTrait, Statement, TransactionTrait};

    let txn = db.begin().await?;

    let count: i64 = txn
        .query_one_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT COUNT(*) AS c FROM schema_migrations WHERE version = ?",
            [version.into()],
        ))
        .await?
        .map(|row| row.try_get::<i64>("", "c").unwrap_or(0))
        .unwrap_or(0);

    if count == 0 {
        for stmt in statements {
            txn.execute_unprepared(stmt).await?;
        }
        txn.execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?, datetime('now'))",
            [version.into()],
        ))
        .await?;
        txn.commit().await?;
        Ok(true)
    } else {
        txn.commit().await?;
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Statement;

    #[test]
    fn test_sqlite_url_path_relative() {
        assert_eq!(
            sqlite_url_path("sqlite://db/app.db?mode=rwc"),
            Some("db/app.db".to_string())
        );
        assert_eq!(
            sqlite_url_path("sqlite:plain.db"),
            Some("plain.db".to_string())
        );
    }

    #[test]
    fn test_sqlite_url_path_absolute_stays_absolute() {
        // Regression: the absolute prod path must not be turned into a
        // relative path, otherwise the directory is created under the CWD.
        assert_eq!(
            sqlite_url_path("sqlite:///config/db/app.db?mode=rwc"),
            Some("/config/db/app.db".to_string())
        );
    }

    #[test]
    fn test_sqlite_url_path_memory_returns_none() {
        assert_eq!(sqlite_url_path("sqlite::memory:"), None);
        assert_eq!(sqlite_url_path("sqlite:"), None);
    }

    #[tokio::test]
    async fn test_ensure_sqlite_dir_creates_relative_parent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("sub/dir/app.db");
        let url = format!("sqlite://{}?mode=rwc", db_path.display());
        // The URL above is absolute on disk ("sqlite:///tmp/..."), so the
        // parent must be created at the absolute location.
        ensure_sqlite_dir(&url).await.unwrap();
        assert!(db_path.parent().unwrap().exists());
    }

    #[tokio::test]
    async fn migrate_creates_template_tables_idempotently() {
        use sea_orm::ConnectionTrait;
        let db = connect("sqlite::memory:").await.unwrap();

        for table in [
            "setting",
            "user",
            "session",
            "cron_jobs",
            "cron_job_runs",
            "cron_job_logs",
            "notes",
            "schema_migrations",
        ] {
            let row = db
                .query_one_raw(Statement::from_string(
                    db.get_database_backend(),
                    format!("SELECT COUNT(*) AS c FROM sqlite_master WHERE type='table' AND name='{table}'"),
                ))
                .await
                .unwrap()
                .unwrap();
            let count: i64 = row.try_get("", "c").unwrap();
            assert_eq!(count, 1, "table {table} should exist after migrate");
        }

        // 二次 migrate 幂等：不重复应用版本化迁移。
        let changed = migrate(&db).await.unwrap();
        assert!(!changed, "second migrate should be a no-op");
    }

    #[tokio::test]
    async fn ensure_columns_adds_all_missing_columns_in_one_version() {
        use sea_orm::ConnectionTrait;
        let db = connect("sqlite::memory:").await.unwrap();

        db.execute_unprepared("CREATE TABLE probe (id INTEGER PRIMARY KEY)")
            .await
            .unwrap();

        // 同一版本内两列都缺：必须一次补齐——若分两次调用同版本，第二次会被
        // 版本守卫吞掉，导致第二列永久缺失。
        assert!(
            ensure_columns(
                &db,
                900,
                &[
                    ("probe", "alpha", "ALTER TABLE probe ADD COLUMN alpha TEXT"),
                    ("probe", "beta", "ALTER TABLE probe ADD COLUMN beta INTEGER"),
                ],
            )
            .await
            .unwrap()
        );
        assert!(column_exists(&db, "probe", "alpha").await.unwrap());
        assert!(column_exists(&db, "probe", "beta").await.unwrap());

        // 重复调用：版本已记录，不再执行也不报错。
        assert!(
            !ensure_columns(
                &db,
                900,
                &[("probe", "alpha", "ALTER TABLE probe ADD COLUMN alpha TEXT")],
            )
            .await
            .unwrap()
        );
    }
}
