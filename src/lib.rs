pub mod app_settings;
pub mod auth;
pub mod config;
pub mod cron;
pub mod crypto;
pub mod db;
pub mod entity;
pub mod i18n;
pub mod logs_cleanup;
pub mod middleware;
pub mod notification;
pub mod response;
pub mod routes;
pub mod state;
pub mod static_assets;

use std::sync::Arc;

use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::{Config, RuntimeEnv};
use crate::cron::JobContext;
use crate::cron::log_capture::JobLogLayer;
use crate::cron::log_repository::{CronJobLogRepository, SeaOrmCronJobLogRepository};
use crate::cron::repository::SeaOrmCronJobRepository;
use crate::cron::scheduler::SchedulerRuntime;
use crate::cron::worker::JobWorker;
use crate::state::AppState;

const LOG_RETENTION_DAYS: u64 = 30;
const SHUTDOWN_TIMEOUT_SECS: u64 = 10;
/// HTTP 层收尾等待上限（秒）：信号到达后长连接仍不结束则放行，进入调度器与任务收尾。
const HTTP_DRAIN_TIMEOUT_SECS: u64 = 8;
/// 任务日志事件广播容量（条）：单次执行日志上限 2000 条、并发执行数默认
/// ≤10，理论最坏 ~20000 条/瞬时——本容量不追求吞下理论峰值（超出即
/// Lagged → worker 记截断并补溢出提示），按「worker 每 ~50 条攒批落库、
/// 事件产生速率远低于消费速率」的实际留量取 8192：约合 4MB（单条
/// ≤4096 字符的 JSON），驻留有界且覆盖正常并发突发。
const JOB_LOG_BROADCAST_CAPACITY: usize = 8192;

struct AppContext {
    #[allow(dead_code)]
    log_guard: tracing_appender::non_blocking::WorkerGuard,
    state: AppState,
    worker_handle: crate::cron::worker::WorkerHandle,
}

async fn setup_logging(
    env: &RuntimeEnv,
    log_tx: broadcast::Sender<Arc<crate::cron::log_capture::JobLogEvent>>,
) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard> {
    let timer = tracing_subscriber::fmt::time::LocalTime::rfc_3339();

    let log_dir = env.log_dir();

    tokio::fs::create_dir_all(log_dir).await?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "app");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // JobLogLayer 直接发送到 tokio broadcast（send 同步且不阻塞），供 worker
    // 与 SSE 订阅；无订阅者时 send 返回 Err（事件静默丢弃）。

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,sqlx::query=warn")),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_timer(timer.clone())
                .with_writer(std::io::stdout),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_timer(timer)
                .with_writer(non_blocking)
                .with_ansi(false),
        )
        // 任务日志捕获层挂在 EnvFilter 之后：捕获级别受 RUST_LOG 限制，
        // 默认只捕获 info 及以上级别的 handler 日志。
        .with(JobLogLayer::new(log_tx))
        .init();

    Ok(guard)
}

async fn init(config: Config) -> anyhow::Result<AppContext> {
    let (log_tx, _) = broadcast::channel(JOB_LOG_BROADCAST_CAPACITY);
    let log_guard = setup_logging(&config.env, log_tx.clone()).await?;

    tracing::info!("Starting selfhosted-template");

    let db = db::connect(&config.database_url).await?;

    let repo = SeaOrmCronJobRepository::new(db.clone());

    // 语言/时区设置缓存：从 setting 表加载（缺失时幂等补种子行），
    // 供 API 消息本地化与定时任务 cron 语义时区使用。
    let settings = crate::app_settings::AppSettings::load_from_db(&db).await?;
    crate::app_settings::AppSettings::set_process_global(settings.clone());

    // 进程重启会中断执行中的任务，把残留的 running 执行标记为 failed。
    let log_repo = SeaOrmCronJobLogRepository::new(db.clone());
    match log_repo.mark_interrupted_runs_failed().await {
        Ok(n) if n > 0 => tracing::warn!("Marked {n} interrupted run(s) as failed after restart"),
        _ => {}
    }
    // 清理历史遗留的无 run 归属孤儿日志（prune 从 run 表倒推删不到它们）。
    if let Err(e) = log_repo.delete_orphan_logs().await {
        tracing::warn!("Failed to delete orphan cron job logs: {e}");
    }

    let worker = JobWorker::new_with_settings(
        db.clone(),
        config.cron_job_max_concurrent,
        config.cron_job_queue_size,
        log_tx.clone(),
        settings.clone(),
    );
    let worker_handle = worker.start();

    let scheduler =
        SchedulerRuntime::new_with_settings(worker_handle.tx.clone(), settings.clone()).await?;

    // 模板示例 handler；新项目把自己的业务 handler 加在这里。
    scheduler
        .register_handler(
            "example",
            Arc::new(|_ctx: JobContext| {
                Box::pin(async move {
                    tracing::info!("示例任务开始执行");
                    for step in 1..=5 {
                        tracing::info!("示例任务执行中：第 {step} 步");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    tracing::info!("示例任务执行完成");
                    Ok(())
                })
            }),
        )
        .await;

    // 内置定时任务种子：示例任务（每小时），与上面的 handler 注册一一对应。
    crate::cron::seed::ensure_example_job(&db).await?;

    scheduler.load_from_db(&repo).await?;
    scheduler.start().await?;

    logs_cleanup::spawn_cleanup_task(config.env.log_dir().to_string(), LOG_RETENTION_DAYS);

    let state = AppState {
        db,
        scheduler,
        log_tx,
        settings,
        feishu_registration: Default::default(),
    };

    Ok(AppContext {
        log_guard,
        state,
        worker_handle,
    })
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    let ctx = init(config.clone()).await?;

    let app = routes::create_app(&ctx.state);

    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    tracing::info!("Listening on {}", config.bind_address);

    // 停止接收新连接并等既有连接收尾，但设上限（长连接不结束则超时进入收尾）。
    // 关闭信号经 oneshot 转交 serve 触发优雅关停；上限只从信号到达后计时。
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let serve = axum::serve(listener, app).with_graceful_shutdown(async move {
        let _ = shutdown_rx.await;
    });
    let shutdown = async move {
        shutdown_signal().await;
        let _ = shutdown_tx.send(());
    };
    serve_until_shutdown(
        serve,
        shutdown,
        std::time::Duration::from_secs(HTTP_DRAIN_TIMEOUT_SECS),
    )
    .await?;

    // Stop scheduling new runs first, then wait for in-flight jobs to finish
    // (bounded by a timeout) so jobs are not aborted mid-write.
    ctx.state.scheduler.stop().await?;
    ctx.worker_handle
        .shutdown(std::time::Duration::from_secs(SHUTDOWN_TIMEOUT_SECS))
        .await;
    tracing::info!("Shutdown complete");

    Ok(())
}

/// 跑 HTTP 服务直到收到关闭信号，随后只给「收尾等待」设上限。
///
/// 上限**必须**从信号到达后才开始计时：SSE 日志流等长连接不会自行结束，无上限会把
/// `scheduler.stop` 与 worker 收尾无限期钉死；而若把超时套在整个 serve 上，未收到
/// 信号时进程也会在启动满 N 秒后走完超时分支自行退出（容器反复重启、健康检查永远
/// 不通过）。
async fn serve_until_shutdown<S, G>(
    serve: S,
    shutdown: G,
    drain_timeout: std::time::Duration,
) -> std::io::Result<()>
where
    S: std::future::IntoFuture<Output = std::io::Result<()>>,
    G: std::future::Future<Output = ()>,
{
    let serve = serve.into_future();
    tokio::pin!(serve);
    tokio::select! {
        result = &mut serve => return result,
        _ = shutdown => {}
    }

    match tokio::time::timeout(drain_timeout, &mut serve).await {
        Ok(result) => result,
        Err(_) => {
            tracing::warn!(
                "HTTP 服务在 {}s 内未完成收尾（长连接未结束），继续执行调度器与任务收尾",
                drain_timeout.as_secs()
            );
            Ok(())
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Signal received, starting graceful shutdown");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::pending;
    use std::time::Duration;

    /// 回归：关闭信号到达前不得因收尾上限退出。
    /// 若把 timeout 套在整个 serve 上，进程会在启动满 N 秒后自行退出、容器反复重启。
    #[tokio::test(start_paused = true)]
    async fn test_serve_until_shutdown_survives_past_drain_timeout_without_signal() {
        let serve = pending::<std::io::Result<()>>();
        let task = tokio::spawn(serve_until_shutdown(
            serve,
            pending::<()>(),
            Duration::from_secs(HTTP_DRAIN_TIMEOUT_SECS),
        ));

        tokio::time::sleep(Duration::from_secs(3600)).await;
        assert!(!task.is_finished(), "信号到达前不得因收尾上限退出");

        task.abort();
    }

    /// 信号到达后收尾仍未结束（长连接钉住）：上限放行，不再无限期等待。
    #[tokio::test(start_paused = true)]
    async fn test_serve_until_shutdown_releases_after_signal() {
        let started = tokio::time::Instant::now();

        serve_until_shutdown(
            pending::<std::io::Result<()>>(),
            async {},
            Duration::from_secs(5),
        )
        .await
        .expect("超时放行应视为正常收尾");

        assert_eq!(started.elapsed(), Duration::from_secs(5));
    }

    /// 信号到达且收尾及时完成：返回 serve 的结果。
    #[tokio::test(start_paused = true)]
    async fn test_serve_until_shutdown_returns_serve_result() {
        let serve = async {
            tokio::time::sleep(Duration::from_secs(1)).await;
            Ok::<(), std::io::Error>(())
        };
        let started = tokio::time::Instant::now();

        serve_until_shutdown(serve, async {}, Duration::from_secs(5))
            .await
            .expect("收尾完成应原样返回");

        assert_eq!(started.elapsed(), Duration::from_secs(1));
    }

    /// serve 自身失败（未收到信号）：错误直接上抛，不等信号也不吃上限。
    #[tokio::test(start_paused = true)]
    async fn test_serve_until_shutdown_propagates_serve_error() {
        let serve = async { Err::<(), std::io::Error>(std::io::Error::other("bind failed")) };

        let err = serve_until_shutdown(serve, pending::<()>(), Duration::from_secs(5))
            .await
            .expect_err("serve 错误必须上抛");

        assert_eq!(err.to_string(), "bind failed");
    }
}
