use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use uuid::Uuid;

use super::books::{book_file, decode_txt, now};
use crate::{domain::AppError, storage::Database};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxtChunk {
    pub paragraph_index: usize,
    pub code_point_offset: usize,
    pub text: String,
    pub fingerprint: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxtSlice {
    pub chunks: Vec<TxtChunk>,
    pub total_paragraphs: usize,
    pub next_paragraph_index: Option<usize>,
    pub next_code_point_offset: Option<usize>,
    pub encoding: String,
    pub chapters: Vec<Chapter>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub title: String,
    pub paragraph_index: usize,
}

fn fingerprint(text: &str) -> String {
    let hash = Sha256::digest(text.as_bytes());
    format!("{:x}", hash)[..16].to_owned()
}

fn is_chapter(text: &str) -> bool {
    let trimmed = text.trim();
    let count = trimmed.chars().count();
    count > 2
        && count < 80
        && (trimmed.starts_with("Chapter ")
            || trimmed.starts_with("CHAPTER ")
            || (trimmed.starts_with('第') && (trimmed.contains('章') || trimmed.contains('节'))))
}

fn txt_content(db: &Database, book_id: &str) -> Result<(String, &'static str), AppError> {
    let (path, format) = book_file(db, book_id)?;
    if format != "txt" {
        return Err(AppError::new(
            "UNSUPPORTED_FORMAT",
            "这不是 TXT 书籍",
            false,
        ));
    }
    let encoding: Option<String> = db
        .connection()?
        .query_row(
            "SELECT txt_encoding FROM books WHERE id=?1",
            [book_id],
            |row| row.get(0),
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    let bytes = fs::read(path).map_err(|error| AppError::storage(error.to_string()))?;
    decode_txt(&bytes, encoding.as_deref())
}

pub fn txt_slice(
    db: &Database,
    book_id: &str,
    paragraph_index: usize,
    code_point_offset: usize,
) -> Result<TxtSlice, AppError> {
    let (text, detected) = txt_content(db, book_id)?;
    let paragraphs: Vec<&str> = text.split('\n').collect();
    if paragraph_index >= paragraphs.len() {
        return Err(AppError::new(
            "ANCHOR_STALE",
            "阅读位置已超出正文，已回到开头",
            false,
        ));
    }
    let chapters = paragraphs
        .iter()
        .enumerate()
        .filter(|(_, text)| is_chapter(text))
        .take(200)
        .map(|(paragraph_index, text)| Chapter {
            title: text.trim().to_owned(),
            paragraph_index,
        })
        .collect();
    let mut chunks = Vec::new();
    let mut remaining = 8000usize;
    let mut index = paragraph_index;
    let mut offset = code_point_offset;
    while index < paragraphs.len() && remaining > 0 && chunks.len() < 30 {
        let paragraph = paragraphs[index];
        let length = paragraph.chars().count();
        if offset > length {
            offset = 0;
        }
        let text: String = paragraph
            .chars()
            .skip(offset)
            .take(remaining.min(3000))
            .collect();
        let taken = text.chars().count();
        chunks.push(TxtChunk {
            paragraph_index: index,
            code_point_offset: offset,
            text,
            fingerprint: fingerprint(paragraph),
        });
        remaining = remaining.saturating_sub(taken.max(1));
        offset += taken;
        if offset >= length {
            index += 1;
            offset = 0;
        }
        if taken == 0 && length > 0 {
            break;
        }
    }
    Ok(TxtSlice {
        chunks,
        total_paragraphs: paragraphs.len(),
        next_paragraph_index: (index < paragraphs.len()).then_some(index),
        next_code_point_offset: (index < paragraphs.len()).then_some(offset),
        encoding: detected.to_owned(),
        chapters,
    })
}

pub fn txt_slice_before(
    db: &Database,
    book_id: &str,
    paragraph_index: usize,
    code_point_offset: usize,
) -> Result<Option<TxtSlice>, AppError> {
    let (text, detected) = txt_content(db, book_id)?;
    let paragraphs: Vec<&str> = text.split('\n').collect();
    if paragraph_index >= paragraphs.len() {
        return Err(AppError::new(
            "ANCHOR_STALE",
            "阅读位置已超出正文，已回到开头",
            false,
        ));
    }
    let mut chunks = Vec::new();
    let mut remaining = 8000usize;
    let mut index = paragraph_index;
    let mut end = code_point_offset.min(paragraphs[index].chars().count());
    while remaining > 0 && chunks.len() < 30 {
        if end == 0 {
            if index == 0 {
                break;
            }
            index -= 1;
            end = paragraphs[index].chars().count();
            if end == 0 {
                chunks.push(TxtChunk {
                    paragraph_index: index,
                    code_point_offset: 0,
                    text: String::new(),
                    fingerprint: fingerprint(paragraphs[index]),
                });
                remaining -= 1;
                continue;
            }
        }
        let taken = end.min(remaining.min(3000));
        let start = end - taken;
        chunks.push(TxtChunk {
            paragraph_index: index,
            code_point_offset: start,
            text: paragraphs[index].chars().skip(start).take(taken).collect(),
            fingerprint: fingerprint(paragraphs[index]),
        });
        remaining -= taken;
        end = start;
    }
    if chunks.is_empty() {
        return Ok(None);
    }
    chunks.reverse();
    Ok(Some(TxtSlice {
        chunks,
        total_paragraphs: paragraphs.len(),
        next_paragraph_index: Some(paragraph_index),
        next_code_point_offset: Some(code_point_offset),
        encoding: detected.to_owned(),
        chapters: Vec::new(),
    }))
}

pub fn set_txt_encoding(db: &Database, book_id: &str, encoding: &str) -> Result<(), AppError> {
    let (path, format) = book_file(db, book_id)?;
    if format != "txt" {
        return Err(AppError::new(
            "UNSUPPORTED_FORMAT",
            "这不是 TXT 书籍",
            false,
        ));
    }
    let bytes = fs::read(path).map_err(|error| AppError::storage(error.to_string()))?;
    let (text, _) = decode_txt(&bytes, Some(encoding))?;
    let count = text.split('\n').count() as i64;
    db.connection()?
        .execute(
            "UPDATE books SET txt_encoding=?1, paragraph_count=?2 WHERE id=?3",
            params![encoding, count, book_id],
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

pub(crate) fn validate_anchor(anchor: &Value) -> Result<(), AppError> {
    if anchor.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(AppError::new("ANCHOR_STALE", "不支持的阅读位置版本", false));
    }
    match anchor.get("kind").and_then(Value::as_str) {
        Some("txt")
            if anchor
                .get("paragraphIndex")
                .and_then(Value::as_u64)
                .is_some()
                && anchor
                    .get("codePointOffset")
                    .and_then(Value::as_u64)
                    .is_some() =>
        {
            Ok(())
        }
        Some("pdf")
            if anchor
                .get("pageNo")
                .and_then(Value::as_u64)
                .is_some_and(|page| page > 0)
                && matches!(
                    anchor.get("mode").and_then(Value::as_str),
                    Some("original" | "reflow")
                ) =>
        {
            Ok(())
        }
        _ => Err(AppError::new("ANCHOR_STALE", "阅读位置格式无效", false)),
    }
}

pub fn save_position(db: &Database, book_id: &str, anchor: Value) -> Result<(), AppError> {
    validate_anchor(&anchor)?;
    let serialized =
        serde_json::to_string(&anchor).map_err(|error| AppError::storage(error.to_string()))?;
    let mut connection = db.connection()?;
    let transaction = connection
        .transaction()
        .map_err(|error| AppError::storage(error.to_string()))?;
    transaction.execute("INSERT INTO reading_positions (book_id,anchor_json,updated_at) VALUES (?1,?2,?3) ON CONFLICT(book_id) DO UPDATE SET anchor_json=excluded.anchor_json,updated_at=excluded.updated_at", params![book_id, serialized, now()])
        .map_err(|error| AppError::storage(error.to_string()))?;
    transaction
        .execute(
            "UPDATE books SET last_opened_at=?1 WHERE id=?2",
            params![now(), book_id],
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    transaction
        .commit()
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bookmark {
    pub id: String,
    pub anchor: Value,
    pub label: Option<String>,
    pub created_at: String,
}

pub fn list_bookmarks(db: &Database, book_id: &str) -> Result<Vec<Bookmark>, AppError> {
    let connection = db.connection()?;
    let mut statement = connection.prepare("SELECT id,anchor_json,label,created_at FROM bookmarks WHERE book_id=?1 ORDER BY created_at DESC")
        .map_err(|error| AppError::storage(error.to_string()))?;
    let rows = statement
        .query_map([book_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| AppError::storage(error.to_string()))?;
    rows.map(|item| {
        let (id, anchor, label, created_at) =
            item.map_err(|error| AppError::storage(error.to_string()))?;
        Ok(Bookmark {
            id,
            anchor: serde_json::from_str(&anchor)
                .map_err(|error| AppError::storage(error.to_string()))?,
            label,
            created_at,
        })
    })
    .collect()
}

pub fn add_bookmark(
    db: &Database,
    book_id: &str,
    anchor: Value,
    label: Option<String>,
) -> Result<Bookmark, AppError> {
    validate_anchor(&anchor)?;
    let key =
        serde_json::to_string(&anchor).map_err(|error| AppError::storage(error.to_string()))?;
    let id = Uuid::new_v4().to_string();
    let created_at = now();
    let label = label
        .map(|value| value.trim().chars().take(120).collect::<String>())
        .filter(|value| !value.is_empty());
    let changed = db.connection()?.execute("INSERT OR IGNORE INTO bookmarks (id,book_id,anchor_key,anchor_json,label,created_at) VALUES (?1,?2,?3,?4,?5,?6)", params![id, book_id, key, key, label, created_at])
        .map_err(|error| AppError::storage(error.to_string()))?;
    if changed == 0 {
        return Err(AppError::new("DUPLICATE_BOOKMARK", "此位置已有书签", false));
    }
    Ok(Bookmark {
        id,
        anchor,
        label,
        created_at,
    })
}

pub fn rename_bookmark(db: &Database, id: &str, label: String) -> Result<(), AppError> {
    let label = label.trim().chars().take(120).collect::<String>();
    db.connection()?
        .execute(
            "UPDATE bookmarks SET label=?1 WHERE id=?2",
            params![label, id],
        )
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

pub fn delete_bookmark(db: &Database, id: &str) -> Result<(), AppError> {
    db.connection()?
        .execute("DELETE FROM bookmarks WHERE id=?1", [id])
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ReadingSettings {
    pub font_size: u16,
    pub font_family: String,
    pub font_color: Option<String>,
    pub line_height: f32,
    pub theme: String,
    pub sidebar_collapsed: bool,
    pub pdf_zoom: f32,
    pub always_on_top: bool,
    pub opacity: f32,
    pub previous_page_shortcut: String,
    pub next_page_shortcut: String,
    pub window_shortcut_mode: WindowShortcutMode,
}

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowShortcutMode {
    Hold,
    Double,
}

impl Default for WindowShortcutMode {
    fn default() -> Self {
        Self::Double
    }
}

impl Default for ReadingSettings {
    fn default() -> Self {
        Self {
            font_size: 19,
            font_family: "system".into(),
            font_color: None,
            line_height: 1.8,
            theme: "paper".into(),
            sidebar_collapsed: false,
            pdf_zoom: 1.0,
            always_on_top: false,
            opacity: 1.0,
            previous_page_shortcut: "ArrowLeft".into(),
            next_page_shortcut: "ArrowRight".into(),
            window_shortcut_mode: WindowShortcutMode::Double,
        }
    }
}

pub fn get_settings(db: &Database) -> Result<ReadingSettings, AppError> {
    let value: Option<String> = db
        .connection()?
        .query_row(
            "SELECT value_json FROM reading_settings WHERE key='global'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| AppError::storage(error.to_string()))?;
    value
        .map(|text| {
            serde_json::from_str(&text).map_err(|error| AppError::storage(error.to_string()))
        })
        .unwrap_or_else(|| Ok(ReadingSettings::default()))
}

pub fn set_settings(db: &Database, settings: ReadingSettings) -> Result<ReadingSettings, AppError> {
    if !(12..=36).contains(&settings.font_size)
        || !matches!(settings.font_family.as_str(), "book" | "system" | "mono")
        || settings.font_color.as_deref().is_some_and(|color| {
            color.len() != 7
                || !color.starts_with('#')
                || !color[1..].bytes().all(|character| character.is_ascii_hexdigit())
        })
        || !(1.2..=2.6).contains(&settings.line_height)
        || !(0.25..=3.0).contains(&settings.pdf_zoom)
        || !(0.0..=1.0).contains(&settings.opacity)
        || !matches!(settings.theme.as_str(), "paper" | "light" | "dark")
    {
        return Err(AppError::new(
            "INVALID_INPUT",
            "阅读设置超出允许范围",
            false,
        ));
    }
    let json =
        serde_json::to_string(&settings).map_err(|error| AppError::storage(error.to_string()))?;
    db.connection()?.execute("INSERT INTO reading_settings(key,value_json) VALUES ('global',?1) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", [json])
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(settings)
}

pub fn get_pdf_mode(db: &Database, book_id: &str) -> Result<String, AppError> {
    let mode: Option<String> = db
        .connection()?
        .query_row(
            "SELECT pdf_mode FROM book_preferences WHERE book_id=?1",
            [book_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(mode.unwrap_or_else(|| "reflow".into()))
}

pub fn set_pdf_mode(db: &Database, book_id: &str, mode: &str) -> Result<(), AppError> {
    if !matches!(mode, "reflow" | "original") {
        return Err(AppError::new("INVALID_INPUT", "阅读模式无效", false));
    }
    db.connection()?.execute("INSERT INTO book_preferences(book_id,pdf_mode) VALUES (?1,?2) ON CONFLICT(book_id) DO UPDATE SET pdf_mode=excluded.pdf_mode", params![book_id, mode])
        .map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfBlock {
    pub id: String,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPageText {
    pub page_no: u32,
    pub quality: String,
    pub blocks: Vec<PdfBlock>,
    pub extractor_version: String,
}

pub fn get_pdf_page_text(
    db: &Database,
    book_id: &str,
    page_no: u32,
) -> Result<Option<PdfPageText>, AppError> {
    let value: Option<(String, String, String)> = db.connection()?.query_row(
        "SELECT quality,blocks_json,extractor_version FROM pdf_page_text WHERE book_id=?1 AND page_no=?2 ORDER BY extractor_version DESC LIMIT 1",
        params![book_id, page_no], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    ).optional().map_err(|error| AppError::storage(error.to_string()))?;
    value
        .map(|(quality, blocks, extractor_version)| {
            Ok(PdfPageText {
                page_no,
                quality,
                blocks: serde_json::from_str(&blocks)
                    .map_err(|error| AppError::storage(error.to_string()))?,
                extractor_version,
            })
        })
        .transpose()
}

pub fn save_pdf_page_text(
    db: &Database,
    book_id: &str,
    page_no: u32,
    quality: String,
    blocks: Vec<PdfBlock>,
    extractor_version: String,
) -> Result<(), AppError> {
    if page_no == 0
        || !matches!(quality.as_str(), "good" | "uncertain" | "no-text")
        || blocks.len() > 2000
    {
        return Err(AppError::new("INVALID_INPUT", "PDF 页文本数据无效", false));
    }
    let json =
        serde_json::to_string(&blocks).map_err(|error| AppError::storage(error.to_string()))?;
    if json.len() > 512 * 1024 {
        return Err(AppError::new("INVALID_INPUT", "PDF 页文本过大", false));
    }
    db.connection()?.execute("INSERT INTO pdf_page_text(book_id,page_no,extractor_version,quality,blocks_json) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(book_id,page_no,extractor_version) DO UPDATE SET quality=excluded.quality,blocks_json=excluded.blocks_json",
        params![book_id, page_no, extractor_version, quality, json]).map_err(|error| AppError::storage(error.to_string()))?;
    Ok(())
}

pub fn pdf_bytes(db: &Database, book_id: &str) -> Result<Vec<u8>, AppError> {
    let (path, format) = book_file(db, book_id)?;
    if format != "pdf" {
        return Err(AppError::new(
            "UNSUPPORTED_FORMAT",
            "这不是 PDF 书籍",
            false,
        ));
    }
    fs::read(path).map_err(|error| AppError::storage(error.to_string()))
}

#[cfg(test)]
mod settings_compatibility_tests {
    use super::ReadingSettings;

    #[test]
    fn older_settings_keep_values_and_default_to_system_font() {
        let settings: ReadingSettings = serde_json::from_str(
            r##"{"fontSize":24,"opacity":0.0,"fontColor":"#456b92","theme":"dark"}"##,
        ).unwrap();
        assert_eq!(settings.font_family, "system");
        assert_eq!(settings.font_size, 24);
        assert_eq!(settings.opacity, 0.0);
        assert_eq!(settings.font_color.as_deref(), Some("#456b92"));
        assert_eq!(settings.theme, "dark");
    }

    #[test]
    fn selected_font_survives_settings_serialization() {
        let settings = ReadingSettings { font_family: "mono".into(), ..Default::default() };
        let restored: ReadingSettings = serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
        assert_eq!(restored.font_family, "mono");
    }
}
