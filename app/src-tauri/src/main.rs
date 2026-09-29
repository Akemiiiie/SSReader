#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod domain;
mod platform;
mod services;
mod storage;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            platform::tray::setup(app)?;
            let state = match app.path().app_data_dir() {
                Ok(path) => storage::AppState::initialize(path),
                Err(error) => {
                    storage::AppState::failed(domain::AppError::storage(error.to_string()))
                }
            };
            if let (Some(database), Some(window)) =
                (&state.database, app.get_webview_window("main"))
            {
                if let Ok(settings) = services::reading::get_settings(database) {
                    let _ = window.set_always_on_top(settings.always_on_top);
                }
            }
            app.manage(state);
            platform::period::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_books,
            commands::get_book,
            commands::select_import_file,
            commands::preview_txt_import,
            commands::import_book,
            commands::cancel_import,
            commands::update_book,
            commands::archive_book,
            commands::delete_book,
            commands::export_book,
            commands::get_txt_slice,
            commands::get_txt_slice_before,
            commands::set_txt_encoding,
            commands::get_pdf_bytes,
            commands::get_pdf_page_text,
            commands::save_pdf_page_text,
            commands::save_position,
            commands::list_bookmarks,
            commands::add_bookmark,
            commands::rename_bookmark,
            commands::delete_bookmark,
            commands::get_reading_settings,
            commands::set_reading_settings,
            commands::get_pdf_mode,
            commands::set_pdf_mode,
            commands::create_backup,
            commands::select_backup_file,
            commands::restore_backup,
            commands::get_shortcut_status,
            commands::toggle_main_window
        ])
        .run(tauri::generate_context!())
        .expect("failed to start SSReader");
}
