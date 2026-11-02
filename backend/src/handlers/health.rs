use axum::{extract::State, Json};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;
use crate::registry::{EngineRegistry, ToolRegistry};

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub timestamp: String,
    pub tools_indexed: usize,
    pub engines_available: Vec<String>,
}

pub struct AppState {
    pub tool_registry: Arc<ToolRegistry>,
    pub engine_registry: Arc<EngineRegistry>,
}

pub async fn health_check(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let available_engines: Vec<String> = state
        .engine_registry
        .list()
        .into_iter()
        .map(|e| format!("{}:{}", e.id, e.version))
        .collect();

    Json(HealthResponse {
        status: "ok",
        version: "1.0.0",
        timestamp: Utc::now().to_rfc3339(),
        tools_indexed: state.tool_registry.total_tools(),
        engines_available: available_engines,
    })
}
