use axum::{extract::State, Json};
use std::sync::Arc;
use crate::handlers::health::AppState;
use crate::models::Category;

pub async fn list_categories(State(state): State<Arc<AppState>>) -> Json<Vec<Category>> {
    Json(state.tool_registry.list_categories().to_vec())
}
