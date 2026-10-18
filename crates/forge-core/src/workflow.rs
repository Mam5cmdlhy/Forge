use crate::error::{ForgeError, Result};
use crate::model::TaskStatus;

/// Validates workflow transitions between task statuses.
pub fn validate_task_transition(from: TaskStatus, to: TaskStatus) -> Result<()> {
    if from == to {
        return Ok(());
    }

    let valid = match from {
        TaskStatus::Backlog => matches!(to, TaskStatus::Todo | TaskStatus::Cancelled),
        TaskStatus::Todo => matches!(
            to,
            TaskStatus::InProgress | TaskStatus::Cancelled | TaskStatus::Backlog
        ),
        TaskStatus::InProgress => matches!(
            to,
            TaskStatus::InReview | TaskStatus::Todo | TaskStatus::Cancelled
        ),
        TaskStatus::InReview => matches!(
            to,
            TaskStatus::Done | TaskStatus::InProgress | TaskStatus::Cancelled
        ),
        TaskStatus::Done => matches!(to, TaskStatus::InProgress | TaskStatus::Backlog),
        TaskStatus::Cancelled => matches!(to, TaskStatus::Backlog | TaskStatus::Todo),
    };

    if valid {
        Ok(())
    } else {
        Err(ForgeError::InvalidTransition {
            from: from.as_str().to_string(),
            to: to.as_str().to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_transitions() {
        assert!(validate_task_transition(TaskStatus::Backlog, TaskStatus::Todo).is_ok());
        assert!(validate_task_transition(TaskStatus::Todo, TaskStatus::InProgress).is_ok());
        assert!(validate_task_transition(TaskStatus::InProgress, TaskStatus::InReview).is_ok());
        assert!(validate_task_transition(TaskStatus::InReview, TaskStatus::Done).is_ok());
        // Invalid jump directly from backlog to done
        assert!(validate_task_transition(TaskStatus::Backlog, TaskStatus::Done).is_err());
    }
}
