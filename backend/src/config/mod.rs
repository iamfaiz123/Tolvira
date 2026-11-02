use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub frontend_url: String,
    pub static_engine_base_url: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(5000);
        let frontend_url = env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:5173".to_string());
        let static_engine_base_url = env::var("STATIC_ENGINE_BASE_URL")
            .unwrap_or_else(|_| "/engines".to_string());

        Self {
            host,
            port,
            frontend_url,
            static_engine_base_url,
        }
    }
}

