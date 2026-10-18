use crate::id::EntityId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    Human,
    AiAgent,
    System,
}

impl Default for ActorType {
    fn default() -> Self {
        Self::Human
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribution {
    pub actor_type: ActorType,
    pub actor_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_provider: Option<String>,
}

impl Attribution {
    pub fn human(name: impl Into<String>) -> Self {
        Self {
            actor_type: ActorType::Human,
            actor_name: name.into(),
            agent_provider: None,
        }
    }

    pub fn agent(name: impl Into<String>, provider: impl Into<String>) -> Self {
        Self {
            actor_type: ActorType::AiAgent,
            actor_name: name.into(),
            agent_provider: Some(provider.into()),
        }
    }

    pub fn system() -> Self {
        Self {
            actor_type: ActorType::System,
            actor_name: "forge".to_string(),
            agent_provider: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Created,
    Updated,
    Deleted,
    StatusChanged,
    RebuiltIndex,
}

/// An audit entry representing a change made to a project entity.
///
/// Kept durable outside SQLite (e.g. in `.forge/audit.jsonl`) so that
/// SQLite can be deleted and rebuilt without losing historical audit logs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: EntityId,
    pub entity_id: EntityId,
    pub entity_type: String,
    pub action: AuditAction,
    pub actor: Attribution,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field_changed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl AuditEntry {
    pub fn new(
        entity_id: EntityId,
        entity_type: impl Into<String>,
        action: AuditAction,
        actor: Attribution,
    ) -> Self {
        Self {
            id: EntityId::new(),
            entity_id,
            entity_type: entity_type.into(),
            action,
            actor,
            timestamp: Utc::now(),
            field_changed: None,
            old_value: None,
            new_value: None,
            message: None,
        }
    }
}
