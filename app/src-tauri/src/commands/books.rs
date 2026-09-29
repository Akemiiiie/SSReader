use super::{blocking, internal};
use crate::{
    domain::{AppError, BookSummary, ImportResult, ImportSelection},
    services::books,
    storage::{AppState, ImportJob},
};
use std::{
    fs,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn list_books(state: State<'_, AppState>) -> Result<Vec<BookSummary>, AppError> {
    state.database()?.list_books()
}

#[tauri::command]
pub fn get_book(state: State<'_, AppState>, book_id: String) -> Result<BookSummary, AppError> {
    books::read_book(state.database()?, &book_id)
}

#[tauri::command]
pub async fn select_import_file(app: AppHandle) -> Result<Option<ImportSelection>, AppError> {
    blocking(app.clone(), move |state| {
        let chosen = app
            .dialog()
            .file()
            .add_filter("阅读文件", &["txt", "pdf"])
            .blocking_pick_file();
        let Some(chosen) = chosen else {
            return Ok(None);
        };
        let path = chosen.into_path().map_err(internal)?;
        let metadata = fs::metadata(&path)
            .map_err(|_| AppError::new("FILE_UNREADABLE", "所选文件无法读取", true))?;
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名文件")
            .to_owned();
        let format = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !matches!(format.as_str(), "txt" | "pdf") {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                "只支持 TXT 和 PDF 文件",
                false,
            ));
        }
        let preview = if format == "txt" {
            Some(books::preview_txt(&path, None)?)
        } else {
            None
        };
        let token = uuid::Uuid::new_v4().to_string();
        state.jobs.lock().map_err(internal)?.insert(
            token.clone(),
            ImportJob {
                path,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
        Ok(Some(ImportSelection {
            token,
            file_name,
            format,
            byte_size: metadata.len(),
            txt_encoding: preview.as_ref().map(|value| value.encoding.clone()),
            preview: preview.as_ref().map(|value| value.text.clone()),
            encoding_uncertain: preview.as_ref().is_some_and(|value| value.uncertain),
        }))
    })
    .await
}

#[tauri::command]
pub async fn preview_txt_import(
    app: AppHandle,
    file_token: String,
    encoding: String,
) -> Result<books::TxtPreview, AppError> {
    blocking(app, move |state| {
        let path = state
            .jobs
            .lock()
            .map_err(internal)?
            .get(&file_token)
            .ok_or_else(|| {
                AppError::new(
                    "IMPORT_TOKEN_INVALID",
                    "导入选择已失效，请重新选择文件",
                    false,
                )
            })?
            .path
            .clone();
        if path
            .extension()
            .and_then(|value| value.to_str())
            .is_none_or(|value| !value.eq_ignore_ascii_case("txt"))
        {
            return Err(AppError::new(
                "UNSUPPORTED_FORMAT",
                "这不是 TXT 文件",
                false,
            ));
        }
        books::preview_txt(&path, Some(&encoding))
    })
    .await
}

#[tauri::command]
pub async fn import_book(
    app: AppHandle,
    file_token: String,
    encoding: Option<String>,
) -> Result<ImportResult, AppError> {
    blocking(app, move |state| {
        let (path, cancelled) = {
            let jobs = state.jobs.lock().map_err(internal)?;
            let job = jobs.get(&file_token).ok_or_else(|| {
                AppError::new(
                    "IMPORT_TOKEN_INVALID",
                    "导入选择已失效，请重新选择文件",
                    false,
                )
            })?;
            (job.path.clone(), job.cancelled.clone())
        };
        let _guard = state.operation.lock().map_err(internal)?;
        let result = if let Some(preferred) = encoding.as_deref() {
            books::import_book_with_encoding(state.database()?, &path, &cancelled, Some(preferred))
        } else {
            books::import_book(state.database()?, &path, &cancelled)
        };
        state.jobs.lock().map_err(internal)?.remove(&file_token);
        result
    })
    .await
}

#[tauri::command]
pub fn cancel_import(state: State<'_, AppState>, file_token: String) -> Result<(), AppError> {
    if let Some(job) = state.jobs.lock().map_err(internal)?.remove(&file_token) {
        job.cancelled.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn update_book(
    state: State<'_, AppState>,
    book_id: String,
    title: String,
    author: Option<String>,
) -> Result<BookSummary, AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    books::update_book(state.database()?, &book_id, title, author)
}

#[tauri::command]
pub fn archive_book(state: State<'_, AppState>, book_id: String) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    books::archive_book(state.database()?, &book_id)
}

#[tauri::command]
pub fn delete_book(state: State<'_, AppState>, book_id: String) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    books::delete_book(state.database()?, &book_id)
}

#[tauri::command]
pub async fn export_book(app: AppHandle, book_id: String) -> Result<bool, AppError> {
    blocking(app.clone(), move |state| {
        let db = state.database()?;
        let book = books::read_book(db, &book_id)?;
        let name = format!("{}.{}", book.title, book.format);
        let target = app
            .dialog()
            .file()
            .set_file_name(&name)
            .blocking_save_file();
        let Some(target) = target else {
            return Ok(false);
        };
        let destination = target.into_path().map_err(internal)?;
        let _guard = state.operation.lock().map_err(internal)?;
        books::export_book(db, &book_id, &destination)?;
        Ok(true)
    })
    .await
}
