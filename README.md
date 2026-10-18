# Forge

Forge is a desktop-first, extensible project-management application built in Rust, designed to manage projects from initial conception through planning, execution, delivery, and long-term maintenance.

This repository currently contains **Phase 0 (Foundation)**: the core domain types, the lossless YAML serialization engine, the rebuildable SQLite indexing store, and the command-line interface (`forge`).

---

## Architecture & Design Principles (Phase 0)

1. **Human-Readable Canonical Storage (YAML):**
   - Project specifications and entity records (`forge.yaml`, `tasks/*.yaml`, `milestones/*.yaml`) are stored as plain YAML files on disk.
   - YAML is the single source of truth.
   - Files can be inspected, committed to version control, and edited with any standard text editor.

2. **Lossless Editing:**
   - In-place scalar edits (such as updating a task's workflow status) preserve all user comments (block headers, inline comments, footer comments), blank lines, custom indentation, CRLF/LF line endings, and UTF-8 Byte Order Marks (BOM) byte-for-byte.
   - Uses Concrete Syntax Tree (CST) parsing via `glaucus-cst` with transparent BOM envelope handling and targeted source-span splicing.

3. **Rebuildable Derived SQLite Index:**
   - SQLite (`.forge/forge.db`) is used purely as an index and relational cache for fast lookups and dependency graph queries.
   - Uses `rusqlite` with WAL mode and explicit versioned migrations (`PRAGMA user_version`).
   - If `.forge/forge.db` is deleted or corrupted, `forge rebuild-index` completely reconstructs the index from the canonical YAML files with zero data loss.

4. **Durable Audit Log Outside SQLite:**
   - Audit records are stored in `.forge/audit.jsonl` outside the SQLite database.
   - Deleting or rebuilding the database index does not erase historical change logs.

5. **Single Canonical Storage of Relationships:**
   - Relationships are stored canonically once (e.g. `Task.depends_on` stores forward dependencies; `Task.milestone_id` stores milestone membership).
   - Reverse relationships (such as which tasks are blocked by a task, subtasks of a parent, or tasks in a milestone) are derived dynamically via in-memory graph models and SQLite relational queries.

---

## Workspace Structure

The project is structured as a Cargo workspace:

* [`crates/forge-core`](crates/forge-core): Domain entities (`Project`, `Task`, `Milestone`, `AuditEntry`), UUIDv7 stable `EntityId`, `RelationshipGraph` for derived reverse relationships, and workflow transition validation.
* [`crates/forge-yaml`](crates/forge-yaml): YAML engine. Uses `serde-saphyr` for typed deserialization and `LosslessYamlDoc` (wrapping `glaucus-cst`) for comment- and format-preserving in-place updates. Includes atomic file writes (`write_atomic`).
* [`crates/forge-store`](crates/forge-store): Project directory layout, durable JSONL audit logging, and `SqliteIndex` with versioned migrations and index rebuilders.
* [`crates/forge-cli`](crates/forge-cli): The `forge` command-line executable.

---

## Prerequisites

* Rust 1.85+ (tested on stable 1.99.1)
* Cargo package manager

---

## Building and Testing

Build the entire workspace:
```bash
cargo build --workspace
```

Run all unit and integration tests:
```bash
cargo test --workspace
```

Verify formatting and linting:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## CLI Usage (Phase 0)

You can run the CLI directly via `cargo run --bin forge --`:

### Initialize a New Project
```bash
forge init /path/to/my-project --name "Alpha" --description "Project Alpha"
```
Creates `forge.yaml`, `tasks/`, `milestones/`, and `.forge/` (with `.forge/forge.db` and `.forge/audit.jsonl`).

### View Project Info
```bash
forge --project /path/to/my-project project info
# Output as JSON:
forge --project /path/to/my-project --format json project info
```

### Create a Task
```bash
forge --project /path/to/my-project task create --title "Build Parser" --status todo --priority high
```
Creates `tasks/build-parser-<id>.yaml`, updates the SQLite index, and appends a record to `.forge/audit.jsonl`.

### List Tasks
```bash
forge --project /path/to/my-project task list
# Or in machine-readable JSON format:
forge --project /path/to/my-project --format json task list
```

### Update Task Status (Lossless In-Place Edit)
```bash
forge --project /path/to/my-project task status <task-id-or-prefix> in_progress
```
Updates `status` in the task's YAML file. Any custom comments, blank lines, unusual indentation, CRLF line endings, and UTF-8 BOM are preserved.

### Rebuild Index from Scratch
```bash
forge --project /path/to/my-project rebuild-index
```
Clears and reconstructs the SQLite index directly from all YAML files on disk.

---

## Current Scope & Limitations (Phase 0)

Phase 0 is the minimal, verified foundation. The following capabilities are planned for subsequent phases and are **not yet implemented**:

* **Graphical User Interface (GUI):** Desktop application UI (Tauri 2 / React) is scheduled for Phase 2.
* **External File-System Watcher:** Real-time background sync when external text editors save changes is scheduled for Phase 3.
* **Import / Export Formats:** JSON and XML import/export are scheduled for Phase 4 (YAML is currently the active format).
* **AI Provider Integrations:** AI agents, context assembly, and provider adapters are scheduled for Phase 5.
* **Multi-User Sync / Remote Collaboration:** Forge is desktop-first; shared synchronization relies on version control (e.g. Git) at this phase.
