use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::handlers::health::AppState;
use crate::models::{Category, Tool};

#[derive(Deserialize)]
pub struct ListToolsQuery {
    pub category: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Serialize)]
pub struct ToolListResponse {
    pub tools: Vec<Tool>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub categories: Vec<Category>,
}

pub async fn list_tools(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListToolsQuery>,
) -> Json<ToolListResponse> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 1000);

    let (tools, total) = state.tool_registry.list_tools(
        query.category.as_deref(),
        query.query.as_deref(),
        query.status.as_deref(),
        page,
        page_size,
    );

    Json(ToolListResponse {
        tools,
        total,
        page,
        page_size,
        categories: state.tool_registry.list_categories().to_vec(),
    })
}

pub async fn get_tool_by_id(
    State(state): State<Arc<AppState>>,
    Path(identifier): Path<String>,
) -> impl IntoResponse {
    let tool = if let Ok(id) = identifier.parse::<usize>() {
        state.tool_registry.get_tool_by_id(id)
    } else {
        state.tool_registry.get_tool_by_slug(&identifier)
    };

    match tool {
        Some(t) => (StatusCode::OK, Json(t.clone())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Tool not found",
                "identifier": identifier
            })),
        )
            .into_response(),
    }
}
