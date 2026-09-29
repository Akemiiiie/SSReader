mod backup;
mod books;
mod reading;

pub use backup::*;
pub use books::*;
pub use reading::*;

use crate::{domain::AppError, storage::AppState};
use tauri::{AppHandle, Manager};

pub(super) fn internal(error: impl ToString) -> AppError {
    AppError::storage(error.to_string())
}

pub(super) async fn blocking<T: Send + 'static>(
    app: AppHandle,
    action: impl FnOnce(&AppState) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        action(&state)
    })
    .await
    .map_err(internal)?
}
