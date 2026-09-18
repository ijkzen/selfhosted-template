//! 定时任务失败通知：任务执行失败时异步发飞书消息。
//!
//! 触发点收敛在 worker 的单次执行收尾（`cron::worker::execute_with_logging`），
//! 未配置 / 已停用时静默跳过。新项目接入自己的业务事件时，照此模块加一个
//! `render_*` + `spawn_*` 即可。

use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use super::feishu;
use crate::app_settings::AppSettings;
use crate::i18n::Lang;

/// 渲染失败通知正文（纯文本多行）。时间按设置表时区展示，与 cron/日志口径一致。
pub fn render_failure(
    job_name: &str,
    error: &str,
    now: DateTime<Utc>,
    tz: Option<chrono_tz::Tz>,
    lang: Lang,
) -> String {
    // 两个分支的时区类型不同，各自格式化后再拼装。
    let stamp = match tz {
        Some(tz) => now
            .with_timezone(&tz)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => now
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    };
    // 标签分隔符也是文案的一部分：中文全角冒号，英文半角加空格。
    let sep = lang.tr("：", ": ");
    format!(
        "{}\n{}{sep}{}\n{}{sep}{}\n{}{sep}{}",
        lang.tr("【定时任务执行失败】", "[Cron job failed]"),
        lang.tr("任务", "Job"),
        job_name,
        lang.tr("原因", "Reason"),
        error,
        lang.tr("时间", "Time"),
        stamp,
    )
}

/// 异步发送入口：**同步函数**，内部 `tokio::spawn`。
///
/// 调用点在任务执行的收尾路径上，绝不能因飞书 HTTP 调用而阻塞，所以这里不返回
/// future 让调用方 await。语言/时区在 spawn 出的任务里读，调用方无需先 await。
pub fn spawn_failure(db: &DatabaseConnection, settings: &AppSettings, job_name: &str, error: &str) {
    let db = db.clone();
    let settings = settings.clone();
    let job_name = job_name.to_string();
    // 与日志同口径截断：handler 错误串可含上游响应片段，通知正文不该无界。
    let error = crate::cron::log_capture::trim_and_limit(error);

    tokio::spawn(async move {
        let lang = settings.lang().await;
        let tz = settings.timezone().await;
        if let Err(e) = notify_failure(&db, &job_name, &error, lang, tz).await {
            tracing::warn!("Failed to send cron failure notification for '{job_name}': {e}");
        }
    });
}

/// 通知主体（`await` 直调，便于确定性测试）。
///
/// 未配置 / 已停用一律静默返回（不构成错误）。发送结果无论成败都回写
/// `last_error` / `last_sent_at`。
pub async fn notify_failure(
    db: &DatabaseConnection,
    job_name: &str,
    error: &str,
    lang: Lang,
    tz: Option<chrono_tz::Tz>,
) -> Result<(), String> {
    let Some(config) = super::load_active_config(db)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(());
    };

    let text = render_failure(job_name, error, Utc::now(), tz, lang);
    let result = feishu::send(&config, &text).await;
    super::record_send_result(db, result.as_ref().err().map(String::as_str)).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 18, 3, 30, 0).unwrap()
    }

    /// 中文正文：标题、任务名、原因、时间四行，时间按给定时区渲染。
    #[test]
    fn renders_failure_message_in_chinese() {
        let text = render_failure(
            "example",
            "connection refused",
            at(),
            Some(chrono_tz::Asia::Shanghai),
            Lang::Zh,
        );

        assert!(text.starts_with("【定时任务执行失败】"), "标题不符：{text}");
        assert!(text.contains("任务：example"), "缺任务名：{text}");
        assert!(text.contains("原因：connection refused"), "缺原因：{text}");
        assert!(
            text.contains("时间：2026-09-18 11:30:00"),
            "时间应按 Asia/Shanghai 渲染：{text}"
        );
    }

    /// 英文正文：四行标签全部为英文（通知收件人看到的是当前界面语言）。
    #[test]
    fn renders_failure_message_in_english() {
        let text = render_failure("example", "boom", at(), Some(chrono_tz::UTC), Lang::En);

        assert!(text.starts_with("[Cron job failed]"), "标题不符：{text}");
        assert!(text.contains("Job: example"), "缺任务名：{text}");
        assert!(text.contains("Reason: boom"), "缺原因：{text}");
        assert!(
            text.contains("Time: 2026-09-18 03:30:00"),
            "时间应按 UTC 渲染：{text}"
        );
    }

    /// 时区未设置时回落服务器本地时区，不 panic（只断言其余三行稳定）。
    #[test]
    fn renders_without_timezone_uses_local() {
        let text = render_failure("example", "boom", at(), None, Lang::Zh);
        assert!(text.contains("任务：example"));
        assert!(text.contains("原因：boom"));
        assert!(text.contains("时间："));
    }
}
