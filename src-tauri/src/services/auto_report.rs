use std::sync::Arc;

use chrono::{Local, NaiveTime, Timelike};
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;
use tokio::time::{sleep, Duration};

use tauri::Manager;

use crate::services::activity_log::ActivityLog;
use crate::services::digest_generator::DigestGenerator;
use crate::services::llm_client::LlmClient;
use crate::storage::config::AppConfig;
use crate::storage::screenshot_store::ScreenshotStore;
use crate::AppState;

pub struct AutoReportScheduler;

impl AutoReportScheduler {
    pub fn start(
        config: AppConfig,
        activity_log: Arc<ActivityLog>,
        mut stop_rx: watch::Receiver<bool>,
        app_handle: tauri::AppHandle,
    ) {
        let target_time = parse_time(&config.auto_report_time);
        let data_dir = config.data_path();

        tauri::async_runtime::spawn(async move {
            loop {
                let wait = duration_until_next(target_time);
                log::info!(
                    "Auto-report scheduled in {} seconds (target {:02}:{:02})",
                    wait.as_secs(),
                    target_time.hour(),
                    target_time.minute()
                );

                tokio::select! {
                    _ = sleep(wait) => {}
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            log::info!("Auto-report scheduler stopped");
                            return;
                        }
                    }
                }

                // Notify that generation is starting
                let _ = app_handle.notification()
                    .builder()
                    .title("Diaroo")
                    .body("Generating daily report...")
                    .show();

                // Generate the digest
                let store = ScreenshotStore::new(data_dir.clone());
                let config = app_handle.state::<AppState>().config.lock().unwrap().clone();
                let llm = LlmClient::new(
                    &config.llm_provider,
                    &config.api_key,
                    &config.model,
                    &config.api_endpoint,
                    Some(config.data_path()),
                );
                match DigestGenerator::generate_daily_digest(&activity_log, &store, &llm).await {
                    Ok(path) => {
                        let _ = app_handle.emit("digest-ready", path.to_string_lossy().to_string());
                        log::info!("Auto-report generated: {:?}", path);

                        // Stop monitoring — report marks end of work
                        let state = app_handle.state::<AppState>();
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
                            log::info!("Monitoring stopped after auto-report generation");
                        }

                        let _ = app_handle.notification()
                            .builder()
                            .title("Diaroo")
                            .body("Daily report generated. Monitoring has been stopped.")
                            .show();
                    }
                    Err(e) => {
                        log::error!("Auto-report generation failed: {}", e);
                    }
                }

                // Sleep 60s to avoid double-trigger if loop re-computes near the same time
                tokio::select! {
                    _ = sleep(Duration::from_secs(60)) => {}
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            log::info!("Auto-report scheduler stopped");
                            return;
                        }
                    }
                }
            }
        });
    }
}

fn duration_until_next(target: NaiveTime) -> Duration {
    let now = Local::now().time();
    let secs_until = if now < target {
        (target - now).num_seconds()
    } else {
        // Target already passed today, schedule for tomorrow
        (chrono::Duration::days(1) - (now - target)).num_seconds()
    };
    Duration::from_secs(secs_until.max(1) as u64)
}

fn parse_time(time_str: &str) -> NaiveTime {
    NaiveTime::parse_from_str(time_str, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(17, 0, 0).unwrap())
}
