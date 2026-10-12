//! Owns the system save and folder dialogs the engine's page may open.
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn save_file(
    app: AppHandle,
    name: String,
    contents: String,
) -> Result<Option<String>, String> {
    let Some(path) = app
        .dialog()
        .file()
        .set_file_name(&name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    std::fs::write(&path, contents).map_err(|e| e.to_string())?;
    Ok(Some(path.display().to_string()))
}

/// The system folder picker: Finder on macOS, Explorer on Windows, the desktop portal on Linux.
#[tauri::command]
pub async fn pick_folder(app: AppHandle, start: Option<String>) -> Result<Option<String>, String> {
    let mut dialog = app.dialog().file().set_title("Choose project folder");
    if let Some(start) = start.filter(|path| std::path::Path::new(path).is_dir()) {
        dialog = dialog.set_directory(start);
    }
    let Some(path) = dialog.blocking_pick_folder() else {
        return Ok(None);
    };
    Ok(Some(
        path.into_path()
            .map_err(|e| e.to_string())?
            .display()
            .to_string(),
    ))
}
