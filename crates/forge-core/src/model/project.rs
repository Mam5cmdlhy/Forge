use crate::id::EntityId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatus {
    Inception,
    Planning,
    Active,
    OnHold,
    Completed,
    Archived,
}

impl Default for ProjectStatus {
    fn default() -> Self {
        Self::Active
    }
}

/// The root project definition stored in `forge.yaml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    #[serde(default = "default_schema_version")]
    pub forge_schema: String,

    #[serde(default = "default_kind_project")]
    pub kind: String,

    pub id: EntityId,
    pub name: String,
    pub description: String,

    #[serde(default)]
    pub status: ProjectStatus,

    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,

    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

fn default_kind_project() -> String {
    "Project".to_string()
}

impl Project {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            forge_schema: default_schema_version(),
            kind: default_kind_project(),
            id: EntityId::new(),
            name: name.into(),
            description: description.into(),
            status: ProjectStatus::Active,
            created_at: now,
            updated_at: now,
            metadata: BTreeMap::new(),
        }
    }
}
