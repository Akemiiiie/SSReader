use super::{blocking, internal};
use crate::{domain::AppError, services::reading, storage::AppState};
use tauri::{ipc::Response, AppHandle, Emitter, Manager, State, Window};

#[tauri::command]
pub fn get_txt_slice(
    state: State<'_, AppState>,
    book_id: String,
    paragraph_index: usize,
    code_point_offset: usize,
) -> Result<reading::TxtSlice, AppError> {
    reading::txt_slice(
        state.database()?,
        &book_id,
        paragraph_index,
        code_point_offset,
    )
}

#[tauri::command]
pub fn get_txt_slice_before(
    state: State<'_, AppState>,
    book_id: String,
    paragraph_index: usize,
    code_point_offset: usize,
) -> Result<Option<reading::TxtSlice>, AppError> {
    reading::txt_slice_before(state.database()?, &book_id, paragraph_index, code_point_offset)
}

#[tauri::command]
pub fn set_txt_encoding(
    state: State<'_, AppState>,
    book_id: String,
    encoding: String,
) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::set_txt_encoding(state.database()?, &book_id, &encoding)
}

#[tauri::command]
pub async fn get_pdf_bytes(app: AppHandle, book_id: String) -> Result<Response, AppError> {
    blocking(app, move |state| {
        Ok(Response::new(reading::pdf_bytes(
            state.database()?,
            &book_id,
        )?))
    })
    .await
}

#[tauri::command]
pub fn get_pdf_page_text(
    state: State<'_, AppState>,
    book_id: String,
    page_no: u32,
) -> Result<Option<reading::PdfPageText>, AppError> {
    reading::get_pdf_page_text(state.database()?, &book_id, page_no)
}

#[tauri::command]
pub fn save_pdf_page_text(
    state: State<'_, AppState>,
    book_id: String,
    page_no: u32,
    quality: String,
    blocks: Vec<reading::PdfBlock>,
    extractor_version: String,
) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::save_pdf_page_text(
        state.database()?,
        &book_id,
        page_no,
        quality,
        blocks,
        extractor_version,
    )
}

#[tauri::command]
pub fn save_position(
    state: State<'_, AppState>,
    book_id: String,
    anchor: serde_json::Value,
) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::save_position(state.database()?, &book_id, anchor)
}

#[tauri::command]
pub fn list_bookmarks(
    state: State<'_, AppState>,
    book_id: String,
) -> Result<Vec<reading::Bookmark>, AppError> {
    reading::list_bookmarks(state.database()?, &book_id)
}

#[tauri::command]
pub fn add_bookmark(
    state: State<'_, AppState>,
    book_id: String,
    anchor: serde_json::Value,
    label: Option<String>,
) -> Result<reading::Bookmark, AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::add_bookmark(state.database()?, &book_id, anchor, label)
}

#[tauri::command]
pub fn rename_bookmark(
    state: State<'_, AppState>,
    id: String,
    label: String,
) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::rename_bookmark(state.database()?, &id, label)
}

#[tauri::command]
pub fn delete_bookmark(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::delete_bookmark(state.database()?, &id)
}

#[tauri::command]
pub fn get_reading_settings(
    state: State<'_, AppState>,
) -> Result<reading::ReadingSettings, AppError> {
    reading::get_settings(state.database()?)
}

#[tauri::command]
pub fn set_reading_settings(
    state: State<'_, AppState>,
    window: Window,
    settings: reading::ReadingSettings,
) -> Result<reading::ReadingSettings, AppError> {
    window
        .set_always_on_top(settings.always_on_top)
        .map_err(internal)?;
    let _guard = state.operation.lock().map_err(internal)?;
    let saved = reading::set_settings(state.database()?, settings)?;
    // Notify the independent webview after persistence; a closed popup is fine.
    let _ = window.app_handle().emit_to("analysis-popup", "reading-settings-changed", &saved);
    Ok(saved)
}

#[tauri::command]
pub fn get_pdf_mode(state: State<'_, AppState>, book_id: String) -> Result<String, AppError> {
    reading::get_pdf_mode(state.database()?, &book_id)
}

#[tauri::command]
pub fn set_pdf_mode(
    state: State<'_, AppState>,
    book_id: String,
    mode: String,
) -> Result<(), AppError> {
    let _guard = state.operation.lock().map_err(internal)?;
    reading::set_pdf_mode(state.database()?, &book_id, &mode)
}

#[tauri::command]
pub fn get_shortcut_status(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    Ok(state.shortcut_error.lock().map_err(internal)?.clone())
}

#[tauri::command]
pub fn toggle_main_window(window: Window) -> Result<(), AppError> {
    if window.is_visible().map_err(internal)? {
        if let Some(popup) = window.app_handle().get_webview_window("analysis-popup") {
            popup.close().map_err(internal)?;
        }
        window.hide().map_err(internal)
    } else {
        window.show().map_err(internal)?;
        window.set_focus().map_err(internal)
    }
}
