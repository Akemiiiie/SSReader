use super::{blocking, internal};
use crate::{domain::AppError, services::backup};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn create_backup(app: AppHandle) -> Result<Option<backup::BackupInfo>, AppError> {
    blocking(app.clone(), move |state| {
        let target = app
            .dialog()
            .file()
            .add_filter("SSReader 备份", &["ssrb"])
            .set_file_name("SSReader-backup.ssrb")
            .blocking_save_file();
        let Some(target) = target else {
            return Ok(None);
        };
        let path = target.into_path().map_err(internal)?;
        let _guard = state.operation.lock().map_err(internal)?;
        Ok(Some(backup::create_backup(state.database()?, &path)?))
    })
    .await
}

#[tauri::command]
pub async fn select_backup_file(app: AppHandle) -> Result<Option<backup::BackupInfo>, AppError> {
    blocking(app.clone(), move |state| {
        let selected = app
            .dialog()
            .file()
            .add_filter("SSReader 备份", &["ssrb"])
            .blocking_pick_file();
        let Some(selected) = selected else {
            return Ok(None);
        };
        let path = selected.into_path().map_err(internal)?;
        let info = backup::inspect_backup(&path)?;
        *state.backup_source.lock().map_err(internal)? = Some(path);
        Ok(Some(info))
    })
    .await
}

#[tauri::command]
pub async fn restore_backup(app: AppHandle) -> Result<backup::BackupInfo, AppError> {
    let restart_handle = app.clone();
    let info = blocking(app, move |state| {
        let source = state
            .backup_source
            .lock()
            .map_err(internal)?
            .take()
            .ok_or_else(|| AppError::new("INVALID_INPUT", "请先选择并预检备份文件", false))?;
        let _guard = state.operation.lock().map_err(internal)?;
        let data_dir = state
            .data_dir
            .as_ref()
            .ok_or_else(|| AppError::storage("本地数据目录不可用".into()))?;
        backup::restore_backup(data_dir, &source)
    })
    .await?;
    restart_handle.request_restart();
    Ok(info)
}
