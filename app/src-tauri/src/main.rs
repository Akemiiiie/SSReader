#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod domain;
mod platform;
mod services;
mod storage;

use tauri::Manager;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

fn main() {
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, pressed, event| {
                    if pressed == &shortcut && event.state() == ShortcutState::Pressed {
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                if let Some(popup) = app.get_webview_window("analysis-popup") {
                                    let _ = popup.close();
                                }
                                let _ = window.hide();
                            } else {
                                let _ = window.unminimize();
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(move |app| {
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
            if let Err(error) = app.global_shortcut().register(shortcut) {
                if let Ok(mut status) = state.shortcut_error.lock() {
                    *status = Some(format!("Ctrl+Alt+R 注册失败：{error}"));
                }
            }
            app.manage(state);
            platform::period::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_ai_config,
            commands::set_ai_api_key,
            commands::clear_ai_api_key,
            commands::test_ai_connection,
            commands::analyze_selection,
            commands::refresh_word_examples,
            commands::speak_word,
            commands::assess_pronunciation,
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
