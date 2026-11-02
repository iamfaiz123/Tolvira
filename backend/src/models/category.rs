use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Category {
    pub name: String,
    pub slug: String,
    pub description: String,
    pub count: usize,
}

