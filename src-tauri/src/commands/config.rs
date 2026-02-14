use crate::services::auto_report::AutoReportScheduler;
use crate::services::scheduled_monitoring::ScheduledMonitoringScheduler;
use crate::storage::config::AppConfig;
use crate::AppState;
use tauri::{Emitter, State};

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let config = state.config.lock().unwrap().clone();
    Ok(config)
}

#[tauri::command]
pub async fn set_config(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    mut config: AppConfig,
) -> Result<(), String> {
    // Trim whitespace from API key (common copy-paste issue)
    config.api_key = config.api_key.trim().to_string();

    let old_pet_name = {
        let old = state.config.lock().unwrap();
        old.pet_name.clone()
    };

    config.save().map_err(|e| e.to_string())?;
    let new_pet_name = config.pet_name.clone();
    let launch_at_startup = config.launch_at_startup;
    *state.config.lock().unwrap() = config;

    if old_pet_name != new_pet_name {
        let _ = app_handle.emit_to("pet", "pet-changed", &new_pet_name);
    }

    // Sync autostart state
    {
        use tauri_plugin_autostart::ManagerExt;
        let autostart = app_handle.autolaunch();
        if launch_at_startup {
            let _ = autostart.enable();
        } else {
            let _ = autostart.disable();
        }
    }

    // Restart or stop auto-report scheduler based on new config
    restart_auto_report(&app_handle, &state);

    // Restart or stop scheduled monitoring scheduler based on new config
    restart_scheduled_monitoring(&app_handle, &state);

    Ok(())
}

#[tauri::command]
pub async fn save_pet_position(
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    config.pet_position_x = Some(x);
    config.pet_position_y = Some(y);
    config.save().map_err(|e| e.to_string())?;
    Ok(())
}

fn restart_auto_report(app_handle: &tauri::AppHandle, state: &State<'_, AppState>) {
    // Stop existing scheduler if running
    if let Some(tx) = state.auto_report_stop_tx.lock().unwrap().take() {
        let _ = tx.send(true);
        log::info!("Auto-report scheduler stopped for config update");
    }

    let config = state.config.lock().unwrap().clone();
    if config.auto_report_enabled {
        let (tx, rx) = tokio::sync::watch::channel(false);
        *state.auto_report_stop_tx.lock().unwrap() = Some(tx);
        AutoReportScheduler::start(
            config,
            state.activity_log.clone(),
            rx,
            app_handle.clone(),
        );
        log::info!("Auto-report scheduler restarted");
    } else {
        log::info!("Auto-report scheduler disabled");
    }
}

fn restart_scheduled_monitoring(app_handle: &tauri::AppHandle, state: &State<'_, AppState>) {
    // Stop existing scheduler if running
    if let Some(tx) = state.scheduled_monitoring_stop_tx.lock().unwrap().take() {
        let _ = tx.send(true);
        log::info!("Scheduled monitoring scheduler stopped for config update");
    }

    let config = state.config.lock().unwrap().clone();
    if config.auto_start_monitoring_time_enabled {
        let (tx, rx) = tokio::sync::watch::channel(false);
        *state.scheduled_monitoring_stop_tx.lock().unwrap() = Some(tx);
        ScheduledMonitoringScheduler::start(
            config,
            rx,
            app_handle.clone(),
        );
        log::info!("Scheduled monitoring scheduler restarted");
    } else {
        log::info!("Scheduled monitoring scheduler disabled");
    }
}
