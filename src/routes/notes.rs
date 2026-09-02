use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
};
use sea_orm::{ActiveModelTrait, EntityTrait, ModelTrait, QueryOrder, Set};
use serde::{Deserialize, Serialize};

use crate::entity::note;
use crate::i18n::Lang;
use crate::response::{self, Response};
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_notes).post(create_note))
        .route("/{id}", put(update_note).delete(delete_note))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NoteResponse {
    id: i32,
    title: String,
    content: String,
    created_at: String,
    updated_at: String,
}

impl From<note::Model> for NoteResponse {
    fn from(model: note::Model) -> Self {
        Self {
            id: model.id,
            title: model.title,
            content: model.content,
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Deserialize)]
struct UpsertNoteRequest {
    title: String,
    content: String,
}

fn validate(req: &UpsertNoteRequest, lang: Lang) -> Option<String> {
    if req.title.trim().is_empty() || req.title.len() > 200 {
        return Some(
            lang.tr("标题需为 1-200 个字符", "title must be 1-200 characters")
                .to_string(),
        );
    }
    if req.content.len() > 10_000 {
        return Some(
            lang.tr(
                "内容不能超过 10000 个字符",
                "content must be at most 10000 characters",
            )
            .to_string(),
        );
    }
    None
}

async fn list_notes(State(state): State<AppState>) -> impl IntoResponse {
    match note::Entity::find()
        .order_by_desc(note::Column::UpdatedAt)
        .all(&state.db)
        .await
    {
        Ok(models) => {
            let response: Vec<NoteResponse> = models.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(Response::success(response)))
        }
        Err(e) => response::db_error(e.to_string()),
    }
}

async fn create_note(
    State(state): State<AppState>,
    Json(req): Json<UpsertNoteRequest>,
) -> impl IntoResponse {
    let lang = state.settings.lang().await;
    if let Some(msg) = validate(&req, lang) {
        return response::bad_request(msg);
    }

    let now = chrono::Utc::now();
    let model = note::ActiveModel {
        title: Set(req.title.trim().to_string()),
        content: Set(req.content),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };
    match model.insert(&state.db).await {
        Ok(model) => (
            StatusCode::OK,
            Json(Response::success(NoteResponse::from(model))),
        ),
        Err(e) => response::db_error(e.to_string()),
    }
}

async fn update_note(
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Json(req): Json<UpsertNoteRequest>,
) -> impl IntoResponse {
    let lang = state.settings.lang().await;
    if let Some(msg) = validate(&req, lang) {
        return response::bad_request(msg);
    }

    match note::Entity::find_by_id(id).one(&state.db).await {
        Ok(Some(model)) => {
            let mut active: note::ActiveModel = model.into();
            active.title = Set(req.title.trim().to_string());
            active.content = Set(req.content);
            active.updated_at = Set(chrono::Utc::now());
            match active.update(&state.db).await {
                Ok(model) => (
                    StatusCode::OK,
                    Json(Response::success(NoteResponse::from(model))),
                ),
                Err(e) => response::db_error(e.to_string()),
            }
        }
        Ok(None) => response::not_found(not_found_msg(lang)),
        Err(e) => response::db_error(e.to_string()),
    }
}

async fn delete_note(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let lang = state.settings.lang().await;
    match note::Entity::find_by_id(id).one(&state.db).await {
        Ok(Some(model)) => match model.delete(&state.db).await {
            Ok(_) => (StatusCode::OK, Json(Response::success(()))),
            Err(e) => response::db_error(e.to_string()),
        },
        Ok(None) => response::not_found(not_found_msg(lang)),
        Err(e) => response::db_error(e.to_string()),
    }
}

fn not_found_msg(lang: Lang) -> String {
    if lang == Lang::En {
        "note does not exist".to_string()
    } else {
        "笔记不存在".to_string()
    }
}
