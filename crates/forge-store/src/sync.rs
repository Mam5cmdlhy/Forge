use crate::audit::DurableAuditLog;
use crate::db::SqliteIndex;
use crate::project_dir::ProjectPaths;
use forge_core::error::{ForgeError, Result};
use forge_core::model::{Attribution, AuditAction, AuditEntry};
use forge_yaml::{read_milestone_file, read_project_file, read_task_file};
use std::fs;
use std::path::Path;

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct RebuildStats {
    pub projects_indexed: usize,
    pub tasks_indexed: usize,
    pub milestones_indexed: usize,
    pub dependencies_indexed: usize,
}

/// Rebuilds the SQLite index entirely from canonical YAML files on disk.
/// Proves that SQLite is 100% disposable and rebuildable at any time.
pub fn rebuild_index(project_root: &Path) -> Result<RebuildStats> {
    let paths = ProjectPaths::new(project_root);
    if !paths.forge_yaml().exists() {
        return Err(ForgeError::Project(format!(
            "Missing forge.yaml in '{}'",
            project_root.display()
        )));
    }

    let mut stats = RebuildStats::default();

    // 1. Open SQLite and ensure migrations are applied
    let mut index = SqliteIndex::open(paths.db_path())?;
    index.clear_all()?;

    // 2. Index the project file
    let project = read_project_file(paths.forge_yaml())?;
    index.upsert_entity(
        project.id,
        "Project",
        &project.name,
        "active",
        None,
        None,
        None,
        "forge.yaml",
        &project.updated_at.to_rfc3339(),
    )?;
    stats.projects_indexed += 1;

    // 3. Index milestones
    if paths.milestones_dir().is_dir() {
        for entry in fs::read_dir(paths.milestones_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                match read_milestone_file(&path) {
                    Ok(milestone) => {
                        let rel_path = path
                            .strip_prefix(project_root)
                            .unwrap_or(&path)
                            .to_string_lossy();
                        index.upsert_entity(
                            milestone.id,
                            "Milestone",
                            &milestone.name,
                            "planned",
                            None,
                            None,
                            None,
                            &rel_path,
                            &milestone.updated_at.to_rfc3339(),
                        )?;
                        stats.milestones_indexed += 1;
                    }
                    Err(e) => {
                        eprintln!("Warning: Skipping invalid milestone {}: {e}", path.display());
                    }
                }
            }
        }
    }

    // 4. Index tasks
    if paths.tasks_dir().is_dir() {
        for entry in fs::read_dir(paths.tasks_dir())? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "yaml" || ext == "yml") {
                match read_task_file(&path) {
                    Ok(task) => {
                        let rel_path = path
                            .strip_prefix(project_root)
                            .unwrap_or(&path)
                            .to_string_lossy();
                        index.upsert_entity(
                            task.id,
                            "Task",
                            &task.title,
                            task.status.as_str(),
                            None,
                            task.milestone_id,
                            task.parent_id,
                            &rel_path,
                            &task.updated_at.to_rfc3339(),
                        )?;
                        if !task.depends_on.is_empty() {
                            index.set_dependencies(task.id, &task.depends_on)?;
                            stats.dependencies_indexed += task.depends_on.len();
                        }
                        stats.tasks_indexed += 1;
                    }
                    Err(e) => {
                        eprintln!("Warning: Skipping invalid task {}: {e}", path.display());
                    }
                }
            }
        }
    }

    // 5. Append durable audit entry outside SQLite
    let audit = DurableAuditLog::new(paths.audit_log_path());
    let audit_entry = AuditEntry::new(
        project.id,
        "Project",
        AuditAction::RebuiltIndex,
        Attribution::system(),
    );
    let _ = audit.append(&audit_entry);

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_dir::init_project;
    use forge_core::model::Task;
    use forge_yaml::write_task_file;

    #[test]
    fn test_sqlite_rebuild_from_scratch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_dir = temp_dir.path();

        // 1. Initialize project
        let _project = init_project(project_dir, "Test Rebuild", "Testing rebuild index", "Tester").unwrap();

        // 2. Add two tasks to tasks/
        let paths = ProjectPaths::new(project_dir);
        let t1 = Task::new("First task");
        let mut t2 = Task::new("Second task");
        t2.depends_on.push(t1.id);

        write_task_file(paths.tasks_dir().join("task1.yaml"), &t1).unwrap();
        write_task_file(paths.tasks_dir().join("task2.yaml"), &t2).unwrap();

        // 3. Delete SQLite database file entirely to simulate database loss / crash
        let db_file = paths.db_path();
        if db_file.exists() {
            fs::remove_file(&db_file).unwrap();
        }
        assert!(!db_file.exists(), "SQLite file must be gone");

        // 4. Rebuild index from YAML files
        let stats = rebuild_index(project_dir).unwrap();
        assert_eq!(stats.projects_indexed, 1);
        assert_eq!(stats.tasks_indexed, 2);
        assert_eq!(stats.dependencies_indexed, 1);

        // 5. Query newly rebuilt SQLite index
        let index = SqliteIndex::open(&db_file).unwrap();
        let tasks = index.list_entities("Task").unwrap();
        assert_eq!(tasks.len(), 2);

        // Check derived reverse relationship survives rebuild:
        let blocked = index.get_blocked_tasks(t1.id).unwrap();
        assert_eq!(blocked, vec![t2.id.to_string()]);

        // Check durable audit log is intact outside SQLite
        let audit = DurableAuditLog::new(paths.audit_log_path());
        let entries = audit.read_all().unwrap();
        assert!(entries.iter().any(|e| e.action == AuditAction::RebuiltIndex));
    }
}
