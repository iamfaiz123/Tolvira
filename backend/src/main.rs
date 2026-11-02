use axum::http::Method;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod handlers;
mod models;
mod registry;
mod services;

use config::AppConfig;
use handlers::health::AppState;
use registry::{EngineRegistry, ToolRegistry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::from_env();
    let addr = format!("{}:{}", config.host, config.port);

    tracing::info!("Initializing Dev Toolbox Engine & Tool Registries...");
    let engine_registry = Arc::new(EngineRegistry::new()?);
    let tool_registry = Arc::new(ToolRegistry::new(&engine_registry)?);

    tracing::info!(
        "Registries initialized: {} tools, {} engines available",
        tool_registry.total_tools(),
        engine_registry.list().len()
    );

    let state = Arc::new(AppState {
        tool_registry,
        engine_registry,
    });

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(tower_http::cors::Any);

    let app = api::create_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    tracing::info!("Starting Dev Toolbox Backend HTTP server at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
