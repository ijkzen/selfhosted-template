//! 认证集成测试：初始化流程、登录、会话拦截、修改密码、登出与设置读写。

mod common;

use axum::body::Body;
use axum::http::{Request, header};
use serde_json::{Value, json};
use tower::ServiceExt;

const ADMIN: &str = "Admin";
const PASSWORD: &str = "Password";

async fn setup_app() -> (axum::Router, sea_orm::DatabaseConnection) {
    let (db, scheduler, log_tx) = common::setup_db_and_scheduler().await;
    let app = common::build_app_loaded(db.clone(), scheduler, log_tx).await;
    (app, db)
}

type TestResponse = (u16, Vec<(String, String)>, Value);

async fn send_with_headers(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    extra_headers: &[(&str, &str)],
) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    for (name, value) in extra_headers {
        builder = builder.header(*name, *value);
    }
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    let request = builder
        .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let headers: Vec<(String, String)> = response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, headers, parsed)
}

async fn send_json(app: axum::Router, method: &str, uri: &str, body: Value) -> TestResponse {
    send_with_headers(app, method, uri, Some(body), &[]).await
}

/// 从 Set-Cookie 响应头提取 session 令牌。
fn session_token_from(headers: &[(String, String)]) -> Option<String> {
    headers.iter().find_map(|(name, value)| {
        if name == "set-cookie" && value.starts_with("session=") {
            value
                .split(';')
                .next()
                .and_then(|pair| pair.split_once('='))
                .map(|(_, token)| token.to_string())
        } else {
            None
        }
    })
}

fn cookie(token: &str) -> [(&'static str, String); 1] {
    [("cookie", format!("session={token}"))]
}

async fn init_admin(app: &axum::Router) -> String {
    let (status, headers, body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/init",
        json!({ "username": ADMIN, "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, 200, "init should succeed: {body}");
    assert_eq!(body["code"], "0");
    assert_eq!(body["data"]["username"], ADMIN);
    session_token_from(&headers).expect("init should set session cookie")
}

#[tokio::test]
async fn init_flow_creates_first_user_and_session() {
    let (app, _db) = setup_app().await;

    let (status, _, body) = send_json(app.clone(), "GET", "/api/auth/status", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["initialized"], false);

    let token = init_admin(&app).await;
    assert!(!token.is_empty());

    let (status, _, body) = send_json(app.clone(), "GET", "/api/auth/status", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["initialized"], true);

    // 已初始化后再次 init 被拒绝。
    let (status, _, body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/init",
        json!({ "username": "Other", "password": "Secret1" }),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["code"], "INVALID_INPUT");
}

#[tokio::test]
async fn init_rejects_invalid_input() {
    let (app, _db) = setup_app().await;
    let (status, _, _) = send_json(
        app.clone(),
        "POST",
        "/api/auth/init",
        json!({ "username": "", "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _, _) = send_json(
        app.clone(),
        "POST",
        "/api/auth/init",
        json!({ "username": ADMIN, "password": "12345" }),
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn login_and_session_guard() {
    let (app, _db) = setup_app().await;
    init_admin(&app).await;

    // 未登录访问管理接口 → 401 统一信封。
    let (status, _, body) = send_json(app.clone(), "GET", "/api/settings", json!({})).await;
    assert_eq!(status, 401);
    assert_eq!(body["code"], "UNAUTHORIZED");

    // healthz 不需要登录，并暴露编译期版本号（跟随 Cargo.toml）。
    let (status, _, body) = send_json(app.clone(), "GET", "/api/healthz", json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));

    // 错误密码 → 401。
    let (status, _, body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": "wrong-pass" }),
    )
    .await;
    assert_eq!(status, 401);
    assert_eq!(body["code"], "UNAUTHORIZED");

    // 正确登录 → 200 + Cookie。
    let (status, headers, body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, 200, "login should succeed: {body}");
    assert_eq!(body["data"]["username"], ADMIN);
    let token = session_token_from(&headers).expect("login sets cookie");
    assert!(
        headers
            .iter()
            .any(|(n, v)| n == "set-cookie" && v.contains("HttpOnly"))
    );

    // 带 Cookie 访问管理接口 → 200，且 /api/auth/me 返回用户名。
    let headers: Vec<(&str, String)> = cookie(&token).to_vec();
    let header_refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, _, body) =
        send_with_headers(app.clone(), "GET", "/api/settings", None, &header_refs).await;
    assert_eq!(status, 200);
    assert_eq!(body["code"], "0");
    let (status, _, body) =
        send_with_headers(app.clone(), "GET", "/api/auth/me", None, &header_refs).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["username"], ADMIN);

    // 伪造 Cookie → 401。
    let (status, _, _) = send_with_headers(
        app.clone(),
        "GET",
        "/api/settings",
        None,
        &[("cookie", "session=deadbeef")],
    )
    .await;
    assert_eq!(status, 401);
}

#[tokio::test]
async fn settings_read_write_roundtrip() {
    let (app, _db) = setup_app().await;
    init_admin(&app).await;

    let (status, headers, _body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, 200);
    let token = session_token_from(&headers).unwrap();
    let binding = cookie(&token);
    let cookie_refs: Vec<(&str, &str)> = binding.iter().map(|(k, v)| (*k, v.as_str())).collect();

    // 内置语言设置存在，默认 zh-CN。
    let (status, _, body) =
        send_with_headers(app.clone(), "GET", "/api/settings", None, &cookie_refs).await;
    assert_eq!(status, 200);
    assert_eq!(body["code"], "0");
    let language = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "language")
        .expect("language setting should be seeded");
    assert_eq!(language["value"], "zh-CN");

    // 更新语言 → 生效。
    let (status, _, _) = send_with_headers(
        app.clone(),
        "PUT",
        "/api/settings/language",
        Some(json!({ "value": "en" })),
        &cookie_refs,
    )
    .await;
    assert_eq!(status, 200);

    let (status, _, body) =
        send_with_headers(app.clone(), "GET", "/api/settings", None, &cookie_refs).await;
    assert_eq!(status, 200);
    let language = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "language")
        .unwrap();
    assert_eq!(language["value"], "en");

    // 非法语言 → 400。
    let (status, _, _) = send_with_headers(
        app.clone(),
        "PUT",
        "/api/settings/language",
        Some(json!({ "value": "fr" })),
        &cookie_refs,
    )
    .await;
    assert_eq!(status, 400);

    // 内置设置不可删除。
    let (status, _, _) = send_with_headers(
        app.clone(),
        "DELETE",
        "/api/settings/language",
        None,
        &cookie_refs,
    )
    .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn change_password_revokes_other_sessions() {
    let (app, _db) = setup_app().await;
    let token_a = init_admin(&app).await;

    // 第二个会话。
    let (_, headers, _) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": PASSWORD }),
    )
    .await;
    let token_b = session_token_from(&headers).unwrap();

    // 旧密码错误 → 400（携带会话 A）。
    let a_headers: Vec<(&str, String)> = cookie(&token_a).to_vec();
    let a_refs: Vec<(&str, &str)> = a_headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, _, _) = send_with_headers(
        app.clone(),
        "POST",
        "/api/auth/change-password",
        Some(json!({ "oldPassword": "wrong-old", "newPassword": "NewPassword1" })),
        &a_refs,
    )
    .await;
    assert_eq!(status, 400);

    // 正确修改（携带会话 A）。
    let a_headers: Vec<(&str, String)> = cookie(&token_a).to_vec();
    let a_refs: Vec<(&str, &str)> = a_headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, _, body) = send_with_headers(
        app.clone(),
        "POST",
        "/api/auth/change-password",
        Some(json!({ "oldPassword": PASSWORD, "newPassword": "NewPassword1" })),
        &a_refs,
    )
    .await;
    assert_eq!(status, 200, "change password should succeed: {body}");

    // 会话 B（其他会话）被吊销。
    let b_cookie = format!("session={token_b}");
    let (status, _, _) = send_with_headers(
        app.clone(),
        "GET",
        "/api/settings",
        None,
        &[("cookie", b_cookie.as_str())],
    )
    .await;
    assert_eq!(status, 401);

    // 会话 A 仍然有效。
    let (status, _, _) = send_with_headers(app.clone(), "GET", "/api/auth/me", None, &a_refs).await;
    assert_eq!(status, 200);

    // 旧密码不能再登录，新密码可以。
    let (status, _, _) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": PASSWORD }),
    )
    .await;
    assert_eq!(status, 401);
    let (status, _, body) = send_json(
        app.clone(),
        "POST",
        "/api/auth/login",
        json!({ "username": ADMIN, "password": "NewPassword1" }),
    )
    .await;
    assert_eq!(status, 200, "new password should work: {body}");
}

#[tokio::test]
async fn logout_revokes_session() {
    let (app, _db) = setup_app().await;
    let token = init_admin(&app).await;

    let headers: Vec<(&str, String)> = cookie(&token).to_vec();
    let refs: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let (status, _, _) =
        send_with_headers(app.clone(), "POST", "/api/auth/logout", None, &refs).await;
    assert_eq!(status, 200);

    let (status, _, _) = send_with_headers(app.clone(), "GET", "/api/settings", None, &refs).await;
    assert_eq!(status, 401);
}
