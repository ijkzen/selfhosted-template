//! 飞书通知渠道。
//!
//! 自研 HTTP 客户端直调飞书官方端点，不引入第三方 SDK。三个端点族：
//!
//! - Open API（取 tenant_access_token、发消息）：`https://open.feishu.cn`
//! - 账号域（扫码一键创建应用）：`https://accounts.feishu.cn`
//!
//! 只支持飞书国内版，不做 Lark 国际版的域名切换。

pub mod feishu;
pub mod notify;
pub mod register;

use anyhow::anyhow;
use sea_orm::{ActiveModelTrait, DatabaseConnection, DbErr, EntityTrait, Set};

use crate::crypto;
use crate::entity::notification_channel;
use crate::i18n::Lang;

/// 渠道标识：当前只支持飞书。
pub const FEISHU_CHANNEL: &str = "feishu";

/// 接收者类型：飞书 `receive_id_type` 的取值子集。
pub const RECEIVER_TYPE_OPEN_ID: &str = "open_id";
pub const RECEIVER_TYPE_EMAIL: &str = "email";

/// 组装「说明：底层错误」形式的消息，分隔符随界面语言。
pub fn with_detail(lang: Lang, message: &str, detail: impl std::fmt::Display) -> String {
    format!("{message}{}{detail}", lang.tr("：", ": "))
}

/// 测试发送的消息文案。
pub fn test_message(lang: Lang) -> &'static str {
    lang.tr(
        "【selfhosted-template】这是一条测试消息，飞书通知配置可用。",
        "[selfhosted-template] This is a test message; your Feishu notification settings work.",
    )
}

/// 渠道配置明文（整段加密后落 `notification_channel.config`）。
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    pub app_id: String,
    pub app_secret: String,
    #[serde(default = "default_receiver_type")]
    pub receiver_type: String,
    pub receiver: String,
}

fn default_receiver_type() -> String {
    RECEIVER_TYPE_OPEN_ID.to_string()
}

impl FeishuConfig {
    /// 值级校验，错误消息按当前界面语言。
    pub fn validate(&self, lang: Lang) -> Result<(), String> {
        if self.app_id.trim().is_empty() {
            return Err(lang
                .tr("App ID 不能为空", "App ID cannot be empty")
                .to_string());
        }
        if self.receiver_type != RECEIVER_TYPE_OPEN_ID && self.receiver_type != RECEIVER_TYPE_EMAIL
        {
            return Err(lang
                .tr(
                    "接收者类型只能是 open_id 或 email",
                    "receiver type must be open_id or email",
                )
                .to_string());
        }
        if self.receiver.trim().is_empty() {
            return Err(lang
                .tr("接收者不能为空", "receiver cannot be empty")
                .to_string());
        }
        Ok(())
    }
}

/// 解码存储的密文配置；解不开（密钥轮换/损坏）或非法 JSON 时返回 None
/// （按「未配置」处理）。
fn decode_config(stored: &str) -> Option<FeishuConfig> {
    let plaintext = match crypto::decrypt(stored) {
        Ok(plaintext) => plaintext,
        Err(e) => {
            tracing::warn!("Failed to decrypt notification config: {e}");
            return None;
        }
    };
    match serde_json::from_str(&plaintext) {
        Ok(config) => Some(config),
        Err(e) => {
            tracing::warn!("Stored notification config is not valid JSON: {e}");
            None
        }
    }
}

/// `GET /api/notification/feishu` 的响应体。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelResponse {
    pub configured: bool,
    pub app_id: String,
    pub app_secret_masked: String,
    pub receiver_type: String,
    pub receiver: String,
    pub enable: bool,
    pub last_error: String,
    pub last_sent_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ChannelResponse {
    fn unconfigured() -> Self {
        Self {
            configured: false,
            app_id: String::new(),
            app_secret_masked: String::new(),
            receiver_type: RECEIVER_TYPE_OPEN_ID.to_string(),
            receiver: String::new(),
            enable: true,
            last_error: String::new(),
            last_sent_at: None,
        }
    }
}

/// 读取通知渠道行（单行表，按渠道常量取）。
async fn load_row(db: &DatabaseConnection) -> Result<Option<notification_channel::Model>, DbErr> {
    notification_channel::Entity::find_by_id(FEISHU_CHANNEL)
        .one(db)
        .await
}

/// 读取渠道配置用于展示（appSecret 掩码）。未配置不是错误，返回
/// `configured:false` 的空壳。
pub async fn read_channel(db: &DatabaseConnection) -> Result<ChannelResponse, DbErr> {
    let Some(row) = load_row(db).await? else {
        return Ok(ChannelResponse::unconfigured());
    };

    let Some(config) = decode_config(&row.config) else {
        return Ok(ChannelResponse::unconfigured());
    };

    Ok(ChannelResponse {
        configured: true,
        app_id: config.app_id,
        app_secret_masked: crypto::mask(&config.app_secret),
        receiver_type: config.receiver_type,
        receiver: config.receiver,
        enable: row.enable,
        last_error: row.last_error,
        last_sent_at: row.last_sent_at,
    })
}

/// 保存渠道配置。`app_secret` 为 `None`/空串时沿用库中原值（编辑时不必重输
/// 凭据）；首次创建没有原值可用，必须提供。
pub async fn save_channel(
    db: &DatabaseConnection,
    lang: Lang,
    app_id: String,
    app_secret: Option<String>,
    receiver_type: String,
    receiver: String,
    enable: bool,
) -> Result<(), String> {
    let existing = load_row(db).await.map_err(|e| {
        with_detail(
            lang,
            lang.tr("读取通知配置失败", "failed to read notification settings"),
            e,
        )
    })?;
    let stored_secret = existing
        .as_ref()
        .and_then(|row| decode_config(&row.config))
        .map(|c| c.app_secret)
        .unwrap_or_default();

    let app_secret = match app_secret.filter(|s| !s.trim().is_empty()) {
        Some(secret) => secret,
        None if !stored_secret.is_empty() => stored_secret,
        None => {
            return Err(lang
                .tr("App Secret 不能为空", "App Secret cannot be empty")
                .to_string());
        }
    };

    let config = FeishuConfig {
        app_id,
        app_secret,
        receiver_type,
        receiver,
    };
    config.validate(lang)?;

    let encoded = crypto::encrypt(&serde_json::to_string(&config).map_err(|e| {
        with_detail(
            lang,
            lang.tr(
                "序列化通知配置失败",
                "failed to serialize notification settings",
            ),
            e,
        )
    })?);
    let now = chrono::Utc::now();

    let saved = match existing {
        Some(row) => {
            let mut active: notification_channel::ActiveModel = row.into();
            active.config = Set(encoded);
            active.enable = Set(enable);
            active.updated_at = Set(now);
            active.update(db).await
        }
        None => {
            notification_channel::ActiveModel {
                channel: Set(FEISHU_CHANNEL.to_string()),
                config: Set(encoded),
                enable: Set(enable),
                last_error: Set(String::new()),
                last_sent_at: Set(None),
                updated_at: Set(now),
            }
            .insert(db)
            .await
        }
    };
    saved.map_err(|e| {
        with_detail(
            lang,
            lang.tr("保存通知配置失败", "failed to save notification settings"),
            e,
        )
    })?;
    Ok(())
}

/// 读取渠道配置明文；未配置/解不开返回 None。供发送路径使用（不经掩码）。
pub async fn load_config(db: &DatabaseConnection) -> Result<Option<FeishuConfig>, anyhow::Error> {
    let row = load_row(db)
        .await
        .map_err(|e| anyhow!("failed to read notification settings: {e}"))?;
    Ok(row.and_then(|row| decode_config(&row.config)))
}

/// 读取「已配置且已启用」的渠道配置；未配置或已停用返回 None。供自动通知使用。
pub async fn load_active_config(
    db: &DatabaseConnection,
) -> Result<Option<FeishuConfig>, anyhow::Error> {
    let row = load_row(db)
        .await
        .map_err(|e| anyhow!("failed to read notification settings: {e}"))?;
    Ok(row
        .filter(|row| row.enable)
        .and_then(|row| decode_config(&row.config)))
}

/// 回写最近一次发送结果（成功清空 `last_error`）。失败只打日志——回写本身
/// 失败不该再掩盖原始发送结果。
pub async fn record_send_result(db: &DatabaseConnection, error: Option<&str>) {
    let Some(row) = load_row(db).await.ok().flatten() else {
        return;
    };

    let mut active: notification_channel::ActiveModel = row.into();
    active.last_error = Set(error.unwrap_or_default().to_string());
    active.last_sent_at = Set(Some(chrono::Utc::now()));
    if let Err(e) = active.update(db).await {
        tracing::warn!("Failed to record notification send result: {e}");
    }
}

/// 用给定值发一条测试消息（**不落库配置**，支持保存前先验证）。
/// `app_secret` 缺省/空串时回落库中已存值（刷新页面后可直接测试）。
pub async fn send_test(
    db: &DatabaseConnection,
    lang: Lang,
    app_id: String,
    app_secret: Option<String>,
    receiver_type: String,
    receiver: String,
) -> Result<(), String> {
    let stored = load_config(db).await.map_err(|e| e.to_string())?;
    let app_secret = match app_secret.filter(|s| !s.trim().is_empty()) {
        Some(secret) => secret,
        None => stored.map(|c| c.app_secret).unwrap_or_default(),
    };

    let config = FeishuConfig {
        app_id,
        app_secret,
        receiver_type,
        receiver,
    };
    config.validate(lang)?;
    if config.app_secret.trim().is_empty() {
        return Err(lang
            .tr("App Secret 不能为空", "App Secret cannot be empty")
            .to_string());
    }

    let result = feishu::send(&config, test_message(lang)).await;
    record_send_result(db, result.as_ref().err().map(String::as_str)).await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(app_id: &str, receiver_type: &str, receiver: &str) -> FeishuConfig {
        FeishuConfig {
            app_id: app_id.to_string(),
            app_secret: "secret-value-1234".to_string(),
            receiver_type: receiver_type.to_string(),
            receiver: receiver.to_string(),
        }
    }

    #[test]
    fn validate_accepts_open_id_and_email_receivers() {
        assert!(
            config("cli_x", RECEIVER_TYPE_OPEN_ID, "ou_x")
                .validate(Lang::Zh)
                .is_ok()
        );
        assert!(
            config("cli_x", RECEIVER_TYPE_EMAIL, "ops@example.com")
                .validate(Lang::Zh)
                .is_ok()
        );
    }

    #[test]
    fn validate_rejects_blank_fields_and_unknown_receiver_type() {
        assert!(
            config("", RECEIVER_TYPE_OPEN_ID, "ou_x")
                .validate(Lang::Zh)
                .is_err()
        );
        assert!(
            config("cli_x", RECEIVER_TYPE_OPEN_ID, "  ")
                .validate(Lang::Zh)
                .is_err()
        );
        assert!(
            config("cli_x", "chat_id", "oc_x")
                .validate(Lang::Zh)
                .is_err()
        );
    }

    /// 校验消息跟随界面语言（默认 zh-CN 与改造前一致）。
    #[test]
    fn validate_messages_follow_language() {
        let err = config("", RECEIVER_TYPE_OPEN_ID, "ou_x")
            .validate(Lang::En)
            .unwrap_err();
        assert!(err.contains("App ID"), "英文语言应给英文消息：{err}");

        let err = config("", RECEIVER_TYPE_OPEN_ID, "ou_x")
            .validate(Lang::Zh)
            .unwrap_err();
        assert!(
            err.contains("App ID 不能为空"),
            "中文语言应给中文消息：{err}"
        );
    }

    /// 缺省 receiverType 的 JSON（老数据/手写配置）回落 open_id。
    #[test]
    fn config_json_defaults_receiver_type_to_open_id() {
        let parsed: FeishuConfig =
            serde_json::from_str(r#"{"appId":"cli_x","appSecret":"s","receiver":"ou_x"}"#).unwrap();
        assert_eq!(parsed.receiver_type, RECEIVER_TYPE_OPEN_ID);
    }

    /// 解不开的密文按「未配置」处理，不 panic。
    #[test]
    fn decode_config_returns_none_for_undecryptable_value() {
        temp_env::with_vars([(crate::crypto::SECRET_KEY_ENV, Some("key-a"))], || {
            let encoded = crypto::encrypt(r#"{"appId":"cli_x","appSecret":"s","receiver":"ou_x"}"#);
            // 换一把密钥后解不开 → None（而非 Err/panic）。
            temp_env::with_vars([(crate::crypto::SECRET_KEY_ENV, Some("key-b"))], || {
                assert!(decode_config(&encoded).is_none());
            });
        });
    }

    #[test]
    fn decode_config_returns_none_for_invalid_json() {
        temp_env::with_vars([(crate::crypto::SECRET_KEY_ENV, None::<&str>)], || {
            assert!(decode_config("not json").is_none());
        });
    }
}
