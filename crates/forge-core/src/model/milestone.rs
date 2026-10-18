use crate::id::EntityId;
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MilestoneStatus {
    Planned,
    InProgress,
    Completed,
    Missed,
}

impl Default for MilestoneStatus {
    fn default() -> Self {
        Self::Planned
    }
}

/// A Milestone entity stored canonically in `milestones/<slug>.yaml`.
///
/// NOTE: The tasks associated with a milestone are derived from tasks
/// whose `milestone_id` points to this milestone. They are NOT duplicated
/// canonically in this file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Milestone {
    #[serde(default = "default_schema_version")]
    pub forge_schema: String,

    #[serde(default = "default_kind_milestone")]
    pub kind: String,

    pub id: EntityId,
    pub name: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub status: MilestoneStatus,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_date: Option<NaiveDate>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_date: Option<NaiveDate>,

    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

fn default_kind_milestone() -> String {
    "Milestone".to_string()
}

impl Milestone {
    pub fn new(name: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            forge_schema: default_schema_version(),
            kind: default_kind_milestone(),
            id: EntityId::new(),
            name: name.into(),
            description: String::new(),
            status: MilestoneStatus::Planned,
            target_date: None,
            completed_date: None,
            created_at: now,
            updated_at: now,
        }
    }
}
