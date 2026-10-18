pub mod lossless;
pub mod validator;

pub use lossless::{LosslessError, LosslessYamlDoc, targeted_source_span_replace};
pub use validator::{SUPPORTED_SCHEMA_VERSION, validate_entity_kind, validate_schema_version};

use forge_core::error::{ForgeError, Result};
use forge_core::model::{Milestone, Project, Task, TaskStatus};
use std::fs;
use std::path::Path;

/// Parses a YAML string into a Project struct.
pub fn parse_project_str(yaml_str: &str) -> Result<Project> {
    let clean_str = if yaml_str.starts_with('\u{FEFF}') {
        &yaml_str['\u{FEFF}'.len_utf8()..]
    } else {
        yaml_str
    };
    let project: Project = serde_saphyr::from_str(clean_str)
        .map_err(|e| ForgeError::Yaml(format!("Failed to parse project YAML: {e}")))?;
    validate_schema_version(&project.forge_schema)?;
    validate_entity_kind(&project.kind, "Project")?;
    Ok(project)
}

/// Parses a YAML string into a Task struct.
pub fn parse_task_str(yaml_str: &str) -> Result<Task> {
    let clean_str = if yaml_str.starts_with('\u{FEFF}') {
        &yaml_str['\u{FEFF}'.len_utf8()..]
    } else {
        yaml_str
    };
    let task: Task = serde_saphyr::from_str(clean_str)
        .map_err(|e| ForgeError::Yaml(format!("Failed to parse task YAML: {e}")))?;
    validate_schema_version(&task.forge_schema)?;
    validate_entity_kind(&task.kind, "Task")?;
    Ok(task)
}

/// Parses a YAML string into a Milestone struct.
pub fn parse_milestone_str(yaml_str: &str) -> Result<Milestone> {
    let clean_str = if yaml_str.starts_with('\u{FEFF}') {
        &yaml_str['\u{FEFF}'.len_utf8()..]
    } else {
        yaml_str
    };
    let milestone: Milestone = serde_saphyr::from_str(clean_str)
        .map_err(|e| ForgeError::Yaml(format!("Failed to parse milestone YAML: {e}")))?;
    validate_schema_version(&milestone.forge_schema)?;
    validate_entity_kind(&milestone.kind, "Milestone")?;
    Ok(milestone)
}

/// Reads and parses a Project file from disk.
pub fn read_project_file(path: impl AsRef<Path>) -> Result<Project> {
    let content = fs::read_to_string(path.as_ref())?;
    parse_project_str(&content)
}

/// Writes a newly created Project to disk with standard clean formatting.
pub fn write_project_file(path: impl AsRef<Path>, project: &Project) -> Result<()> {
    let yaml = serde_saphyr::to_string(project)
        .map_err(|e| ForgeError::Yaml(format!("Failed to serialize project: {e}")))?;
    let header = format!(
        "# Forge Project Definition\n# Schema: {}\n\n",
        project.forge_schema
    );
    let full_content = format!("{header}{yaml}");
    write_atomic(path.as_ref(), &full_content)?;
    Ok(())
}

/// Reads and parses a Task file from disk.
pub fn read_task_file(path: impl AsRef<Path>) -> Result<Task> {
    let content = fs::read_to_string(path.as_ref())?;
    parse_task_str(&content)
}

/// Writes a newly created Task to disk with standard clean formatting.
pub fn write_task_file(path: impl AsRef<Path>, task: &Task) -> Result<()> {
    let yaml = serde_saphyr::to_string(task)
        .map_err(|e| ForgeError::Yaml(format!("Failed to serialize task: {e}")))?;
    write_atomic(path.as_ref(), &yaml)?;
    Ok(())
}

/// Updates the status of an existing task YAML file in place using lossless CST editing,
/// strictly preserving all comments, indentation, custom formatting, CRLF/LF line endings, and BOM.
pub fn update_task_status_lossless(path: impl AsRef<Path>, new_status: TaskStatus) -> Result<()> {
    let path = path.as_ref();
    let content = fs::read_to_string(path)?;

    let mut doc = LosslessYamlDoc::parse(&content);
    let new_status_str = new_status.as_str();

    // First attempt: CST path replacement
    let new_content = match doc.set("status", new_status_str) {
        Ok(()) => doc.to_string(),
        Err(_) => {
            // Fallback: targeted source-span replacement
            targeted_source_span_replace(&content, "status", new_status_str).ok_or_else(|| {
                ForgeError::Yaml("Failed to locate 'status' key in task YAML".to_string())
            })?
        }
    };

    write_atomic(path, &new_content)?;
    Ok(())
}

/// Reads and parses a Milestone file from disk.
pub fn read_milestone_file(path: impl AsRef<Path>) -> Result<Milestone> {
    let content = fs::read_to_string(path.as_ref())?;
    parse_milestone_str(&content)
}

/// Writes a newly created Milestone to disk.
pub fn write_milestone_file(path: impl AsRef<Path>, milestone: &Milestone) -> Result<()> {
    let yaml = serde_saphyr::to_string(milestone)
        .map_err(|e| ForgeError::Yaml(format!("Failed to serialize milestone: {e}")))?;
    write_atomic(path.as_ref(), &yaml)?;
    Ok(())
}


/// Writes data to a temporary file in the same directory and atomically renames it.
/// Prevents partial writes or file corruption on crash.
pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;

    let tmp_path = parent.join(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("forge"),
        uuid::Uuid::now_v7()
    ));

    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, path).or_else(|_| {
        // Fallback for Windows if target already exists: copy and remove tmp
        fs::copy(&tmp_path, path)?;
        let _ = fs::remove_file(&tmp_path);
        Ok::<(), std::io::Error>(())
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPRESENTATIVE_FIXTURE: &str = r#"# ==============================================================================
# Forge Task Definition
# Leading header comment explaining file purpose
# ==============================================================================

forge_schema: "1.0"   # Schema version
kind: Task            # Entity type discriminator

# Identification
id: "019c4a30-1234-7fff-a000-111111111111"
title: "Implement YAML parser" # Brief title

# Status and Priority
status: todo # Current workflow state
priority: high

# Nested mapping with unusual spacing and inline comments
metadata:
    component:   "forge-yaml"    # Component name with extra spacing
    experimental: false
    custom_field: 'single-quoted value'

# Blank line followed by array with mixed flow and block styles
tags:
  - core
  - "parser"
  - yaml

# Dependencies
depends_on: [] # Empty flow sequence

estimate:
  unit: hours
  value: 16 # Estimated hours

# Trailing footer comment
"#;

    #[test]
    fn test_lossless_edit_on_lf() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), REPRESENTATIVE_FIXTURE).unwrap();

        update_task_status_lossless(temp.path(), TaskStatus::InProgress).unwrap();

        let updated = fs::read_to_string(temp.path()).unwrap();
        let expected = REPRESENTATIVE_FIXTURE.replace("status: todo", "status: in_progress");
        assert_eq!(updated, expected, "All comments, blank lines, and unusual formatting outside status must be 100% preserved");
    }

    #[test]
    fn test_lossless_edit_on_crlf() {
        let crlf_fixture = REPRESENTATIVE_FIXTURE.replace('\n', "\r\n");
        assert!(crlf_fixture.contains("\r\n"));

        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), &crlf_fixture).unwrap();

        update_task_status_lossless(temp.path(), TaskStatus::InProgress).unwrap();

        let updated = fs::read_to_string(temp.path()).unwrap();
        let expected = crlf_fixture.replace("status: todo", "status: in_progress");

        assert_eq!(updated, expected, "CRLF line endings and formatting must survive the edit");
        assert!(updated.contains("\r\n"), "Must retain CRLF");
        assert!(!updated.contains("\r\r\n"), "Must not create corrupted double CR");
    }

    #[test]
    fn test_lossless_edit_on_utf8_bom() {
        let bom_fixture = format!("\u{FEFF}{}", REPRESENTATIVE_FIXTURE);
        assert!(bom_fixture.starts_with('\u{FEFF}'));

        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), &bom_fixture).unwrap();

        update_task_status_lossless(temp.path(), TaskStatus::InProgress).unwrap();

        let updated = fs::read_to_string(temp.path()).unwrap();
        let expected = bom_fixture.replace("status: todo", "status: in_progress");

        assert_eq!(updated, expected, "UTF-8 BOM must survive the edit");
        assert!(updated.starts_with('\u{FEFF}'), "Must retain UTF-8 BOM");
    }

    #[test]
    fn test_lossless_edit_on_both_crlf_and_bom() {
        let fixture_both = format!("\u{FEFF}{}", REPRESENTATIVE_FIXTURE.replace('\n', "\r\n"));

        let temp = tempfile::NamedTempFile::new().unwrap();
        fs::write(temp.path(), &fixture_both).unwrap();

        update_task_status_lossless(temp.path(), TaskStatus::Done).unwrap();

        let updated = fs::read_to_string(temp.path()).unwrap();
        let expected = fixture_both.replace("status: todo", "status: done");

        assert_eq!(updated, expected, "Both BOM and CRLF must survive the edit");
        assert!(updated.starts_with('\u{FEFF}'));
        assert!(updated.contains("\r\n"));
    }

    #[test]
    fn test_typed_parsing_of_fixture() {
        let task = parse_task_str(REPRESENTATIVE_FIXTURE).unwrap();
        assert_eq!(task.id.to_string(), "019c4a30-1234-7fff-a000-111111111111");
        assert_eq!(task.title, "Implement YAML parser");
        assert_eq!(task.status, TaskStatus::Todo);
        assert_eq!(task.tags, vec!["core", "parser", "yaml"]);
        assert!(task.depends_on.is_empty());
    }
}
