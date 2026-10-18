use crate::audit::DurableAuditLog;
use crate::db::SqliteIndex;
use forge_core::error::{ForgeError, Result};
use forge_core::model::{Attribution, AuditAction, AuditEntry, Project};
use forge_yaml::{read_project_file, write_project_file};
use std::fs;
use std::path::{Path, PathBuf};

pub struct ProjectPaths {
    pub root: PathBuf,
}

impl ProjectPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn forge_yaml(&self) -> PathBuf {
        self.root.join("forge.yaml")
    }

    pub fn dot_forge_dir(&self) -> PathBuf {
        self.root.join(".forge")
    }

    pub fn db_path(&self) -> PathBuf {
        self.dot_forge_dir().join("forge.db")
    }

    pub fn audit_log_path(&self) -> PathBuf {
        self.dot_forge_dir().join("audit.jsonl")
    }

    pub fn tasks_dir(&self) -> PathBuf {
        self.root.join("tasks")
    }

    pub fn milestones_dir(&self) -> PathBuf {
        self.root.join("milestones")
    }

    /// Finds a project root starting from `start_dir` and walking upwards.
    pub fn find_root(start_dir: &Path) -> Result<PathBuf> {
        let mut curr = start_dir.canonicalize().unwrap_or_else(|_| start_dir.to_path_buf());
        loop {
            if curr.join("forge.yaml").is_file() {
                return Ok(curr);
            }
            if !curr.pop() {
                break;
            }
        }
        Err(ForgeError::Project(format!(
            "No Forge project found in '{}' or any parent directory",
            start_dir.display()
        )))
    }
}

/// Initializes a new Forge project in `target_dir`.
pub fn init_project(
    target_dir: &Path,
    name: &str,
    description: &str,
    actor_name: &str,
) -> Result<Project> {
    let paths = ProjectPaths::new(target_dir);

    if paths.forge_yaml().exists() {
        return Err(ForgeError::Project(format!(
            "Project already exists at '{}'",
            target_dir.display()
        )));
    }

    // 1. Create directories
    fs::create_dir_all(&paths.root)?;
    fs::create_dir_all(paths.dot_forge_dir())?;
    fs::create_dir_all(paths.tasks_dir())?;
    fs::create_dir_all(paths.milestones_dir())?;

    // 2. Create .gitignore in .forge so cache is not committed
    let gitignore_path = paths.dot_forge_dir().join(".gitignore");
    fs::write(gitignore_path, "forge.db\nforge.db-wal\nforge.db-shm\n")?;

    // 3. Write canonical forge.yaml
    let project = Project::new(name, description);
    write_project_file(paths.forge_yaml(), &project)?;

    // 4. Initialize durable audit log outside SQLite
    let audit = DurableAuditLog::new(paths.audit_log_path());
    let init_audit = AuditEntry::new(
        project.id,
        "Project",
        AuditAction::Created,
        Attribution::human(actor_name),
    );
    audit.append(&init_audit)?;

    // 5. Initialize SQLite index with migrations and index the project
    let index = SqliteIndex::open(paths.db_path())?;
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

    Ok(project)
}

/// Loads the project definition from a project directory.
pub fn load_project(root: &Path) -> Result<Project> {
    let paths = ProjectPaths::new(root);
    read_project_file(paths.forge_yaml())
}
