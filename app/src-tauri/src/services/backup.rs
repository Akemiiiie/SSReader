use rusqlite::{Connection, MAIN_DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    domain::AppError,
    storage::{Database, SCHEMA_VERSION},
};

const MANIFEST_VERSION: u32 = 1;
const MAX_ARCHIVE_BYTES: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    path: String,
    byte_size: u64,
    sha256: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    format_version: u32,
    schema_version: i64,
    files: Vec<FileEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub book_count: usize,
    pub byte_size: u64,
    pub schema_version: i64,
}

fn failure(message: &str) -> AppError {
    AppError::new("BACKUP_CORRUPT", message, false)
}

fn hash_file(path: &Path) -> Result<(u64, String), AppError> {
    let mut file = File::open(path).map_err(|error| AppError::storage(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| AppError::storage(error.to_string()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((size, format!("{:x}", hasher.finalize())))
}

fn valid_entry(path: &str) -> bool {
    if path == "reader.sqlite3" {
        return true;
    }
    let Some(name) = path.strip_prefix("books/") else {
        return false;
    };
    !name.is_empty()
        && !name.contains(['/', '\\'])
        && !name.contains("..")
        && (name.ends_with(".txt") || name.ends_with(".pdf"))
}

pub fn create_backup(db: &Database, destination: &Path) -> Result<BackupInfo, AppError> {
    if destination.exists() {
        return Err(AppError::new(
            "FILE_EXISTS",
            "备份文件已存在，请选择其他文件名",
            false,
        ));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::new("INVALID_INPUT", "备份位置无效", false))?;
    let snapshot = tempfile::NamedTempFile::new_in(&db.data_dir)
        .map_err(|error| AppError::storage(error.to_string()))?;
    let connection = db.connection()?;
    connection
        .backup(MAIN_DB, snapshot.path(), None)
        .map_err(|error| AppError::storage(error.to_string()))?;
    drop(connection);
    let mut files = vec![("reader.sqlite3".to_owned(), snapshot.path().to_path_buf())];
    let connection =
        Connection::open(snapshot.path()).map_err(|error| AppError::storage(error.to_string()))?;
    let mut query = connection
        .prepare("SELECT relative_path FROM books")
        .map_err(|error| AppError::storage(error.to_string()))?;
    let paths = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::storage(error.to_string()))?;
    for item in paths {
        let relative = item.map_err(|error| AppError::storage(error.to_string()))?;
        let archive_path = format!("books/{relative}");
        if !valid_entry(&archive_path) {
            return Err(AppError::new("FILE_UNREADABLE", "书籍文件名无效", false));
        }
        let source = db.managed_path(&relative)?;
        if !source.is_file() {
            return Err(AppError::new(
                "FILE_MISSING",
                "有受管原文件缺失，无法制作完整备份",
                false,
            ));
        }
        files.push((archive_path, source));
    }
    let mut manifest = Manifest {
        format_version: MANIFEST_VERSION,
        schema_version: SCHEMA_VERSION,
        files: Vec::new(),
    };
    for (name, source) in &files {
        let (byte_size, sha256) = hash_file(source)?;
        manifest.files.push(FileEntry {
            path: name.clone(),
            byte_size,
            sha256,
        });
    }
    let total = manifest.files.iter().map(|item| item.byte_size).sum();
    if total > MAX_ARCHIVE_BYTES {
        return Err(AppError::new(
            "FILE_TOO_LARGE",
            "备份内容超过 4 GiB 上限",
            false,
        ));
    }
    let temp = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| AppError::storage(error.to_string()))?;
    let mut zip = ZipWriter::new(
        temp.reopen()
            .map_err(|error| AppError::storage(error.to_string()))?,
    );
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)
        .map_err(|error| AppError::storage(error.to_string()))?;
    zip.write_all(
        &serde_json::to_vec(&manifest).map_err(|error| AppError::storage(error.to_string()))?,
    )
    .map_err(|error| AppError::storage(error.to_string()))?;
    for (name, source) in files {
        zip.start_file(name, options)
            .map_err(|error| AppError::storage(error.to_string()))?;
        std::io::copy(
            &mut File::open(source).map_err(|error| AppError::storage(error.to_string()))?,
            &mut zip,
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    }
    let file = zip
        .finish()
        .map_err(|error| AppError::storage(error.to_string()))?;
    file.sync_all()
        .map_err(|error| AppError::storage(error.to_string()))?;
    drop(file);
    temp.persist_noclobber(destination)
        .map_err(|error| AppError::storage(error.error.to_string()))?;
    Ok(BackupInfo {
        book_count: manifest.files.len().saturating_sub(1),
        byte_size: total,
        schema_version: SCHEMA_VERSION,
    })
}

fn read_manifest(archive: &mut ZipArchive<File>) -> Result<Manifest, AppError> {
    let mut entry = archive
        .by_name("manifest.json")
        .map_err(|_| failure("备份缺少清单"))?;
    if entry.size() > 1024 * 1024 {
        return Err(failure("备份清单过大"));
    }
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .map_err(|_| failure("备份清单不可读"))?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|_| failure("备份清单格式无效"))?;
    if manifest.format_version != MANIFEST_VERSION || manifest.schema_version > SCHEMA_VERSION {
        return Err(AppError::new(
            "BACKUP_INCOMPATIBLE",
            "备份版本高于当前应用支持的版本",
            false,
        ));
    }
    if manifest.files.is_empty() || manifest.files[0].path != "reader.sqlite3" {
        return Err(failure("备份缺少数据库"));
    }
    let mut names = std::collections::HashSet::new();
    let total = manifest
        .files
        .iter()
        .try_fold(0u64, |total, item| total.checked_add(item.byte_size))
        .ok_or_else(|| failure("备份文件清单大小无效"))?;
    if total > MAX_ARCHIVE_BYTES
        || manifest
            .files
            .iter()
            .any(|item| !valid_entry(&item.path) || !names.insert(item.path.clone()))
    {
        return Err(failure("备份文件清单无效"));
    }
    Ok(manifest)
}

pub fn inspect_backup(source: &Path) -> Result<BackupInfo, AppError> {
    let mut archive = ZipArchive::new(File::open(source).map_err(|_| failure("备份文件无法打开"))?)
        .map_err(|_| failure("不是有效的备份归档"))?;
    let manifest = read_manifest(&mut archive)?;
    for item in &manifest.files {
        let mut entry = archive
            .by_name(&item.path)
            .map_err(|_| failure("备份缺少清单中的文件"))?;
        if entry.size() != item.byte_size {
            return Err(failure("备份文件大小与清单不一致"));
        }
        let mut hasher = Sha256::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 1024 * 1024];
        loop {
            let count = entry
                .read(&mut buffer)
                .map_err(|_| failure("备份数据读取失败"))?;
            if count == 0 {
                break;
            }
            size += count as u64;
            if size > item.byte_size {
                return Err(failure("备份文件长度错误"));
            }
            hasher.update(&buffer[..count]);
        }
        if size != item.byte_size || format!("{:x}", hasher.finalize()) != item.sha256 {
            return Err(failure("备份文件校验失败，现有书架未更改"));
        }
    }
    Ok(BackupInfo {
        book_count: manifest.files.len() - 1,
        byte_size: manifest.files.iter().map(|item| item.byte_size).sum(),
        schema_version: manifest.schema_version,
    })
}

pub fn restore_backup(data_dir: &Path, source: &Path) -> Result<BackupInfo, AppError> {
    let mut archive = ZipArchive::new(File::open(source).map_err(|_| failure("备份文件无法打开"))?)
        .map_err(|_| failure("不是有效的备份归档"))?;
    let manifest = read_manifest(&mut archive)?;
    let stage =
        tempfile::tempdir_in(data_dir).map_err(|error| AppError::storage(error.to_string()))?;
    fs::create_dir(stage.path().join("books"))
        .map_err(|error| AppError::storage(error.to_string()))?;
    for item in &manifest.files {
        let entry = archive
            .by_name(&item.path)
            .map_err(|_| failure("备份缺少清单中的文件"))?;
        if entry.size() != item.byte_size {
            return Err(failure("备份文件大小与清单不一致"));
        }
        let destination = stage.path().join(&item.path);
        let mut target =
            File::create(&destination).map_err(|error| AppError::storage(error.to_string()))?;
        let copied = std::io::copy(&mut entry.take(item.byte_size + 1), &mut target)
            .map_err(|_| failure("备份数据读取失败"))?;
        if copied != item.byte_size {
            return Err(failure("备份文件长度错误"));
        }
        let (_, hash) = hash_file(&destination)?;
        if hash != item.sha256 {
            return Err(failure("备份文件校验失败，现有书架未更改"));
        }
    }
    let staged_db = stage.path().join("reader.sqlite3");
    let connection =
        Connection::open_with_flags(&staged_db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|_| failure("备份数据库不可读"))?;
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|_| failure("备份数据库版本不可读"))?;
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|_| failure("备份数据库损坏"))?;
    if version != manifest.schema_version || integrity != "ok" {
        return Err(failure("备份数据库校验失败"));
    }
    if version < 1 || version > SCHEMA_VERSION {
        return Err(AppError::new(
            "BACKUP_INCOMPATIBLE",
            "备份数据库版本与当前应用不兼容",
            false,
        ));
    }
    drop(connection);
    if version < SCHEMA_VERSION {
        Database::initialize(stage.path().to_path_buf())?;
    }

    let marker = data_dir.join("restore-in-progress");
    let rollback = data_dir.join("restore-rollback");
    if marker.exists() || rollback.exists() {
        return Err(AppError::new(
            "RESTORE_PENDING",
            "存在未完成的恢复，请重新启动应用",
            false,
        ));
    }
    fs::create_dir(&rollback).map_err(|error| AppError::storage(error.to_string()))?;
    fs::write(&marker, b"rollback").map_err(|error| AppError::storage(error.to_string()))?;
    let switched = (|| -> std::io::Result<()> {
        if data_dir.join("reader.sqlite3").exists() {
            fs::rename(
                data_dir.join("reader.sqlite3"),
                rollback.join("reader.sqlite3"),
            )?;
        }
        if data_dir.join("books").exists() {
            fs::rename(data_dir.join("books"), rollback.join("books"))?;
        }
        fs::rename(
            stage.path().join("reader.sqlite3"),
            data_dir.join("reader.sqlite3"),
        )?;
        fs::rename(stage.path().join("books"), data_dir.join("books"))?;
        Ok(())
    })();
    if let Err(error) = switched {
        rollback_restore(data_dir)?;
        return Err(AppError::storage(error.to_string()));
    }
    fs::remove_file(marker).map_err(|error| AppError::storage(error.to_string()))?;
    fs::remove_dir_all(rollback).map_err(|error| AppError::storage(error.to_string()))?;
    Ok(BackupInfo {
        book_count: manifest.files.len() - 1,
        byte_size: manifest.files.iter().map(|item| item.byte_size).sum(),
        schema_version: manifest.schema_version,
    })
}

pub fn rollback_restore(data_dir: &Path) -> Result<(), AppError> {
    let rollback = data_dir.join("restore-rollback");
    let old_db = rollback.join("reader.sqlite3");
    let old_books = rollback.join("books");
    if old_db.exists() {
        let current = data_dir.join("reader.sqlite3");
        if current.exists() {
            fs::remove_file(current).map_err(|error| AppError::storage(error.to_string()))?;
        }
        fs::rename(old_db, data_dir.join("reader.sqlite3"))
            .map_err(|error| AppError::storage(error.to_string()))?;
    }
    if old_books.exists() {
        let current = data_dir.join("books");
        if current.exists() {
            fs::remove_dir_all(current).map_err(|error| AppError::storage(error.to_string()))?;
        }
        fs::rename(old_books, data_dir.join("books"))
            .map_err(|error| AppError::storage(error.to_string()))?;
    }
    let marker = data_dir.join("restore-in-progress");
    if marker.exists() {
        fs::remove_file(marker).map_err(|error| AppError::storage(error.to_string()))?;
    }
    if rollback.exists() {
        fs::remove_dir(rollback).map_err(|error| AppError::storage(error.to_string()))?;
    }
    Ok(())
}
