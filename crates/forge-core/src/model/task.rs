use crate::id::EntityId;
use crate::model::audit::Attribution;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Backlog,
    Todo,
    InProgress,
    InReview,
    Done,
    Cancelled,
}

impl Default for TaskStatus {
    fn default() -> Self {
        Self::Todo
    }
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::InProgress => "in_progress",
            Self::InReview => "in_review",
            Self::Done => "done",
            Self::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "backlog" => Ok(Self::Backlog),
            "todo" => Ok(Self::Todo),
            "in_progress" | "inprogress" => Ok(Self::InProgress),
            "in_review" | "inreview" => Ok(Self::InReview),
            "done" => Ok(Self::Done),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            other => Err(format!("Unknown task status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Medium
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Estimate {
    pub unit: String,
    pub value: f64,
}

/// A Task entity stored canonically in `tasks/<slug>.yaml`.
///
/// NOTE ON RELATIONSHIPS:
/// Forward relationships are stored canonically once:
/// - `depends_on`: Tasks that this task depends on (cannot start until they are done).
/// - `milestone_id`: The milestone this task belongs to.
/// - `parent_id`: Parent task if this is a subtask.
///
/// Reverse relationships (`blocked_by`, `subtasks`, `milestone_tasks`) are NEVER
/// duplicated in YAML files. They are derived dynamically via `RelationshipGraph`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    #[serde(default = "default_schema_version")]
    pub forge_schema: String,

    #[serde(default = "default_kind_task")]
    pub kind: String,

    pub id: EntityId,
    pub title: String,

    #[serde(default)]
    pub description: String,

    #[serde(default)]
    pub status: TaskStatus,

    #[serde(default)]
    pub priority: Priority,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignee: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub milestone_id: Option<EntityId>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<EntityId>,

    /// Canonical forward dependencies: tasks that this task depends upon.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<EntityId>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimate: Option<Estimate>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, serde_json::Value>,

    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<Attribution>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<Attribution>,
}

fn default_schema_version() -> String {
    "1.0".to_string()
}

fn default_kind_task() -> String {
    "Task".to_string()
}

impl Task {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            forge_schema: default_schema_version(),
            kind: default_kind_task(),
            id: EntityId::new(),
            title: title.into(),
            description: String::new(),
            status: TaskStatus::Todo,
            priority: Priority::Medium,
            assignee: None,
            milestone_id: None,
            parent_id: None,
            depends_on: Vec::new(),
            tags: Vec::new(),
            estimate: None,
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
            created_by: None,
            updated_by: None,
        }
    }
}
