use std::sync::Arc;

use selfhosted_template::app_settings::AppSettings;
use selfhosted_template::cron::worker::{JobInvocation, JobWorker};
use selfhosted_template::cron::{JobContext, JobHandler};
use selfhosted_template::i18n::Lang;
use selfhosted_template::notification;

use super::*;

// 定时任务失败触发通知：模板自带的业务无关触发点（`cron::worker` 收尾）。

/// 任务执行失败 → 飞书收到一条含任务名与失败原因的消息。
#[tokio::test]
async fn failed_job_sends_notification() {
    let _guard = SEND_LOCK.lock().await;
    let (base, mock) = spawn_feishu_mock().await;

    temp_env::async_with_vars([("FEISHU_BASE_URL", Some(base.as_str()))], async {
        let (db, _scheduler, _log_tx) = common::setup_db_and_scheduler().await;
        // 直连模块保存配置：HTTP 侧读写已由 config.rs 覆盖，此处只关心发送链路。
        notification::save_channel(
            &db,
            Lang::Zh,
            "cli_mock".to_string(),
            Some("cli-secret-abcd1234".to_string()),
            notification::RECEIVER_TYPE_OPEN_ID.to_string(),
            "ou_tester".to_string(),
            true,
        )
        .await
        .unwrap();

        run_failing_job(&db, "failing_job_test", "intentional failure").await;

        wait_for_messages(&mock, 1).await;
        let messages = mock.message_bodies();
        assert_eq!(messages.len(), 1, "任务失败应发出恰好一条通知");
        assert_eq!(messages[0]["receive_id"], "ou_tester");

        let content = messages[0]["content"]
            .as_str()
            .expect("content 必须是字符串而不是对象");
        let text = serde_json::from_str::<Value>(content).unwrap()["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("定时任务执行失败"), "标题不符：{text}");
        assert!(text.contains("任务：failing_job_test"), "缺任务名：{text}");
        assert!(text.contains("intentional failure"), "缺失败原因：{text}");
    })
    .await;
}

/// 未配置渠道时失败任务照常结束，不产生任何请求（静默跳过，不是错误）。
#[tokio::test]
async fn failed_job_without_config_sends_nothing() {
    let _guard = SEND_LOCK.lock().await;
    let (base, mock) = spawn_feishu_mock().await;

    temp_env::async_with_vars([("FEISHU_BASE_URL", Some(base.as_str()))], async {
        let (db, _scheduler, _log_tx) = common::setup_db_and_scheduler().await;

        run_failing_job(&db, "unconfigured_job_test", "boom").await;

        // 反向等待：给足窗口仍不应有任何投递。
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            mock.message_bodies().is_empty(),
            "未配置渠道不应发送：{:?}",
            mock.message_bodies()
        );
    })
    .await;
}

/// 渠道已停用（enable=false）时同样静默跳过。
#[tokio::test]
async fn disabled_channel_sends_nothing() {
    let _guard = SEND_LOCK.lock().await;
    let (base, mock) = spawn_feishu_mock().await;

    temp_env::async_with_vars([("FEISHU_BASE_URL", Some(base.as_str()))], async {
        let (db, _scheduler, _log_tx) = common::setup_db_and_scheduler().await;
        notification::save_channel(
            &db,
            Lang::Zh,
            "cli_mock".to_string(),
            Some("cli-secret-abcd1234".to_string()),
            notification::RECEIVER_TYPE_OPEN_ID.to_string(),
            "ou_tester".to_string(),
            false,
        )
        .await
        .unwrap();

        run_failing_job(&db, "disabled_channel_job_test", "boom").await;

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            mock.message_bodies().is_empty(),
            "停用渠道不应发送：{:?}",
            mock.message_bodies()
        );
    })
    .await;
}

/// 把一个必然失败的 handler 投进 worker，并等 run 收尾落库
/// （通知在 run 收尾之后发出，等库里的终态即可确定顺序）。
async fn run_failing_job(db: &DatabaseConnection, name: &str, error: &str) {
    let (log_tx, _) = tokio::sync::broadcast::channel(64);
    let worker = JobWorker::new_with_settings(db.clone(), 2, 100, log_tx, AppSettings::default());
    let handle = worker.start();

    let message = error.to_string();
    let handler: JobHandler = Arc::new(move |_ctx: JobContext| {
        let message = message.clone();
        Box::pin(async move { Err(message.into()) })
    });
    handle
        .tx
        .send(JobInvocation {
            name: name.to_string(),
            expression: "@hourly".to_string(),
            handler,
            scheduled_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    wait_for_failed_run(db, name).await;
    handle.shutdown(std::time::Duration::from_secs(5)).await;
}

/// 有界等待该任务的最新 run 落成 failed 终态。
async fn wait_for_failed_run(db: &DatabaseConnection, name: &str) {
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
    use selfhosted_template::entity::cron_job_run::{Column, Entity};

    for _ in 0..250 {
        let latest = Entity::find()
            .filter(Column::JobName.eq(name))
            .order_by_desc(Column::StartedAt)
            .one(db)
            .await
            .unwrap();
        if latest.is_some_and(|run| run.status == "failed") {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("run for '{name}' 未在预期时间内进入 failed 终态");
}
