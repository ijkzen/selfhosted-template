//! Shared bootstrap helpers for the HTTP integration tests.
//! 共享辅助模块被多个 test 二进制引用，各自只用到一部分，故允许未使用项。
#![allow(dead_code)]

use axum::extract::Request;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::Response;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

use selfhosted_template::app_settings::AppSettings;
use selfhosted_template::auth::{hash_password, hash_token};
use selfhosted_template::cron::log_capture::JobLogEvent;
use selfhosted_template::cron::scheduler::SchedulerRuntime;
use selfhosted_template::cron::worker::JobWorker;
use selfhosted_template::db;
use selfhosted_template::entity::{session, user};
use selfhosted_template::routes;
use selfhosted_template::state::AppState;

/// 集成测试默认用户（Admin / Password）与固定会话令牌。
pub const TEST_USERNAME: &str = "Admin";
pub const TEST_PASSWORD: &str = "Password";
pub const TEST_SESSION_TOKEN: &str = "itest-session-token-0123456789abcdef";
const TEST_COOKIE: &str = "session=itest-session-token-0123456789abcdef";

/// Creates an in-memory database, starts a job worker, and builds the
/// scheduler on top of it. The scheduler is returned unstarted so each test
/// can register handlers and seed data before calling `start()`.
pub async fn setup_db_and_scheduler() -> (
    DatabaseConnection,
    SchedulerRuntime,
    tokio::sync::broadcast::Sender<std::sync::Arc<JobLogEvent>>,
) {
    let db = db::connect("sqlite::memory:").await.unwrap();

    let (log_tx, _) = tokio::sync::broadcast::channel::<std::sync::Arc<JobLogEvent>>(64);
    let worker = JobWorker::new(db.clone(), 2, 100, log_tx.clone());
    let handle = worker.start();

    let scheduler = SchedulerRuntime::new(handle.tx.clone()).await.unwrap();
    (db, scheduler, log_tx)
}

/// Builds the Axum app with the given database, scheduler, log channel and
/// settings as state.
pub fn build_app(
    db: DatabaseConnection,
    scheduler: SchedulerRuntime,
    log_tx: tokio::sync::broadcast::Sender<std::sync::Arc<JobLogEvent>>,
    settings: AppSettings,
) -> axum::Router {
    let state = AppState {
        db,
        scheduler,
        log_tx,
        settings,
        feishu_registration: Default::default(),
    };
    routes::create_app(&state)
}

/// 与 [`build_app`] 相同，但设置从 DB 加载（等价于生产 `AppSettings::load_from_db`，
/// 会幂等种入 language/timezone 种子行）。
pub async fn build_app_loaded(
    db: DatabaseConnection,
    scheduler: SchedulerRuntime,
    log_tx: tokio::sync::broadcast::Sender<std::sync::Arc<JobLogEvent>>,
) -> axum::Router {
    let settings = AppSettings::load_from_db(&db).await.unwrap();
    build_app(db, scheduler, log_tx, settings)
}

/// 在测试库中种入默认用户与固定会话。
async fn seed_default_auth(db: &DatabaseConnection) {
    let now = chrono::Utc::now();
    let user_id = user::ActiveModel {
        username: Set(TEST_USERNAME.to_string()),
        password_hash: Set(hash_password(TEST_PASSWORD).unwrap()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap()
    .id;

    session::ActiveModel {
        id: Set(hash_token(TEST_SESSION_TOKEN)),
        user_id: Set(user_id),
        created_at: Set(now),
        expires_at: Set(now + chrono::Duration::days(365)),
    }
    .insert(db)
    .await
    .unwrap();
}

/// 测试专用请求头注入：/api/* 注入会话 Cookie（/api/auth/* 与 healthz 除外，
/// 中间件本就不要求认证）。
async fn inject_test_auth(mut req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") && !path.starts_with("/api/auth/") && path != "/api/healthz" {
        req.headers_mut().insert(
            axum::http::header::COOKIE,
            HeaderValue::from_static(TEST_COOKIE),
        );
    }
    next.run(req).await
}

/// 与 build_app_loaded 相同，但种入默认凭证并给请求自动注入认证头。
pub async fn build_authed_app(
    db: DatabaseConnection,
    scheduler: SchedulerRuntime,
    log_tx: tokio::sync::broadcast::Sender<std::sync::Arc<JobLogEvent>>,
) -> axum::Router {
    seed_default_auth(&db).await;
    build_app_loaded(db, scheduler, log_tx)
        .await
        .layer(axum::middleware::from_fn(inject_test_auth))
}
