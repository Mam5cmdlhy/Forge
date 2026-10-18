use clap::{Parser, Subcommand, ValueEnum};
use forge_core::model::{Attribution, AuditAction, AuditEntry, Priority, Task, TaskStatus};
use forge_core::workflow::validate_task_transition;
use forge_store::audit::DurableAuditLog;
use forge_store::db::SqliteIndex;
use forge_store::project_dir::{ProjectPaths, init_project, load_project};
use forge_store::sync::rebuild_index;
use forge_yaml::{read_task_file, update_task_status_lossless, write_task_file};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(name = "forge")]
#[command(about = "Forge — Desktop-first extensible project management platform", long_about = None)]
#[command(version)]
struct Cli {
    #[arg(global = true, long, help = "Path to project root directory")]
    project: Option<PathBuf>,

    #[arg(global = true, long, value_enum, default_value_t = Format::Text, help = "Output format")]
    format: Format,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Text,
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new Forge project
    Init {
        #[arg(default_value = ".")]
        path: PathBuf,

        #[arg(long, default_value = "New Project")]
        name: String,

        #[arg(long, default_value = "A Forge project")]
        description: String,
    },

    /// Project level operations
    Project {
        #[command(subcommand)]
        cmd: ProjectCommands,
    },

    /// Task management
    Task {
        #[command(subcommand)]
        cmd: TaskCommands,
    },

    /// Rebuild the derived SQLite index from canonical YAML files
    RebuildIndex,
}

#[derive(Subcommand, Debug)]
enum ProjectCommands {
    /// Display project metadata
    Info,
    /// Validate all YAML files in project
    Validate,
}

#[derive(Subcommand, Debug)]
enum TaskCommands {
    /// List tasks in project
    List,

    /// Create a new task
    Create {
        #[arg(long)]
        title: String,

        #[arg(long, default_value = "todo")]
        status: String,

        #[arg(long, default_value = "medium")]
        priority: String,
    },

    /// Losslessly update task status preserving comments, indentation, and formatting
    Status {
        /// Task ID or short ID prefix
        id: String,

        /// New status (backlog, todo, in_progress, in_review, done, cancelled)
        status: String,
    },
}

fn resolve_project_root(cli_project: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = cli_project {
        ProjectPaths::find_root(p).map_err(|e| e.to_string())
    } else {
        let cwd = std::env::current_dir().map_err(|e| e.to_string())?;
        ProjectPaths::find_root(&cwd).map_err(|e| e.to_string())
    }
}

fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli) {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Commands::Init {
            path,
            name,
            description,
        } => {
            let project =
                init_project(&path, &name, &description, "User").map_err(|e| e.to_string())?;
            match cli.format {
                Format::Text => {
                    println!("Initialized empty Forge project in {}", path.display());
                    println!("Project ID: {}", project.id);
                    println!("Name:       {}", project.name);
                }
                Format::Json => {
                    let json = serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?;
                    println!("{json}");
                }
            }
            Ok(())
        }

        Commands::Project { cmd } => {
            let root = resolve_project_root(cli.project.as_deref())?;
            match cmd {
                ProjectCommands::Info => {
                    let project = load_project(&root).map_err(|e| e.to_string())?;
                    match cli.format {
                        Format::Text => {
                            println!("Project:     {}", project.name);
                            println!("ID:          {}", project.id);
                            println!("Status:      {:?}", project.status);
                            println!("Description: {}", project.description);
                            println!("Root:        {}", root.display());
                        }
                        Format::Json => {
                            let json = serde_json::to_string_pretty(&project)
                                .map_err(|e| e.to_string())?;
                            println!("{json}");
                        }
                    }
                    Ok(())
                }
                ProjectCommands::Validate => {
                    let stats = rebuild_index(&root).map_err(|e| e.to_string())?;
                    match cli.format {
                        Format::Text => {
                            println!("Project validation PASSED.");
                            println!(
                                "Indexed {} tasks, {} milestones, {} dependencies.",
                                stats.tasks_indexed,
                                stats.milestones_indexed,
                                stats.dependencies_indexed
                            );
                        }
                        Format::Json => {
                            let json =
                                serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?;
                            println!("{json}");
                        }
                    }
                    Ok(())
                }
            }
        }

        Commands::RebuildIndex => {
            let root = resolve_project_root(cli.project.as_deref())?;
            let stats = rebuild_index(&root).map_err(|e| e.to_string())?;
            match cli.format {
                Format::Text => {
                    println!("Successfully rebuilt SQLite index from canonical YAML files:");
                    println!("  Projects:     {}", stats.projects_indexed);
                    println!("  Tasks:        {}", stats.tasks_indexed);
                    println!("  Milestones:   {}", stats.milestones_indexed);
                    println!("  Dependencies: {}", stats.dependencies_indexed);
                }
                Format::Json => {
                    let json = serde_json::to_string_pretty(&stats).map_err(|e| e.to_string())?;
                    println!("{json}");
                }
            }
            Ok(())
        }

        Commands::Task { cmd } => {
            let root = resolve_project_root(cli.project.as_deref())?;
            let paths = ProjectPaths::new(&root);

            match cmd {
                TaskCommands::List => {
                    let index = SqliteIndex::open(paths.db_path()).map_err(|e| e.to_string())?;
                    let tasks = index.list_entities("Task").map_err(|e| e.to_string())?;

                    match cli.format {
                        Format::Text => {
                            if tasks.is_empty() {
                                println!("No tasks found.");
                            } else {
                                println!("{:<10} {:<14} {:<8} TITLE", "ID", "STATUS", "PRIORITY");
                                for t in &tasks {
                                    let short_id = if t.id.len() >= 8 { &t.id[..8] } else { &t.id };
                                    let prio = t.priority.as_deref().unwrap_or("-");
                                    println!(
                                        "{:<10} {:<14} {:<8} {}",
                                        short_id, t.status, prio, t.title
                                    );
                                }
                            }
                        }
                        Format::Json => {
                            let json =
                                serde_json::to_string_pretty(&tasks).map_err(|e| e.to_string())?;
                            println!("{json}");
                        }
                    }
                    Ok(())
                }

                TaskCommands::Create {
                    title,
                    status,
                    priority,
                } => {
                    let mut task = Task::new(&title);
                    task.status = TaskStatus::from_str(&status)?;
                    task.priority = match priority.to_lowercase().as_str() {
                        "critical" => Priority::Critical,
                        "high" => Priority::High,
                        "low" => Priority::Low,
                        _ => Priority::Medium,
                    };

                    let slug: String = title
                        .to_lowercase()
                        .chars()
                        .map(|c| if c.is_alphanumeric() { c } else { '-' })
                        .collect();
                    let file_name =
                        format!("{}-{}.yaml", slug.trim_matches('-'), task.id.short_id());
                    let file_path = paths.tasks_dir().join(&file_name);

                    write_task_file(&file_path, &task).map_err(|e| e.to_string())?;

                    // Update index
                    let index = SqliteIndex::open(paths.db_path()).map_err(|e| e.to_string())?;
                    let rel_path = format!("tasks/{file_name}");
                    index
                        .upsert_entity(
                            task.id,
                            "Task",
                            &task.title,
                            task.status.as_str(),
                            Some(priority.as_str()),
                            task.milestone_id,
                            task.parent_id,
                            &rel_path,
                            &task.updated_at.to_rfc3339(),
                        )
                        .map_err(|e| e.to_string())?;

                    // Record durable audit log outside SQLite
                    let audit = DurableAuditLog::new(paths.audit_log_path());
                    let entry = AuditEntry::new(
                        task.id,
                        "Task",
                        AuditAction::Created,
                        Attribution::human("CLI User"),
                    );
                    let _ = audit.append(&entry);

                    match cli.format {
                        Format::Text => {
                            println!(
                                "Created task {} in {}",
                                task.id.short_id(),
                                file_path.display()
                            );
                        }
                        Format::Json => {
                            let json =
                                serde_json::to_string_pretty(&task).map_err(|e| e.to_string())?;
                            println!("{json}");
                        }
                    }
                    Ok(())
                }

                TaskCommands::Status { id, status } => {
                    let target_status = TaskStatus::from_str(&status)?;
                    let index = SqliteIndex::open(paths.db_path()).map_err(|e| e.to_string())?;
                    let tasks = index.list_entities("Task").map_err(|e| e.to_string())?;

                    let matched = tasks
                        .into_iter()
                        .find(|t| t.id == id || t.id.starts_with(&id))
                        .ok_or_else(|| format!("Task with ID '{id}' not found"))?;

                    let full_path = root.join(&matched.file_path);
                    let current_task = read_task_file(&full_path).map_err(|e| e.to_string())?;

                    // Validate workflow transition
                    validate_task_transition(current_task.status, target_status)
                        .map_err(|e| e.to_string())?;

                    // Lossless edit on YAML file
                    update_task_status_lossless(&full_path, target_status)
                        .map_err(|e| e.to_string())?;

                    // Update SQLite index
                    index
                        .upsert_entity(
                            current_task.id,
                            "Task",
                            &current_task.title,
                            target_status.as_str(),
                            matched.priority.as_deref(),
                            current_task.milestone_id,
                            current_task.parent_id,
                            &matched.file_path,
                            &chrono::Utc::now().to_rfc3339(),
                        )
                        .map_err(|e| e.to_string())?;

                    // Record durable audit log outside SQLite
                    let audit = DurableAuditLog::new(paths.audit_log_path());
                    let mut entry = AuditEntry::new(
                        current_task.id,
                        "Task",
                        AuditAction::StatusChanged,
                        Attribution::human("CLI User"),
                    );
                    entry.old_value = Some(current_task.status.as_str().to_string());
                    entry.new_value = Some(target_status.as_str().to_string());
                    let _ = audit.append(&entry);

                    match cli.format {
                        Format::Text => {
                            println!(
                                "Updated task {} status from {} to {} (lossless edit verified)",
                                current_task.id.short_id(),
                                current_task.status.as_str(),
                                target_status.as_str()
                            );
                        }
                        Format::Json => {
                            let obj = serde_json::json!({
                                "id": current_task.id.to_string(),
                                "previous_status": current_task.status.as_str(),
                                "new_status": target_status.as_str(),
                                "file": matched.file_path,
                            });
                            println!("{}", serde_json::to_string_pretty(&obj).unwrap());
                        }
                    }
                    Ok(())
                }
            }
        }
    }
}
