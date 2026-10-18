use forge_core::error::{ForgeError, Result};
use forge_core::id::EntityId;
use rusqlite::{Connection, params};
use std::path::Path;

const CURRENT_DB_VERSION: i32 = 1;

/// SQLite index manager using rusqlite with explicit versioned migrations.
/// Note: SQLite is strictly a derived index/cache that can be deleted and rebuilt.
pub struct SqliteIndex {
    conn: Connection,
}

impl SqliteIndex {
    /// Opens or creates an SQLite index database at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path.as_ref())
            .map_err(|e| ForgeError::Store(format!("Failed to open SQLite index: {e}")))?;
        let mut index = Self { conn };
        index.init_pragmas()?;
        index.run_migrations()?;
        Ok(index)
    }

    /// Creates an in-memory database (useful for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| ForgeError::Store(format!("Failed to open in-memory SQLite: {e}")))?;
        let mut index = Self { conn };
        index.init_pragmas()?;
        index.run_migrations()?;
        Ok(index)
    }

    fn init_pragmas(&self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                PRAGMA foreign_keys = ON;
                "#,
            )
            .map_err(|e| ForgeError::Store(format!("Failed to set SQLite pragmas: {e}")))?;
        Ok(())
    }

    /// Runs explicit versioned migrations.
    pub fn run_migrations(&mut self) -> Result<()> {
        let mut user_version: i32 = self
            .conn
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .map_err(|e| ForgeError::Store(format!("Failed to read user_version: {e}")))?;

        if user_version < 1 {
            self.migrate_to_v1()?;
            user_version = 1;
        }

        debug_assert_eq!(user_version, CURRENT_DB_VERSION);
        Ok(())
    }

    fn migrate_to_v1(&mut self) -> Result<()> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| ForgeError::Store(format!("Failed to start transaction: {e}")))?;

        tx.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS entities (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL,
                priority TEXT,
                milestone_id TEXT,
                parent_id TEXT,
                file_path TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS dependencies (
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                PRIMARY KEY (source_id, target_id)
            );

            CREATE INDEX IF NOT EXISTS idx_entities_kind ON entities(kind);
            CREATE INDEX IF NOT EXISTS idx_entities_status ON entities(status);
            CREATE INDEX IF NOT EXISTS idx_entities_milestone ON entities(milestone_id);
            CREATE INDEX IF NOT EXISTS idx_entities_parent ON entities(parent_id);
            CREATE INDEX IF NOT EXISTS idx_deps_target ON dependencies(target_id);

            INSERT OR REPLACE INTO schema_migrations (version, applied_at)
            VALUES (1, datetime('now'));

            PRAGMA user_version = 1;
            "#,
        )
        .map_err(|e| ForgeError::Store(format!("Failed to apply migration v1: {e}")))?;

        tx.commit()
            .map_err(|e| ForgeError::Store(format!("Failed to commit migration v1: {e}")))?;
        Ok(())
    }

    /// Clears all tables in the index so that it can be cleanly rebuilt from YAML.
    pub fn clear_all(&mut self) -> Result<()> {
        self.conn
            .execute_batch(
                r#"
                DELETE FROM dependencies;
                DELETE FROM entities;
                "#,
            )
            .map_err(|e| ForgeError::Store(format!("Failed to clear index: {e}")))?;
        Ok(())
    }

    /// Upserts an entity in the index.
    #[allow(clippy::too_many_arguments)]
    pub fn upsert_entity(
        &self,
        id: EntityId,
        kind: &str,
        title: &str,
        status: &str,
        priority: Option<&str>,
        milestone_id: Option<EntityId>,
        parent_id: Option<EntityId>,
        file_path: &str,
        updated_at: &str,
    ) -> Result<()> {
        self.conn
            .execute(
                r#"
                INSERT INTO entities (id, kind, title, status, priority, milestone_id, parent_id, file_path, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                ON CONFLICT(id) DO UPDATE SET
                    kind = excluded.kind,
                    title = excluded.title,
                    status = excluded.status,
                    priority = excluded.priority,
                    milestone_id = excluded.milestone_id,
                    parent_id = excluded.parent_id,
                    file_path = excluded.file_path,
                    updated_at = excluded.updated_at;
                "#,
                params![
                    id.to_string(),
                    kind,
                    title,
                    status,
                    priority,
                    milestone_id.map(|m| m.to_string()),
                    parent_id.map(|p| p.to_string()),
                    file_path,
                    updated_at,
                ],
            )
            .map_err(|e| ForgeError::Store(format!("Failed to upsert entity: {e}")))?;
        Ok(())
    }

    /// Replaces the dependencies for an entity.
    pub fn set_dependencies(&self, source_id: EntityId, targets: &[EntityId]) -> Result<()> {
        let source_str = source_id.to_string();
        self.conn
            .execute(
                "DELETE FROM dependencies WHERE source_id = ?1;",
                params![source_str],
            )
            .map_err(|e| ForgeError::Store(format!("Failed to clear dependencies: {e}")))?;

        for target in targets {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO dependencies (source_id, target_id) VALUES (?1, ?2);",
                    params![source_str, target.to_string()],
                )
                .map_err(|e| ForgeError::Store(format!("Failed to insert dependency: {e}")))?;
        }
        Ok(())
    }

    /// Queries entities by kind.
    pub fn list_entities(&self, kind: &str) -> Result<Vec<IndexedEntity>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, kind, title, status, priority, milestone_id, parent_id, file_path, updated_at
                 FROM entities WHERE kind = ?1 ORDER BY title ASC;",
            )
            .map_err(|e| ForgeError::Store(format!("Failed to prepare query: {e}")))?;

        let rows = stmt
            .query_map(params![kind], |row| {
                Ok(IndexedEntity {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    priority: row.get(4)?,
                    milestone_id: row.get(5)?,
                    parent_id: row.get(6)?,
                    file_path: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })
            .map_err(|e| ForgeError::Store(format!("Query failed: {e}")))?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| ForgeError::Store(format!("Row error: {e}")))?);
        }
        Ok(result)
    }

    /// Derived: Finds tasks that are blocked by `task_id` (reverse dependency).
    pub fn get_blocked_tasks(&self, task_id: EntityId) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT source_id FROM dependencies WHERE target_id = ?1;")
            .map_err(|e| ForgeError::Store(format!("Prepare failed: {e}")))?;

        let rows = stmt
            .query_map(params![task_id.to_string()], |row| row.get(0))
            .map_err(|e| ForgeError::Store(format!("Query failed: {e}")))?;

        let mut res = Vec::new();
        for r in rows {
            res.push(r.map_err(|e| ForgeError::Store(format!("Row error: {e}")))?);
        }
        Ok(res)
    }

    pub fn inner(&self) -> &Connection {
        &self.conn
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexedEntity {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
    pub milestone_id: Option<String>,
    pub parent_id: Option<String>,
    pub file_path: String,
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sqlite_migrations_and_indexing() {
        let index = SqliteIndex::open_in_memory().unwrap();

        let t1_id = EntityId::new();
        let t2_id = EntityId::new();

        index
            .upsert_entity(
                t1_id,
                "Task",
                "First Task",
                "todo",
                Some("high"),
                None,
                None,
                "tasks/first-task.yaml",
                "2026-10-18T14:00:00Z",
            )
            .unwrap();

        index
            .upsert_entity(
                t2_id,
                "Task",
                "Second Task",
                "in_progress",
                Some("medium"),
                None,
                None,
                "tasks/second-task.yaml",
                "2026-10-18T14:00:00Z",
            )
            .unwrap();

        // t2 depends on t1
        index.set_dependencies(t2_id, &[t1_id]).unwrap();

        let tasks = index.list_entities("Task").unwrap();
        assert_eq!(tasks.len(), 2);

        // Derived reverse relationship: t1 blocks t2
        let blocked_by_t1 = index.get_blocked_tasks(t1_id).unwrap();
        assert_eq!(blocked_by_t1, vec![t2_id.to_string()]);
    }
}
