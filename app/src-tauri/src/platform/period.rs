#[cfg(target_os = "windows")]
use crate::{
    services::reading::{self, WindowShortcutMode},
    storage::AppState,
};
#[cfg(target_os = "windows")]
use std::time::{Duration, Instant};
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tauri::Manager;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    #[link_name = "GetAsyncKeyState"]
    fn get_async_key_state(key: i32) -> i16;
}

#[cfg(target_os = "windows")]
const PERIOD_KEY: i32 = 0xBE;
#[cfg(target_os = "windows")]
const NUMPAD_DECIMAL_KEY: i32 = 0x6E;

#[cfg(target_os = "windows")]
fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(target_os = "windows")]
fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

#[cfg(target_os = "windows")]
fn toggle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show(app);
        }
    }
}

#[cfg(target_os = "windows")]
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut pressed_at: Option<Instant> = None;
        let mut last_tap_at: Option<Instant> = None;
        let mut active_mode = WindowShortcutMode::Double;
        let mut hold_shown = false;
        let mut double_handled = false;

        loop {
            let now = Instant::now();
            let pressed = unsafe {
                get_async_key_state(PERIOD_KEY) < 0 || get_async_key_state(NUMPAD_DECIMAL_KEY) < 0
            };
            if pressed && pressed_at.is_none() {
                active_mode = app
                    .state::<AppState>()
                    .database()
                    .ok()
                    .and_then(|database| reading::get_settings(database).ok())
                    .map(|settings| settings.window_shortcut_mode)
                    .unwrap_or_default();
                pressed_at = Some(now);
                hold_shown = false;
                double_handled = false;
                match active_mode {
                    WindowShortcutMode::Hold => last_tap_at = None,
                    WindowShortcutMode::Double => {
                        if last_tap_at.is_some_and(|last| {
                            now.duration_since(last) <= Duration::from_millis(400)
                        }) {
                            toggle(&app);
                            last_tap_at = None;
                            double_handled = true;
                        }
                    }
                }
            } else if pressed {
                if matches!(active_mode, WindowShortcutMode::Hold)
                    && !hold_shown
                    && pressed_at.is_some_and(|start| {
                        now.duration_since(start) >= Duration::from_millis(600)
                    })
                {
                    show(&app);
                    hold_shown = true;
                }
            } else if let Some(start) = pressed_at.take() {
                match active_mode {
                    WindowShortcutMode::Hold => {
                        if hold_shown {
                            hide(&app);
                        }
                    }
                    WindowShortcutMode::Double => {
                        last_tap_at = if !double_handled
                            && now.duration_since(start) < Duration::from_millis(600)
                        {
                            Some(now)
                        } else {
                            None
                        };
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    });
}

#[cfg(not(target_os = "windows"))]
pub fn start(_app: AppHandle) {}
