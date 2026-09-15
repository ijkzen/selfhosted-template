mod auth;
mod cron_jobs;
mod notes;
mod settings;

use axum::Json;
use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;

use crate::middleware as http_middleware;
use crate::state::AppState;

/// 组装完整应用路由（含登录拦截中间件）。
pub fn create_app(state: &AppState) -> Router {
    let router = Router::new()
        .route("/api/healthz", get(healthz))
        .nest("/api/auth", auth::routes())
        .nest("/api/cron-jobs", cron_jobs::routes())
        .nest("/api/notes", notes::routes())
        .nest("/api/settings", settings::routes())
        .fallback(api_aware_fallback)
        .layer(DefaultBodyLimit::max(5 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::auth::auth_middleware,
        ))
        .with_state(state.clone());

    http_middleware::apply(router)
}

/// 静态资源回退，但 `/api/` 前缀的不存在路径返回 JSON 404：拼错的 API 路径
/// 此前会拿到 index.html（200 text/html），误导客户端与排障。
async fn api_aware_fallback(
    uri: axum::http::Uri,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    let path = uri.path();
    if path.starts_with("/api/") {
        let lang = crate::app_settings::AppSettings::lang_sync();
        let msg = if lang == crate::i18n::Lang::En {
            format!("API endpoint not found: {path}")
        } else {
            format!("接口不存在：{path}")
        };
        return crate::response::not_found::<()>(msg).into_response();
    }
    match crate::static_assets::serve_asset(uri, headers).await {
        Ok(response) => response,
        Err(status) => status.into_response(),
    }
}

async fn healthz() -> Json<serde_json::Value> {
    // version 编译期注入，跟随 Cargo.toml 的 version，供前端侧边栏展示与部署探活。
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}
