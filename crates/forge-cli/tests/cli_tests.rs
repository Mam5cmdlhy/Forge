use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_cli_end_to_end_workflow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_dir = temp_dir.path();

    // 1. forge init
    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("init")
        .arg(project_dir)
        .arg("--name")
        .arg("Test Project")
        .arg("--description")
        .arg("Integration test project")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized empty Forge project"));

    assert!(project_dir.join("forge.yaml").is_file());
    assert!(project_dir.join(".forge/forge.db").is_file());
    assert!(project_dir.join(".forge/audit.jsonl").is_file());

    // 2. forge project info
    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("--project")
        .arg(project_dir)
        .arg("project")
        .arg("info")
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Project"));

    // 3. forge task create
    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("--project")
        .arg(project_dir)
        .arg("task")
        .arg("create")
        .arg("--title")
        .arg("Custom Task")
        .arg("--status")
        .arg("todo")
        .arg("--priority")
        .arg("high")
        .assert()
        .success()
        .stdout(predicate::str::contains("Created task"));

    // 4. forge task list (JSON format)
    let mut cmd = Command::cargo_bin("forge").unwrap();
    let output = cmd
        .arg("--project")
        .arg(project_dir)
        .arg("--format")
        .arg("json")
        .arg("task")
        .arg("list")
        .assert()
        .success();

    let stdout_str = String::from_utf8(output.get_output().stdout.clone()).unwrap();
    let tasks: Vec<serde_json::Value> = serde_json::from_str(&stdout_str).unwrap();
    assert_eq!(tasks.len(), 1);
    let task_id = tasks[0]["id"].as_str().unwrap().to_string();
    assert_eq!(tasks[0]["status"].as_str().unwrap(), "todo");

    // 5. Hand-format the task YAML file with unusual formatting, comments, CRLF, and UTF-8 BOM
    let task_file_name = tasks[0]["file_path"].as_str().unwrap();
    let task_file_path = project_dir.join(task_file_name);
    assert!(task_file_path.is_file());

    let custom_yaml = format!(
        "\u{FEFF}# ========================================================\r\n\
         # Hand-crafted comments that must survive CLI status update\r\n\
         # ========================================================\r\n\
         \r\n\
         forge_schema: \"1.0\" # schema version\r\n\
         kind: Task\r\n\
         id: \"{}\"\r\n\
         title: \"Custom Task\"\r\n\
         \r\n\
         # Status section with custom comment\r\n\
         status: todo # initial state\r\n\
         priority: high\r\n\
         \r\n\
         # Custom nested metadata\r\n\
         metadata:\r\n\
             special_field:   \"custom value with spacing\"\r\n\
         \r\n\
         # Trailing comments\r\n",
        task_id
    );
    fs::write(&task_file_path, &custom_yaml).unwrap();

    // 6. Update status via CLI: forge task status <id> in_progress
    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("--project")
        .arg(project_dir)
        .arg("task")
        .arg("status")
        .arg(&task_id)
        .arg("in_progress")
        .assert()
        .success()
        .stdout(predicate::str::contains("lossless edit verified"));

    // Verify file on disk: comments, blank lines, CRLF, and BOM MUST survive!
    let updated_content = fs::read_to_string(&task_file_path).unwrap();
    let expected_content = custom_yaml.replace("status: todo", "status: in_progress");
    assert_eq!(
        updated_content, expected_content,
        "CLI task status update must losslessly preserve all comments, blank lines, indentation, CRLF, and BOM!"
    );
    assert!(updated_content.starts_with('\u{FEFF}'), "BOM must survive CLI edit");
    assert!(updated_content.contains("\r\n"), "CRLF must survive CLI edit");

    // 7. Test SQLite index rebuildability: delete forge.db and rebuild
    let db_path = project_dir.join(".forge/forge.db");
    fs::remove_file(&db_path).unwrap();
    assert!(!db_path.exists(), "forge.db has been deleted");

    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("--project")
        .arg(project_dir)
        .arg("rebuild-index")
        .assert()
        .success()
        .stdout(predicate::str::contains("Successfully rebuilt SQLite index"));

    assert!(db_path.exists(), "forge.db has been completely rebuilt from YAML files");

    // Verify task list works again from rebuilt database
    let mut cmd = Command::cargo_bin("forge").unwrap();
    cmd.arg("--project")
        .arg(project_dir)
        .arg("task")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Custom Task"))
        .stdout(predicate::str::contains("in_progress"));

    // Verify durable audit log is intact outside SQLite
    let audit_file = project_dir.join(".forge/audit.jsonl");
    assert!(audit_file.is_file());
    let audit_lines = fs::read_to_string(audit_file).unwrap();
    assert!(audit_lines.contains("created"));
    assert!(audit_lines.contains("status_changed"));
    assert!(audit_lines.contains("rebuilt_index"));
}
