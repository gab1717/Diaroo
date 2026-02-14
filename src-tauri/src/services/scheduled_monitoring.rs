use chrono::{Local, NaiveTime, Timelike};
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;
use tokio::sync::watch;
use tokio::time::{sleep, Duration};

use crate::storage::config::AppConfig;
use crate::AppState;

pub struct ScheduledMonitoringScheduler;

impl ScheduledMonitoringScheduler {
    pub fn start(
        config: AppConfig,
        mut stop_rx: watch::Receiver<bool>,
        app_handle: tauri::AppHandle,
    ) {
        let target_time = parse_time(&config.auto_start_monitoring_time);

        tauri::async_runtime::spawn(async move {
            loop {
                let wait = duration_until_next(target_time);
                log::info!(
                    "Scheduled monitoring start in {} seconds (target {:02}:{:02})",
                    wait.as_secs(),
                    target_time.hour(),
                    target_time.minute()
                );

                tokio::select! {
                    _ = sleep(wait) => {}
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            log::info!("Scheduled monitoring scheduler stopped");
                            return;
                        }
                    }
                }

                // Check if already monitoring
                let state = app_handle.state::<AppState>();
                let already_monitoring = *state.is_monitoring.lock().unwrap();
                if already_monitoring {
                    log::info!("Scheduled monitoring trigger skipped — already monitoring");
                } else {
                    // Start monitoring (same logic as tray "Start Monitoring")
                    let config = state.config.lock().unwrap().clone();
                    let activity_log = state.activity_log.clone();
                    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                    *state.stop_tx.lock().unwrap() = Some(stop_tx);
                    *state.is_monitoring.lock().unwrap() = true;
                    crate::services::scheduler::Scheduler::start(
                        config,
                        activity_log,
                        stop_rx,
                        app_handle.clone(),
                    );
                    log::info!("Monitoring started by scheduled trigger");

                    crate::rebuild_tray_menu(&app_handle, true);
                    crate::update_tray_icon(&app_handle, true);

                    let _ = app_handle
                        .notification()
                        .builder()
                        .title("Diaroo")
                        .body("Monitoring started (scheduled)")
                        .show();
                }

                // Sleep 60s to avoid double-trigger if loop re-computes near the same time
                tokio::select! {
                    _ = sleep(Duration::from_secs(60)) => {}
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            log::info!("Scheduled monitoring scheduler stopped");
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
        (chrono::Duration::days(1) - (now - target)).num_seconds()
    };
    Duration::from_secs(secs_until.max(1) as u64)
}

fn parse_time(time_str: &str) -> NaiveTime {
    NaiveTime::parse_from_str(time_str, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(9, 0, 0).unwrap())
}
