use forge_core::error::{ForgeError, Result};
use forge_core::model::AuditEntry;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// A durable audit log stored outside SQLite in a plain JSONL file (`.forge/audit.jsonl`).
///
/// Ensures that even if SQLite (`.forge/forge.db`) is deleted or rebuilt from scratch,
/// the complete historical record of project changes is never lost.
pub struct DurableAuditLog {
    path: PathBuf,
}

impl DurableAuditLog {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Appends an audit entry to the durable audit log on disk.
    pub fn append(&self, entry: &AuditEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        let line = serde_json::to_string(entry)
            .map_err(|e| ForgeError::Store(format!("Failed to serialize audit entry: {e}")))?;

        writeln!(file, "{line}")?;
        file.flush()?;
        Ok(())
    }

    /// Reads all audit entries from the durable log file.
    pub fn read_all(&self) -> Result<Vec<AuditEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let entry: AuditEntry = serde_json::from_str(trimmed).map_err(|e| {
                ForgeError::Store(format!(
                    "Corrupt audit log at {}:{}: {e}",
                    self.path.display(),
                    line_num + 1
                ))
            })?;
            entries.push(entry);
        }

        Ok(entries)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_core::id::EntityId;
    use forge_core::model::{Attribution, AuditAction};

    #[test]
    fn test_durable_audit_log_persists_outside_sqlite() {
        let temp_dir = tempfile::tempdir().unwrap();
        let log_path = temp_dir.path().join("audit.jsonl");
        let audit = DurableAuditLog::new(&log_path);

        let entity_id = EntityId::new();
        let entry1 = AuditEntry::new(
            entity_id,
            "Task",
            AuditAction::Created,
            Attribution::human("Alice"),
        );
        let mut entry2 = AuditEntry::new(
            entity_id,
            "Task",
            AuditAction::StatusChanged,
            Attribution::human("Bob"),
        );
        entry2.old_value = Some("todo".to_string());
        entry2.new_value = Some("in_progress".to_string());

        audit.append(&entry1).unwrap();
        audit.append(&entry2).unwrap();

        let loaded = audit.read_all().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].action, AuditAction::Created);
        assert_eq!(loaded[1].action, AuditAction::StatusChanged);
        assert_eq!(loaded[1].new_value, Some("in_progress".to_string()));
    }
}
