use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolUiConfig {
    pub component: Option<String>,
    pub lazy: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolEngineConfig {
    pub id: String,
    pub version: String,
    pub r#type: String, // "wasm", "js"
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tool {
    pub id: usize,
    pub name: String,
    pub slug: String,
    pub category: String,
    pub description: String,
    pub status: String, // "available" or "coming-soon"
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_execution")]
    pub execution: String, // "client", "worker", "server"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<ToolUiConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<ToolEngineConfig>,
    #[serde(default)]
    pub features: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_execution() -> String {
    "client".to_string()
}

