//! 示例域（Notes）集成测试：列表、创建、更新、删除与校验。

mod common;

use axum::body::Body;
use axum::http::Request;
use serde_json::Value;
use tower::ServiceExt;

type TestResponse = (u16, Value);

async fn setup_app() -> axum::Router {
    let (db, scheduler, log_tx) = common::setup_db_and_scheduler().await;
    common::build_authed_app(db, scheduler, log_tx).await
}

async fn send(app: axum::Router, method: &str, uri: &str, body: Option<Value>) -> TestResponse {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(Body::from(body.map(|b| b.to_string()).unwrap_or_default()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let parsed: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, parsed)
}

#[tokio::test]
async fn notes_crud_lifecycle() {
    let app = setup_app().await;

    // 初始为空列表。
    let (status, body) = send(app.clone(), "GET", "/api/notes", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["code"], "0");
    assert_eq!(body["data"].as_array().unwrap().len(), 0);

    // 创建两条。
    let (status, body) = send(
        app.clone(),
        "POST",
        "/api/notes",
        Some(serde_json::json!({ "title": "First", "content": "hello" })),
    )
    .await;
    assert_eq!(status, 200, "create should succeed: {body}");
    assert_eq!(body["data"]["title"], "First");
    let id = body["data"]["id"].as_i64().unwrap() as i32;

    let (status, body) = send(
        app.clone(),
        "POST",
        "/api/notes",
        Some(serde_json::json!({ "title": "Second", "content": "world" })),
    )
    .await;
    assert_eq!(status, 200);
    let id2 = body["data"]["id"].as_i64().unwrap() as i32;

    // 列表按更新时间倒序，两条都在。
    let (status, body) = send(app.clone(), "GET", "/api/notes", None).await;
    assert_eq!(status, 200);
    let notes = body["data"].as_array().unwrap();
    assert_eq!(notes.len(), 2);

    // 更新第一条。
    let (status, body) = send(
        app.clone(),
        "PUT",
        &format!("/api/notes/{id}"),
        Some(serde_json::json!({ "title": "First updated", "content": "hi again" })),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(body["data"]["title"], "First updated");

    // 更新不存在的 → 404。
    let (status, _) = send(
        app.clone(),
        "PUT",
        "/api/notes/9999",
        Some(serde_json::json!({ "title": "x", "content": "y" })),
    )
    .await;
    assert_eq!(status, 404);

    // 空标题 → 400。
    let (status, body) = send(
        app.clone(),
        "POST",
        "/api/notes",
        Some(serde_json::json!({ "title": "", "content": "x" })),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(body["code"], "INVALID_INPUT");

    // 删除两条，列表清空。
    let (status, _) = send(app.clone(), "DELETE", &format!("/api/notes/{id}"), None).await;
    assert_eq!(status, 200);
    let (status, _) = send(app.clone(), "DELETE", &format!("/api/notes/{id2}"), None).await;
    assert_eq!(status, 200);

    // 删除不存在的 → 404。
    let (status, _) = send(app.clone(), "DELETE", "/api/notes/9999", None).await;
    assert_eq!(status, 404);

    let (status, body) = send(app.clone(), "GET", "/api/notes", None).await;
    assert_eq!(status, 200);
    assert_eq!(body["data"].as_array().unwrap().len(), 0);
}
