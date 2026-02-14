use serde::Serialize;

use crate::services::digest_generator::{DEFAULT_DIGEST_PROMPT, DEFAULT_EXTRACT_PROMPT};
use crate::storage::config::AppConfig;
use crate::AppState;
use tauri::State;
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize, Clone)]
pub struct DateInfo {
    pub date: String,
    pub has_report: bool,
}

#[tauri::command]
pub fn list_data_dates(state: State<'_, AppState>) -> Result<Vec<DateInfo>, String> {
    let config = state.config.lock().unwrap().clone();
    let data_dir = config.data_path();

    if !data_dir.exists() {
        return Ok(vec![]);
    }

    let mut dates: Vec<DateInfo> = std::fs::read_dir(&data_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Must match YYYY-MM-DD pattern and contain activity.db
            if name.len() == 10
                && name.chars().nth(4) == Some('-')
                && name.chars().nth(7) == Some('-')
                && entry.path().join("activity.db").exists()
            {
                let has_report = entry.path().join("report.md").exists();
                Some(DateInfo {
                    date: name,
                    has_report,
                })
            } else {
                None
            }
        })
        .collect();

    dates.sort_by(|a, b| b.date.cmp(&a.date));
    Ok(dates)
}

#[tauri::command]
pub fn list_reports(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let config = state.config.lock().unwrap().clone();
    let data_dir = config.data_path();

    if !data_dir.exists() {
        return Ok(vec![]);
    }

    let mut dates: Vec<String> = std::fs::read_dir(&data_dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Must match YYYY-MM-DD pattern and contain report.md
            if name.len() == 10
                && name.chars().nth(4) == Some('-')
                && name.chars().nth(7) == Some('-')
                && entry.path().join("report.md").exists()
            {
                Some(name)
            } else {
                None
            }
        })
        .collect();

    dates.sort();
    dates.reverse();
    Ok(dates)
}

#[tauri::command]
pub fn read_report(state: State<'_, AppState>, date: String) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let report_path = config.data_path().join(&date).join("report.md");

    std::fs::read_to_string(&report_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_report_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    date: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let path = config.data_path().join(&date).join("report.md");

    if !path.exists() {
        return Err("Report file not found".to_string());
    }

    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_prompt_file(app: tauri::AppHandle) -> Result<(), String> {
    let path = AppConfig::prompt_path();

    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, DEFAULT_DIGEST_PROMPT).map_err(|e| e.to_string())?;
    }

    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_extract_prompt_file(app: tauri::AppHandle) -> Result<(), String> {
    let path = AppConfig::extract_prompt_path();

    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, DEFAULT_EXTRACT_PROMPT).map_err(|e| e.to_string())?;
    }

    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}
