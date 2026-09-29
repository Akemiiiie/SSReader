mod database;

use crate::domain::AppError;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

pub use database::{Database, SCHEMA_VERSION};

pub struct ImportJob {
    pub path: PathBuf,
    pub cancelled: Arc<AtomicBool>,
}

pub struct AppState {
    pub data_dir: Option<PathBuf>,
    pub database: Option<Database>,
    pub startup_error: Option<AppError>,
    pub jobs: Mutex<HashMap<String, ImportJob>>,
    pub operation: Mutex<()>,
    pub backup_source: Mutex<Option<PathBuf>>,
    pub shortcut_error: Mutex<Option<String>>,
}

impl AppState {
    pub fn initialize(data_dir: PathBuf) -> Self {
        match Database::initialize(data_dir.clone()) {
            Ok(database) => Self {
                data_dir: Some(data_dir),
                database: Some(database),
                startup_error: None,
                jobs: Mutex::new(HashMap::new()),
                operation: Mutex::new(()),
                backup_source: Mutex::new(None),
                shortcut_error: Mutex::new(None),
            },
            Err(error) => Self {
                data_dir: Some(data_dir),
                database: None,
                startup_error: Some(error),
                jobs: Mutex::new(HashMap::new()),
                operation: Mutex::new(()),
                backup_source: Mutex::new(None),
                shortcut_error: Mutex::new(None),
            },
        }
    }

    pub fn failed(error: AppError) -> Self {
        Self {
            data_dir: None,
            database: None,
            startup_error: Some(error),
            jobs: Mutex::new(HashMap::new()),
            operation: Mutex::new(()),
            backup_source: Mutex::new(None),
            shortcut_error: Mutex::new(None),
        }
    }

    pub fn database(&self) -> Result<&Database, AppError> {
        self.database.as_ref().ok_or_else(|| {
            self.startup_error
                .clone()
                .unwrap_or_else(|| AppError::storage("数据库未初始化".into()))
        })
    }
}
