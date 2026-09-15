//! 内置定时任务的种子行（与 `src/lib.rs::init` 中注册的 handler 一一对应）。
//!
//! 模板没有创建任务的 API，内置周期任务通过启动时幂等 upsert 种子行进入调度器。

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::entity::cron_job;

/// 示例任务名：与 `src/lib.rs::init` 注册的 "example" handler 对应，
/// 演示模板的定时任务注册与执行。
pub const EXAMPLE_JOB: &str = "example";

/// 内置任务的默认标题（按语言）。语言切换同步未自定义任务时复用。
pub fn default_title(name: &str, lang: crate::i18n::Lang) -> String {
    match name {
        EXAMPLE_JOB => lang.tr("示例任务", "Example Job").to_string(),
        _ => lang.tr("定时任务", "Cron Job").to_string(),
    }
}

/// 内置任务的默认描述（按语言）。
pub fn default_description(name: &str, lang: crate::i18n::Lang) -> String {
    match name {
        EXAMPLE_JOB => lang
            .tr(
                "模板内置的示例定时任务，演示 cron 引擎的注册与执行",
                "Built-in example job demonstrating cron engine registration and execution",
            )
            .to_string(),
        _ => lang
            .tr("系统内置定时任务", "Built-in scheduled job")
            .to_string(),
    }
}

/// 确保 `example` 任务行存在（不存在则插入，幂等）。
pub async fn ensure_example_job(db: &DatabaseConnection) -> anyhow::Result<()> {
    ensure_job(db, EXAMPLE_JOB, "@every 1h").await
}

async fn ensure_job(db: &DatabaseConnection, name: &str, expression: &str) -> anyhow::Result<()> {
    let exists = cron_job::Entity::find()
        .filter(cron_job::Column::Name.eq(name))
        .one(db)
        .await?;
    if exists.is_some() {
        return Ok(());
    }
    let now = chrono::Utc::now();
    let lang = crate::i18n::Lang::default();
    // next_run_at 按表达式计算：写成 now 会让 @hourly 任务在首跑前展示
    // 「下次运行 = 过去的时刻」数小时。计算失败兜底 now（展示偏差不阻塞种子）。
    let next_run_at = super::parser::compute_next_run_tz(expression, None).unwrap_or(now);
    cron_job::ActiveModel {
        name: Set(name.to_string()),
        title: Set(default_title(name, lang)),
        description: Set(default_description(name, lang)),
        expression: Set(expression.to_string()),
        enabled: Set(true),
        group: Set("system".to_string()),
        last_run_at: Set(now),
        next_run_at: Set(next_run_at),
        created_at: Set(now),
        updated_at: Set(now),
        is_deleted: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await?;
    tracing::info!(name, expression, "已创建内置定时任务");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_is_idempotent_and_loadable() {
        let db = crate::db::connect("sqlite::memory:").await.unwrap();
        ensure_example_job(&db).await.unwrap();
        ensure_example_job(&db).await.unwrap();

        let rows = cron_job::Entity::find()
            .filter(cron_job::Column::Name.eq(EXAMPLE_JOB))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].expression, "@every 1h");
        assert!(rows[0].enabled);
        assert_eq!(rows[0].group, "system");
        assert!(!rows[0].is_deleted);
    }
}
