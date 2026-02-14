use crate::services::scheduler::Scheduler;
use crate::AppState;
use tauri::State;
use tauri_plugin_notification::NotificationExt;

#[tauri::command]
pub async fn start_monitoring(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let mut is_monitoring = state.is_monitoring.lock().unwrap();
    if *is_monitoring {
        return Err("Already monitoring".to_string());
    }

    let config = state.config.lock().unwrap().clone();
    let activity_log = state.activity_log.clone();

    // Create a new stop channel
    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    *state.stop_tx.lock().unwrap() = Some(stop_tx);

    Scheduler::start(config, activity_log, stop_rx, app_handle.clone());
    *is_monitoring = true;

    log::info!("Monitoring started");

    let _ = app_handle
        .notification()
        .builder()
        .title("Diaroo")
        .body("Monitoring started")
        .show();

    Ok(())
}

#[tauri::command]
pub async fn stop_monitoring(state: State<'_, AppState>, app_handle: tauri::AppHandle) -> Result<(), String> {
    let mut is_monitoring = state.is_monitoring.lock().unwrap();
    if !*is_monitoring {
        return Err("Not monitoring".to_string());
    }

    if let Some(tx) = state.stop_tx.lock().unwrap().take() {
        let _ = tx.send(true);
    }
    *is_monitoring = false;

    log::info!("Monitoring stopped");

    let _ = app_handle
        .notification()
        .builder()
        .title("Diaroo")
        .body("Monitoring stopped")
        .show();

    Ok(())
}
