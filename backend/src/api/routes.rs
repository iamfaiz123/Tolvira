use axum::{routing::get, Router};
use std::sync::Arc;
use crate::handlers::{categories, engines, health, tools, health::AppState};

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/health", get(health::health_check))
        .route("/api/v1/tools", get(tools::list_tools))
        .route("/api/v1/tools/{id}", get(tools::get_tool_by_id))
        .route("/api/v1/categories", get(categories::list_categories))
        .route("/api/v1/engines", get(engines::list_engines))
        .route("/api/v1/engines/{id}", get(engines::get_engine_by_id))
        .route("/api/healthz", get(health::health_check))
        .with_state(state)
}
