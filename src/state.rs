use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::app_settings::AppSettings;
use crate::cron::log_capture::JobLogEvent;
use crate::cron::scheduler::SchedulerRuntime;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub scheduler: SchedulerRuntime,
    /// 任务日志事件广播通道，SSE 端点订阅后按任务名过滤推送。
    /// 载荷为 `Arc`：捕获侧每事件只分配一次，各订阅者只克隆指针。
    pub log_tx: broadcast::Sender<Arc<JobLogEvent>>,
    /// 语言/时区设置缓存（设置页更新后热刷新）。
    pub settings: AppSettings,
    /// 飞书扫码创建的注册会话槽（单槽位，内存态，见 `notification::register`）。
    pub feishu_registration: crate::notification::register::RegistrationSlot,
}
