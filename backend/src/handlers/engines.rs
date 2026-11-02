use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use std::sync::Arc;
use crate::handlers::health::AppState;
use crate::models::EngineManifest;

pub async fn list_engines(State(state): State<Arc<AppState>>) -> Json<Vec<EngineManifest>> {
    Json(state.engine_registry.list())
}

pub async fn get_engine_by_id(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.engine_registry.get(&id) {
        Some(manifest) => (StatusCode::OK, Json(manifest.clone())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Engine not found",
                "id": id
            })),
        )
            .into_response(),
    }
}
