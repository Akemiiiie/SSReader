use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

use crate::{
    domain::{AppError, BookSummary, ImportResult},
    storage::Database,
};

const MAX_TXT_BYTES: u64 = 32 * 1024 * 1024;
const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxtPreview {
    pub encoding: String,
    pub text: String,
    pub uncertain: bool,
}

pub fn preview_txt(source: &Path, preferred: Option<&str>) -> Result<TxtPreview, AppError> {
    let metadata = fs::metadata(source)
        .map_err(|_| AppError::new("FILE_UNREADABLE", "所选文件无法读取", true))?;
    if metadata.len() > MAX_TXT_BYTES {
        return Err(AppError::new(
            "FILE_TOO_LARGE",
            "TXT 超过本版本的 32 MiB 上限",
            false,
        ));
    }
    let bytes =
        fs::read(source).map_err(|_| AppError::new("FILE_UNREADABLE", "所选文件无法读取", true))?;
    let uncertain = preferred.is_none()
        && !bytes.starts_with(&[0xEF, 0xBB, 0xBF])
        && !bytes.starts_with(&[0xFF, 0xFE])
        && !bytes.starts_with(&[0xFE, 0xFF])
        && std::str::from_utf8(&bytes).is_err();
    let (text, encoding) = decode_txt(&bytes, preferred)?;
    Ok(TxtPreview {
        encoding: encoding.to_owned(),
        text: text.chars().take(240).collect(),
        uncertain,
    })
}

pub fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}

pub fn read_book(db: &Database, id: &str) -> Result<BookSummary, AppError> {
    db.list_books()?
        .into_iter()
        .find(|book| book.id == id)
        .ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "书籍不存在或已归档", false))
}

pub fn book_file(db: &Database, id: &str) -> Result<(PathBuf, String), AppError> {
    let connection = db.connection()?;
    let item: Option<(String, String)> = connection
        .query_row(
            "SELECT relative_path, format FROM books WHERE id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| AppError::storage(error.to_string()))?;
    let (relative, format) =
        item.ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "书籍不存在", false))?;
    let path = db.managed_path(&relative)?;
    if !path.is_file() {
        return Err(AppError::new(
            "FILE_MISSING",
            "受管原文件缺失，可从备份恢复或删除此书",
            false,
        ));
    }
    Ok((path, format))
}

pub fn decode_txt(
    bytes: &[u8],
    preferred: Option<&str>,
) -> Result<(String, &'static str), AppError> {
    let chosen = match preferred {
        Some("utf-8") => "utf-8",
        Some("utf-16le") => "utf-16le",
        Some("utf-16be") => "utf-16be",
        Some("gb18030") => "gb18030",
        Some(_) => return Err(AppError::new("ENCODING_UNKNOWN", "不支持的文本编码", false)),
        None if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) => "utf-8",
        None if bytes.starts_with(&[0xFF, 0xFE]) => "utf-16le",
        None if bytes.starts_with(&[0xFE, 0xFF]) => "utf-16be",
        None if std::str::from_utf8(bytes).is_ok() => "utf-8",
        None => "gb18030",
    };
    let body = match chosen {
        "utf-8" if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) => &bytes[3..],
        "utf-16le" if bytes.starts_with(&[0xFF, 0xFE]) => &bytes[2..],
        "utf-16be" if bytes.starts_with(&[0xFE, 0xFF]) => &bytes[2..],
        _ => bytes,
    };
    let text = match chosen {
        "utf-8" => std::str::from_utf8(body)
            .map(|value| value.to_owned())
            .map_err(|_| AppError::new("ENCODING_UNKNOWN", "UTF-8 解码失败，请改选编码", false))?,
        _ => {
            let codec = match chosen {
                "utf-16le" => UTF_16LE,
                "utf-16be" => UTF_16BE,
                _ => GB18030,
            };
            let (value, errors) = codec.decode_without_bom_handling(body);
            if errors {
                return Err(AppError::new(
                    "ENCODING_UNKNOWN",
                    "文本包含无法按当前编码解读的字节",
                    false,
                ));
            }
            value.into_owned()
        }
    };
    if text.chars().any(|ch| ch == '\0') {
        return Err(AppError::new(
            "UNSUPPORTED_FORMAT",
            "文件包含二进制内容，不能作为 TXT 阅读",
            false,
        ));
    }
    Ok((text.replace("\r\n", "\n").replace('\r', "\n"), chosen))
}

pub fn import_book(
    db: &Database,
    source: &Path,
    cancelled: &Arc<AtomicBool>,
) -> Result<ImportResult, AppError> {
    import_book_with_encoding(db, source, cancelled, None)
}

pub fn import_book_with_encoding(
    db: &Database,
    source: &Path,
    cancelled: &Arc<AtomicBool>,
    preferred_encoding: Option<&str>,
) -> Result<ImportResult, AppError> {
    let metadata = fs::metadata(source)
        .map_err(|_| AppError::new("FILE_UNREADABLE", "所选文件无法读取", true))?;
    if !metadata.is_file() {
        return Err(AppError::new("UNSUPPORTED_FORMAT", "请选择普通文件", false));
    }
    let extension = source
        .extension()
        .and_then(|part| part.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension == "pdf" && preferred_encoding.is_some() {
        return Err(AppError::new(
            "INVALID_INPUT",
            "PDF 不使用文本编码设置",
            false,
        ));
    }
    let limit = match extension.as_str() {
        "txt" => MAX_TXT_BYTES,
        "pdf" => MAX_PDF_BYTES,
        _ => {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                "只支持 TXT 和 PDF 文件",
                false,
            ))
        }
    };
    if metadata.len() > limit {
        return Err(AppError::new(
            "FILE_TOO_LARGE",
            format!("文件超过本版本的 {} MiB 上限", limit / 1024 / 1024),
            false,
        ));
    }
    let staging = db.data_dir.join("staging");
    fs::create_dir_all(&staging).map_err(|error| AppError::storage(error.to_string()))?;
    let temp_path = staging.join(format!("{}.part", Uuid::new_v4()));
    let result = import_from_temp(
        db,
        source,
        &temp_path,
        &extension,
        limit,
        cancelled,
        preferred_encoding,
    );
    let _ = fs::remove_file(&temp_path);
    result
}

fn import_from_temp(
    db: &Database,
    source: &Path,
    temp_path: &Path,
    extension: &str,
    limit: u64,
    cancelled: &Arc<AtomicBool>,
    preferred_encoding: Option<&str>,
) -> Result<ImportResult, AppError> {
    let mut source_file = File::open(source)
        .map_err(|_| AppError::new("FILE_UNREADABLE", "所选文件无法读取", true))?;
    let mut target =
        File::create(temp_path).map_err(|error| AppError::storage(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::new("IMPORT_CANCELLED", "导入已取消", true));
        }
        let count = source_file
            .read(&mut buffer)
            .map_err(|_| AppError::new("FILE_UNREADABLE", "读取所选文件失败", true))?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > limit {
            return Err(AppError::new(
                "FILE_TOO_LARGE",
                "文件超过本版本大小上限",
                false,
            ));
        }
        hasher.update(&buffer[..count]);
        target
            .write_all(&buffer[..count])
            .map_err(|error| AppError::storage(error.to_string()))?;
    }
    target
        .sync_all()
        .map_err(|error| AppError::storage(error.to_string()))?;
    drop(target);
    if size == 0 {
        return Err(AppError::new("UNSUPPORTED_FORMAT", "不能导入空文件", false));
    }
    let hash = format!("{:x}", hasher.finalize());
    let mut connection = db.connection()?;
    let duplicate: Option<String> = connection
        .query_row("SELECT id FROM books WHERE sha256 = ?1", [&hash], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| AppError::storage(error.to_string()))?;
    if let Some(id) = duplicate {
        connection
            .execute("UPDATE books SET archived=0 WHERE id=?1", [&id])
            .map_err(|error| AppError::storage(error.to_string()))?;
        let book = read_book(db, &id)?;
        return Ok(ImportResult {
            book,
            duplicate: true,
        });
    }
    let mut prefix = [0u8; 5];
    let count = File::open(temp_path)
        .and_then(|mut file| file.read(&mut prefix))
        .map_err(|error| AppError::storage(error.to_string()))?;
    let (format, encoding, page_count, paragraph_count) = if extension == "pdf" {
        if count < 5 || &prefix != b"%PDF-" {
            return Err(AppError::new(
                "PDF_DAMAGED",
                "文件没有有效的 PDF 标识",
                false,
            ));
        }
        let doc = lopdf::Document::load(temp_path)
            .map_err(|_| AppError::new("PDF_DAMAGED", "PDF 结构损坏，无法导入", false))?;
        if doc.is_encrypted() {
            return Err(AppError::new(
                "PDF_PASSWORD_REQUIRED",
                "加密 PDF 暂不支持，请先解除密码保护",
                false,
            ));
        }
        let pages = doc.get_pages().len() as i64;
        if pages == 0 {
            return Err(AppError::new("PDF_DAMAGED", "PDF 没有可阅读的页面", false));
        }
        ("pdf", None, Some(pages), None)
    } else {
        if count >= 5 && &prefix == b"%PDF-" {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                "文件内容是 PDF，请使用 .pdf 扩展名",
                false,
            ));
        }
        let bytes = fs::read(temp_path).map_err(|error| AppError::storage(error.to_string()))?;
        let (text, encoding) = decode_txt(&bytes, preferred_encoding)?;
        (
            "txt",
            Some(encoding),
            None,
            Some(text.split('\n').count() as i64),
        )
    };
    if cancelled.load(Ordering::Relaxed) {
        return Err(AppError::new("IMPORT_CANCELLED", "导入已取消", true));
    }
    let id = Uuid::new_v4().to_string();
    let relative = format!("{id}.{format}");
    let managed = db.managed_path(&relative)?;
    fs::rename(temp_path, &managed).map_err(|error| AppError::storage(error.to_string()))?;
    let title = source
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or("未命名书籍")
        .trim();
    let transaction = connection
        .transaction()
        .map_err(|error| AppError::storage(error.to_string()))?;
    let inserted = transaction.execute(
        "INSERT INTO books (id,title,format,relative_path,sha256,byte_size,created_at,txt_encoding,page_count,paragraph_count) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![id, title, format, relative, hash, size as i64, now(), encoding, page_count, paragraph_count]
    ).and_then(|_| transaction.commit());
    if let Err(error) = inserted {
        let _ = fs::remove_file(&managed);
        return Err(AppError::storage(error.to_string()));
    }
    Ok(ImportResult {
        book: read_book(db, &id)?,
        duplicate: false,
    })
}

pub fn update_book(
    db: &Database,
    id: &str,
    title: String,
    author: Option<String>,
) -> Result<BookSummary, AppError> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > 200 {
        return Err(AppError::new(
            "INVALID_INPUT",
            "书名须为 1–200 个字符",
            false,
        ));
    }
    let author = author
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let changed = db
        .connection()?
        .execute(
            "UPDATE books SET title=?1, author=?2 WHERE id=?3",
            params![title, author, id],
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    if changed == 0 {
        return Err(AppError::new("BOOK_NOT_FOUND", "书籍不存在", false));
    }
    read_book(db, id)
}

pub fn archive_book(db: &Database, id: &str) -> Result<(), AppError> {
    let changed = db
        .connection()?
        .execute("UPDATE books SET archived=1 WHERE id=?1", [id])
        .map_err(|error| AppError::storage(error.to_string()))?;
    if changed == 0 {
        return Err(AppError::new("BOOK_NOT_FOUND", "书籍不存在", false));
    }
    Ok(())
}

pub fn delete_book(db: &Database, id: &str) -> Result<(), AppError> {
    let relative: String = db
        .connection()?
        .query_row("SELECT relative_path FROM books WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| AppError::storage(error.to_string()))?
        .ok_or_else(|| AppError::new("BOOK_NOT_FOUND", "书籍不存在", false))?;
    let path = db.managed_path(&relative)?;
    let mut connection = db.connection()?;
    let transaction = connection
        .transaction()
        .map_err(|error| AppError::storage(error.to_string()))?;
    transaction
        .execute("DELETE FROM books WHERE id=?1", [id])
        .map_err(|error| AppError::storage(error.to_string()))?;
    transaction
        .commit()
        .map_err(|error| AppError::storage(error.to_string()))?;
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

pub fn export_book(db: &Database, id: &str, destination: &Path) -> Result<(), AppError> {
    let (source, _) = book_file(db, id)?;
    if destination.exists() {
        return Err(AppError::new(
            "FILE_EXISTS",
            "目标文件已存在，请选择其他文件名",
            false,
        ));
    }
    fs::copy(source, destination).map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}
