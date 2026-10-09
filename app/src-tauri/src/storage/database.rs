use crate::domain::{AppError, BookSummary};
use rusqlite::{Connection, OpenFlags};
use std::{collections::HashSet, fs, path::PathBuf};

pub const SCHEMA_VERSION: i64 = 4;

pub struct Database {
    pub(crate) data_dir: PathBuf,
    pub(crate) db_path: PathBuf,
}

impl Database {
    pub fn initialize(data_dir: PathBuf) -> Result<Self, AppError> {
        fs::create_dir_all(&data_dir).map_err(|error| AppError::storage(error.to_string()))?;
        if data_dir.join("restore-in-progress").exists() {
            crate::services::backup::rollback_restore(&data_dir)?;
        }
        let db_path = data_dir.join("reader.sqlite3");
        let mut connection =
            Connection::open(&db_path).map_err(|error| AppError::storage(error.to_string()))?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| AppError::storage(error.to_string()))?;
        let current: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| AppError::migration(error.to_string()))?;
        if current > SCHEMA_VERSION {
            return Err(AppError::migration(format!(
                "数据库版本 {current} 高于应用支持的版本 {SCHEMA_VERSION}"
            )));
        }
        if current < 1 {
            let transaction = connection
                .transaction()
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .execute_batch(include_str!("../../migrations/0001_initial.sql"))
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .pragma_update(None, "user_version", 1)
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .commit()
                .map_err(|error| AppError::migration(error.to_string()))?;
        }
        if current < 2 {
            let transaction = connection
                .transaction()
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .execute_batch(include_str!("../../migrations/0002_reader.sql"))
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .pragma_update(None, "user_version", 2)
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .commit()
                .map_err(|error| AppError::migration(error.to_string()))?;
        }
        if current < 3 {
            let transaction = connection
                .transaction()
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .execute_batch(include_str!("../../migrations/0003_analysis.sql"))
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .pragma_update(None, "user_version", 3)
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction
                .commit()
                .map_err(|error| AppError::migration(error.to_string()))?;
        }
        if current < 4 {
            let transaction = connection.transaction()
                .map_err(|error| AppError::migration(error.to_string()))?;
            let columns: Vec<String> = {
                let mut statement = transaction.prepare("PRAGMA table_info(analysis_cache)")
                    .map_err(|error| AppError::migration(error.to_string()))?;
                let rows = statement.query_map([], |row| row.get(1))
                    .map_err(|error| AppError::migration(error.to_string()))?;
                rows.collect::<Result<_, _>>()
                    .map_err(|error| AppError::migration(error.to_string()))?
            };
            if !columns.iter().any(|column| column == "book_id") {
                // Early v3 caches had no book ownership. Their rows cannot be
                // safely assigned to a book; preserve them separately, never reuse them.
                if !columns.is_empty() {
                    transaction.execute_batch("ALTER TABLE analysis_cache RENAME TO analysis_cache_legacy_v3;")
                        .map_err(|error| AppError::migration(error.to_string()))?;
                }
                transaction.execute_batch(include_str!("../../migrations/0003_analysis.sql"))
                    .map_err(|error| AppError::migration(error.to_string()))?;
            }
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(|error| AppError::migration(error.to_string()))?;
            transaction.commit()
                .map_err(|error| AppError::migration(error.to_string()))?;
        }
        fs::create_dir_all(data_dir.join("books"))
            .map_err(|error| AppError::storage(error.to_string()))?;
        fs::create_dir_all(data_dir.join("cache"))
            .map_err(|error| AppError::storage(error.to_string()))?;
        fs::create_dir_all(data_dir.join("staging"))
            .map_err(|error| AppError::storage(error.to_string()))?;
        let database = Self { data_dir, db_path };
        database.cleanup_orphans()?;
        Ok(database)
    }

    pub fn connection(&self) -> Result<Connection, AppError> {
        let connection =
            Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_WRITE)
                .map_err(|error| AppError::storage(error.to_string()))?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(|error| AppError::storage(error.to_string()))?;
        Ok(connection)
    }

    pub fn managed_path(&self, relative_path: &str) -> Result<PathBuf, AppError> {
        let path = std::path::Path::new(relative_path);
        if path.components().count() != 1
            || !matches!(
                path.components().next(),
                Some(std::path::Component::Normal(_))
            )
        {
            return Err(AppError::new("FILE_UNREADABLE", "受管文件名无效", false));
        }
        Ok(self.data_dir.join("books").join(path))
    }

    fn cleanup_orphans(&self) -> Result<(), AppError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT relative_path FROM books")
            .map_err(|error| AppError::storage(error.to_string()))?;
        let referenced: HashSet<String> = statement
            .query_map([], |row| row.get(0))
            .map_err(|error| AppError::storage(error.to_string()))?
            .collect::<Result<_, _>>()
            .map_err(|error| AppError::storage(error.to_string()))?;
        for entry in fs::read_dir(self.data_dir.join("books"))
            .map_err(|error| AppError::storage(error.to_string()))?
        {
            let entry = entry.map_err(|error| AppError::storage(error.to_string()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            let is_managed = name.rsplit_once('.').is_some_and(|(id, ext)| {
                uuid::Uuid::parse_str(id).is_ok() && matches!(ext, "txt" | "pdf")
            });
            if is_managed && !referenced.contains(&name) {
                let _ = fs::remove_file(entry.path());
            }
        }
        for entry in fs::read_dir(self.data_dir.join("staging"))
            .map_err(|error| AppError::storage(error.to_string()))?
        {
            let entry = entry.map_err(|error| AppError::storage(error.to_string()))?;
            if entry.file_name().to_string_lossy().ends_with(".part") {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    pub fn list_books(&self) -> Result<Vec<BookSummary>, AppError> {
        let connection =
            Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .map_err(|error| AppError::storage(error.to_string()))?;
        let mut statement = connection.prepare(
            "SELECT b.id, b.title, b.author, b.format, b.relative_path, b.last_opened_at, b.page_count, b.paragraph_count, b.txt_encoding, p.anchor_json FROM books b LEFT JOIN reading_positions p ON p.book_id = b.id WHERE b.archived = 0 ORDER BY b.last_opened_at DESC, b.created_at DESC"
        ).map_err(|error| AppError::storage(error.to_string()))?;
        let rows = statement
            .query_map([], |row| {
                let relative_path: String = row.get(4)?;
                Ok(BookSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    author: row.get(2)?,
                    format: row.get(3)?,
                    available: self.data_dir.join("books").join(relative_path).is_file(),
                    last_opened_at: row.get(5)?,
                    page_count: row.get(6)?,
                    paragraph_count: row.get(7)?,
                    txt_encoding: row.get(8)?,
                    position: row
                        .get::<_, Option<String>>(9)?
                        .and_then(|value| serde_json::from_str(&value).ok()),
                })
            })
            .map_err(|error| AppError::storage(error.to_string()))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| AppError::storage(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_reopens_empty_library() {
        let root = tempfile::tempdir().unwrap();
        let database = Database::initialize(root.path().to_path_buf()).unwrap();
        assert!(database.list_books().unwrap().is_empty());
        assert!(root.path().join("books").is_dir());
        assert!(root.path().join("cache").is_dir());

        let reopened = Database::initialize(root.path().to_path_buf()).unwrap();
        assert!(reopened.list_books().unwrap().is_empty());
        let version: i64 = Connection::open(root.path().join("reader.sqlite3"))
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn newer_database_is_preserved() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("reader.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        drop(connection);

        let error = Database::initialize(root.path().to_path_buf())
            .err()
            .unwrap();
        assert_eq!(error.code, "DB_MIGRATION_FAILED");
        let version: i64 = Connection::open(path)
            .unwrap()
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION + 1);
    }

    #[test]
    fn migrates_existing_version_one_database() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("reader.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .unwrap();
        connection.pragma_update(None, "user_version", 1).unwrap();
        drop(connection);

        Database::initialize(root.path().to_path_buf()).unwrap();
        let connection = Connection::open(path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let count: i64 = connection
            .query_row("SELECT count(*) FROM book_preferences", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn migrates_existing_version_two_database() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("reader.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(include_str!("../../migrations/0001_initial.sql"))
            .unwrap();
        connection
            .execute_batch(include_str!("../../migrations/0002_reader.sql"))
            .unwrap();
        connection.pragma_update(None, "user_version", 2).unwrap();
        drop(connection);

        Database::initialize(root.path().to_path_buf()).unwrap();
        let connection = Connection::open(path).unwrap();
        let version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        let count: i64 = connection
            .query_row("SELECT count(*) FROM analysis_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
