use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde_json::Value;
use tauri::{AppHandle, Manager};

static WRITE_LOCK: Mutex<()> = Mutex::new(());

fn task_path(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join("tasks.json"))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn load_tasks(app: AppHandle) -> Result<Option<Vec<Value>>, String> {
    let path = task_path(&app)?;
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn write_tasks(path: &Path, tasks: &[Value]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let pending = path.with_extension(format!("{}.tmp", std::process::id()));
    let result = (|| {
        let mut file = File::create(&pending)?;
        serde_json::to_writer(&mut file, tasks)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::rename(&pending, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(pending);
    }
    result
}

#[tauri::command]
pub fn save_tasks(app: AppHandle, tasks: Vec<Value>) -> Result<(), String> {
    let path = task_path(&app)?;
    let _guard = WRITE_LOCK.lock().map_err(|error| error.to_string())?;
    write_tasks(&path, &tasks).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replaces_task_snapshot_without_losing_fields() -> io::Result<()> {
        let directory =
            std::env::temp_dir().join(format!("next-up-store-test-{}", std::process::id()));
        let path = directory.join("tasks.json");
        let tasks = vec![json!({
            "id": "task-1",
            "name": "Review notes",
            "trackedMs": 12000,
            "startedAt": 10,
            "deadlineAt": 20,
            "link": "https://example.com",
            "done": false
        })];
        write_tasks(&path, &tasks)?;
        assert_eq!(
            serde_json::from_slice::<Vec<Value>>(&fs::read(&path)?)?,
            tasks
        );
        write_tasks(&path, &[])?;
        assert_eq!(fs::read_to_string(&path)?, "[]");
        assert_eq!(fs::read_dir(&directory)?.count(), 1);
        fs::remove_dir_all(directory)
    }
}
