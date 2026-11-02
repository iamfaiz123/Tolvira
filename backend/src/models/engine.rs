use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub r#type: String, // "wasm", "js"
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    pub hash: String,
    pub size: usize,
    pub capabilities: Vec<String>,
    pub dependencies: Vec<String>,
}

