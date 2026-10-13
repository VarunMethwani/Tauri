use crate::{Message, PermissionMode, Session};
use rusqlite::{params, Connection};
use std::{
    path::Path,
    str::FromStr,
    sync::{Arc, Mutex, MutexGuard},
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, HarnessError>;

#[derive(Debug, Error)]
pub enum HarnessError {
    #[error("SQLite error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(
        "database schema version {found} is newer than the supported version {supported}"
    )]
    UnsupportedSchemaVersion { found: i64, supported: i64 },
    #[error("session store lock was poisoned")]
    StoreLockPoisoned,
    #[error("database contains an unknown permission mode: {0}")]
    UnknownPermissionMode(String),
}

/// The latest schema understood by this crate.
///
/// Version zero is the schema written by older releases before schema
/// versioning was introduced. It has the same tables as version one, so its
/// migration is intentionally data-preserving and only records the version.
const CURRENT_SCHEMA_VERSION: i64 = 1;

const SCHEMA_V1: &str = "CREATE TABLE IF NOT EXISTS sessions (
                 id TEXT PRIMARY KEY NOT NULL,
                 title TEXT NOT NULL,
                 workspace TEXT NOT NULL,
                 mode TEXT NOT NULL CHECK (mode IN ('plan', 'ask', 'auto_edit', 'autonomous'))
             );
             CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 session_id TEXT NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS messages_session_id_idx
                 ON messages(session_id, id);";

/// SQLite-backed session and transcript storage.
///
/// The schema stores no provider configuration, credentials, or secrets.
#[derive(Clone)]
pub struct SessionStore {
    connection: Arc<Mutex<Connection>>,
}

impl SessionStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        migrate_schema(&mut connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn create_session(
        &self,
        workspace: &str,
        title: &str,
        mode: PermissionMode,
    ) -> Result<Session> {
        let connection = self.connection()?;
        let id: String = connection.query_row(
            "SELECT lower(hex(randomblob(16)))",
            [],
            |row| row.get(0),
        )?;
        connection.execute(
            "INSERT INTO sessions (id, title, workspace, mode) VALUES (?1, ?2, ?3, ?4)",
            params![id, title, workspace, mode.as_str()],
        )?;

        Ok(Session {
            id,
            title: title.to_owned(),
            workspace: workspace.to_owned(),
            mode,
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, title, workspace, mode FROM sessions ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        rows.map(|row| {
            let (id, title, workspace, raw_mode) = row?;
            let mode = PermissionMode::from_str(&raw_mode)
                .map_err(HarnessError::UnknownPermissionMode)?;
            Ok(Session {
                id,
                title,
                workspace,
                mode,
            })
        })
        .collect()
    }

    pub fn messages(&self, session_id: &str) -> Result<Vec<Message>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT role, content FROM messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;
        let messages = statement
            .query_map([session_id], |row| {
                Ok(Message {
                    role: row.get(0)?,
                    content: row.get(1)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn append_message(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO messages (session_id, role, content) VALUES (?1, ?2, ?3)",
            params![session_id, role, content],
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection
            .lock()
            .map_err(|_| HarnessError::StoreLockPoisoned)
    }
}

/// Applies all migrations needed to bring a database to the current schema.
///
/// The old, unversioned schema is treated as version zero. Every migration is
/// performed in one transaction so a failed open cannot leave a partially
/// upgraded database behind. `CREATE ... IF NOT EXISTS` also makes opening a
/// new database and reopening a current database idempotent.
fn migrate_schema(connection: &mut Connection) -> Result<()> {
    let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version > CURRENT_SCHEMA_VERSION {
        return Err(HarnessError::UnsupportedSchemaVersion {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }
    if version < 0 {
        return Err(HarnessError::UnsupportedSchemaVersion {
            found: version,
            supported: CURRENT_SCHEMA_VERSION,
        });
    }

    let transaction = connection.transaction()?;
    // Version one is currently the only concrete schema. Keeping the match
    // explicit makes future migrations additive and prevents silently
    // accepting an unhandled version.
    match version {
        0 | CURRENT_SCHEMA_VERSION => {
            transaction.execute_batch(SCHEMA_V1)?;
            if version == 0 {
                transaction.execute_batch("PRAGMA user_version = 1;")?;
            }
        }
        _ => unreachable!("schema version was checked above"),
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        path::PathBuf,
        process,
        sync::atomic::{AtomicU64, Ordering},
    };

    static NEXT_TEST_DB: AtomicU64 = AtomicU64::new(0);

    struct TestDatabase(PathBuf);

    impl TestDatabase {
        fn new() -> Self {
            let serial = NEXT_TEST_DB.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "harness-core-{}-{serial}.sqlite3",
                process::id()
            )))
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
        }
    }

    #[test]
    fn sessions_and_messages_resume_after_reopen() {
        let database = TestDatabase::new();
        let session = {
            let store = SessionStore::open(&database.0).unwrap();
            let session = store
                .create_session("/workspace/project", "First session", PermissionMode::Ask)
                .unwrap();
            store
                .append_message(&session.id, "user", "Keep this message")
                .unwrap();
            store
                .append_message(&session.id, "assistant", "It is persisted locally")
                .unwrap();
            session
        };

        let reopened = SessionStore::open(&database.0).unwrap();
        assert_eq!(reopened.list_sessions().unwrap(), vec![session.clone()]);
        assert_eq!(
            reopened.messages(&session.id).unwrap(),
            vec![
                Message {
                    role: "user".into(),
                    content: "Keep this message".into(),
                },
                Message {
                    role: "assistant".into(),
                    content: "It is persisted locally".into(),
                },
            ]
        );
        assert_eq!(schema_version(&database.0), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn migrates_unversioned_schema_without_losing_sessions_or_messages() {
        let database = TestDatabase::new();
        {
            // Databases created by the previous release have user_version =
            // 0 and already contain these tables.
            let connection = Connection::open(&database.0).unwrap();
            connection.execute_batch(SCHEMA_V1).unwrap();
            connection
                .execute(
                    "INSERT INTO sessions (id, title, workspace, mode)
                     VALUES (?1, ?2, ?3, ?4)",
                    params!["legacy-session", "Legacy", "/workspace/old", "plan"],
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO messages (session_id, role, content)
                     VALUES (?1, ?2, ?3)",
                    params!["legacy-session", "user", "Keep this transcript"],
                )
                .unwrap();
        }

        let store = SessionStore::open(&database.0).unwrap();
        assert_eq!(schema_version(&database.0), CURRENT_SCHEMA_VERSION);
        assert_eq!(
            store.list_sessions().unwrap(),
            vec![Session {
                id: "legacy-session".into(),
                title: "Legacy".into(),
                workspace: "/workspace/old".into(),
                mode: PermissionMode::Plan,
            }]
        );
        assert_eq!(
            store.messages("legacy-session").unwrap(),
            vec![Message {
                role: "user".into(),
                content: "Keep this transcript".into(),
            }]
        );
    }

    #[test]
    fn rejects_a_database_from_a_newer_schema() {
        let database = TestDatabase::new();
        {
            let connection = Connection::open(&database.0).unwrap();
            connection
                .execute_batch("PRAGMA user_version = 99;")
                .unwrap();
        }

        let error = match SessionStore::open(&database.0) {
            Err(error) => error,
            Ok(_) => panic!("a newer schema version must be rejected"),
        };
        assert!(matches!(
            error,
            HarnessError::UnsupportedSchemaVersion {
                found: 99,
                supported: CURRENT_SCHEMA_VERSION,
            }
        ));
        assert_eq!(schema_version(&database.0), 99);
    }

    #[test]
    fn messages_require_an_existing_session() {
        let store = SessionStore::open(":memory:").unwrap();
        let error = store
            .append_message("missing", "user", "orphan")
            .unwrap_err();
        assert!(matches!(error, HarnessError::Database(_)));
    }

    fn schema_version(path: &Path) -> i64 {
        let connection = Connection::open(path).unwrap();
        connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }
}

