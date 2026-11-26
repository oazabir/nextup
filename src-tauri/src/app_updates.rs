use std::sync::Mutex;
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct Updates(Mutex<PendingUpdate>);

#[derive(Default)]
struct PendingUpdate {
    available: Option<Update>,
    downloaded: Option<Vec<u8>>,
}

#[tauri::command]
pub async fn check_for_update(
    app: AppHandle,
    state: State<'_, Updates>,
) -> Result<Option<String>, String> {
    if cfg!(debug_assertions) {
        return Err("Updates are available in installed release builds.".into());
    }
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        return Err(
            "Download the latest .deb from GitHub Releases to update this installation.".into(),
        );
    }

    let update = app
        .updater_builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let available = update.as_ref().map(|update| update.version.clone());
    *state.0.lock().map_err(|error| error.to_string())? = PendingUpdate {
        available: update,
        downloaded: None,
    };
    Ok(available)
}

#[tauri::command]
pub async fn download_update(version: String, state: State<'_, Updates>) -> Result<(), String> {
    let update = {
        let pending = state.0.lock().map_err(|error| error.to_string())?;
        let update = pending
            .available
            .as_ref()
            .filter(|update| update.version == version)
            .ok_or("Check for updates again.")?;
        if pending.downloaded.is_some() {
            return Ok(());
        }
        update.clone()
    };
    let bytes = update
        .download(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    let mut pending = state.0.lock().map_err(|error| error.to_string())?;
    if pending
        .available
        .as_ref()
        .map(|update| update.version.as_str())
        != Some(version.as_str())
    {
        return Err("The available update changed. Check for updates again.".into());
    }
    pending.downloaded = Some(bytes);
    Ok(())
}

#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    version: String,
    state: State<'_, Updates>,
) -> Result<(), String> {
    let (update, bytes) = {
        let mut pending = state.0.lock().map_err(|error| error.to_string())?;
        let update = pending
            .available
            .as_ref()
            .filter(|update| update.version == version)
            .ok_or("Check for updates again.")?
            .clone();
        let bytes = pending
            .downloaded
            .take()
            .ok_or("Download the update before installing.")?;
        (update, bytes)
    };
    update.install(bytes).map_err(|error| error.to_string())?;
    app.restart();
}
