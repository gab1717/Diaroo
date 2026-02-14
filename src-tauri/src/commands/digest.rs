use chrono::Local;
use tauri_plugin_notification::NotificationExt;

use crate::services::digest_generator::DigestGenerator;
use crate::services::llm_client::LlmClient;
use crate::storage::screenshot_store::ScreenshotStore;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn generate_digest(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    date: Option<String>,
) -> Result<String, String> {
    let config = state.config.lock().unwrap().clone();
    let activity_log = state.activity_log.clone();
    let store = ScreenshotStore::new(config.data_path());
    let llm = LlmClient::new(
        &config.llm_provider,
        &config.api_key,
        &config.model,
        &config.api_endpoint,
        Some(config.data_path()),
    );

    let target_date = date.unwrap_or_else(|| Local::now().format("%Y-%m-%d").to_string());

    let report_path =
        DigestGenerator::generate_digest_for_date(&activity_log, &store, &llm, &target_date)
            .await
            .map_err(|e| e.to_string())?;

    // Stop monitoring — report marks end of work
    let was_monitoring = {
        let mut is_monitoring = state.is_monitoring.lock().unwrap();
        if *is_monitoring {
            if let Some(tx) = state.stop_tx.lock().unwrap().take() {
                let _ = tx.send(true);
            }
            *is_monitoring = false;
            true
        } else {
            false
        }
    };
    if was_monitoring {
        crate::rebuild_tray_menu(&app_handle, false);
        crate::update_tray_icon(&app_handle, false);
        log::info!("Monitoring stopped after manual report generation");

        let _ = app_handle.notification()
            .builder()
            .title("Diaroo")
            .body("Report generated. Monitoring has been stopped.")
            .show();
    }

    Ok(report_path.to_string_lossy().to_string())
}
