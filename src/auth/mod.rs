//! 登录认证：密码哈希、会话管理与请求拦截中间件。
//!
//! 管理后台（`/api/*`）使用 Cookie Session（HttpOnly，服务端 session 表）。

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response as AxumResponse},
};
use chrono::{DateTime, Utc};
use rand::RngCore;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};

use crate::entity::{session, user};
use crate::i18n::Lang;
use crate::response::Response;
use crate::state::AppState;

/// 会话 Cookie 名称。
pub const SESSION_COOKIE: &str = "session";
/// 会话有效期：7 天。
pub const SESSION_TTL_SECS: i64 = 7 * 24 * 3600;

/// 已通过会话认证的管理用户（注入 /api 请求 extensions）。
#[derive(Clone, Debug)]
pub struct AuthedUser {
    pub username: String,
}

// ---------- 密码哈希（argon2id） ----------

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

// ---------- 会话 ----------

/// 生成 256 位随机 hex 会话令牌（Cookie 值；库中只存其 SHA-256）。
pub fn new_session_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// 令牌摘要（session 表主键）。
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// 创建会话，返回 (明文令牌, 过期时间)。
pub async fn create_session(
    db: &DatabaseConnection,
    user_id: i32,
) -> anyhow::Result<(String, DateTime<Utc>)> {
    let token = new_session_token();
    let now = Utc::now();
    let expires_at = now + chrono::Duration::seconds(SESSION_TTL_SECS);
    let active = session::ActiveModel {
        id: Set(hash_token(&token)),
        user_id: Set(user_id),
        created_at: Set(now),
        expires_at: Set(expires_at),
    };
    active.insert(db).await?;
    Ok((token, expires_at))
}

/// 校验会话令牌，返回归属用户。过期会话顺带删除并视为无效。
pub async fn session_user(
    db: &DatabaseConnection,
    token: &str,
) -> anyhow::Result<Option<user::Model>> {
    let now = Utc::now();
    let Some(session) = session::Entity::find_by_id(hash_token(token))
        .one(db)
        .await?
    else {
        return Ok(None);
    };
    if session.expires_at <= now {
        session::Entity::delete_by_id(session.id.clone())
            .exec(db)
            .await?;
        return Ok(None);
    }
    Ok(user::Entity::find_by_id(session.user_id).one(db).await?)
}

/// 清理全部过期会话（登录时调用）。
pub async fn delete_expired_sessions(db: &DatabaseConnection) {
    let _ = session::Entity::delete_many()
        .filter(session::Column::ExpiresAt.lte(Utc::now()))
        .exec(db)
        .await;
}

/// 吊销单个会话（登出）。
pub async fn revoke_session(db: &DatabaseConnection, token: &str) {
    let _ = session::Entity::delete_by_id(hash_token(token))
        .exec(db)
        .await;
}

/// 吊销指定用户的其他会话（修改密码后踢掉旧登录，保留当前会话）。
pub async fn revoke_other_sessions(db: &DatabaseConnection, user_id: i32, keep_token: &str) {
    let keep_id = hash_token(keep_token);
    let _ = session::Entity::delete_many()
        .filter(session::Column::UserId.eq(user_id))
        .filter(session::Column::Id.ne(keep_id))
        .exec(db)
        .await;
}

// ---------- Cookie ----------

/// 从 Cookie 头中提取指定名称的值。
pub fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for pair in header.split(';') {
        let pair = pair.trim();
        if let Some((key, value)) = pair.split_once('=')
            && key.trim() == name
        {
            return Some(value.trim().to_string());
        }
    }
    None
}

/// 会话 Cookie 的 Set-Cookie 值。
pub fn session_cookie(token: &str) -> String {
    format!("{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={SESSION_TTL_SECS}")
}

/// 清除会话 Cookie 的 Set-Cookie 值。
pub fn clear_session_cookie() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0")
}

// ---------- 中间件 ----------

/// 当前进程语言（中间件等无 State 场景用；默认 zh-CN）。
async fn current_lang() -> Lang {
    match crate::app_settings::AppSettings::process_global() {
        Some(settings) => settings.lang().await,
        None => Lang::default(),
    }
}

/// 请求拦截：
/// - `/api/*`（除 `/api/auth/status|login|init`、`/api/healthz`）要求有效会话 Cookie，
///   认证后的用户信息注入 extensions（`AuthedUser`）；
/// - 其余路径（SPA 静态资源）直接放行。
pub async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> AxumResponse {
    let path = req.uri().path();

    let auth_public = path == "/api/healthz"
        || path == "/api/auth/status"
        || path == "/api/auth/login"
        || path == "/api/auth/init";
    if auth_public {
        return next.run(req).await;
    }
    if !path.starts_with("/api/") {
        return next.run(req).await;
    }

    let Some(token) = extract_cookie(req.headers(), SESSION_COOKIE) else {
        return unauthorized_session().await;
    };
    match session_user(&state.db, &token).await {
        Ok(Some(user)) => {
            let mut req = req;
            req.extensions_mut().insert(AuthedUser {
                username: user.username,
            });
            next.run(req).await
        }
        Ok(None) => unauthorized_session().await,
        Err(e) => {
            let lang = current_lang().await;
            let msg = if lang == Lang::En {
                format!("session validation failed: {e}")
            } else {
                format!("会话校验失败：{e}")
            };
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(Response::<()>::error(crate::response::INTERNAL_ERROR, msg)),
            )
                .into_response()
        }
    }
}

async fn unauthorized_session() -> AxumResponse {
    let lang = current_lang().await;
    let msg = lang.tr("未登录或登录已过期", "not logged in or session expired");
    (
        StatusCode::UNAUTHORIZED,
        Json(Response::<()>::error(crate::response::UNAUTHORIZED, msg)),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("Password").unwrap();
        assert_ne!(hash, "Password");
        assert!(verify_password("Password", &hash));
        assert!(!verify_password("password", &hash));
        assert!(!verify_password("Password", "not-a-hash"));
    }

    #[test]
    fn token_hash_is_deterministic_sha256_hex() {
        let a = hash_token("abc");
        let b = hash_token("abc");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert_ne!(a, hash_token("abd"));
    }

    #[test]
    fn session_token_is_random_hex() {
        let a = new_session_token();
        let b = new_session_token();
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn extract_cookie_parses_pairs() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::COOKIE,
            "a=1; session=tok ; b=2".parse().unwrap(),
        );
        assert_eq!(
            extract_cookie(&headers, SESSION_COOKIE).as_deref(),
            Some("tok")
        );
        assert_eq!(extract_cookie(&headers, "missing"), None);
    }

    #[test]
    fn cookie_values_roundtrip() {
        let set = session_cookie("tok");
        assert!(set.starts_with("session=tok;"));
        assert!(set.contains("HttpOnly"));
        assert!(set.contains("SameSite=Lax"));
        let clear = clear_session_cookie();
        assert!(clear.contains("Max-Age=0"));
    }
}
