use anyhow::Result;
use chrono::{Local, Timelike};
use std::sync::Arc;
use tauri::Emitter;
use tokio::sync::watch;
use tokio::time::{interval, sleep, Duration};

use tauri::Manager;

use crate::services::activity_log::ActivityLog;
use crate::services::digest_generator::DigestGenerator;
use crate::services::llm_client::LlmClient;
use crate::services::screenshot::{DHash, ScreenshotCapture};
use crate::services::window_info;
use crate::storage::config::AppConfig;
use crate::storage::screenshot_store::ScreenshotStore;
use crate::AppState;

struct TickResult {
    app_name: String,
    window_title: String,
    hash_distance: u32,
    was_skipped: bool,
}

pub struct Scheduler;

impl Scheduler {
    pub fn start(
        config: AppConfig,
        activity_log: Arc<ActivityLog>,
        _stop_rx: watch::Receiver<bool>,
        app_handle: tauri::AppHandle,
    ) {
        let screenshot_interval = config.screenshot_interval_secs;
        let batch_interval = config.batch_interval_secs;
        let dedup_threshold = config.dedup_threshold;
        let data_dir = config.data_path();

        // Screenshot capture task
        let log_clone = activity_log.clone();
        let data_dir_clone = data_dir.clone();
        let mut stop_rx_clone = _stop_rx.clone();
        let capture_app_handle = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            let store = ScreenshotStore::new(data_dir_clone);
            let mut ticker = interval(Duration::from_secs(screenshot_interval));
            let mut last_hash: Option<DHash> = None;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        match Self::capture_tick(&store, &log_clone, &mut last_hash, dedup_threshold) {
                            Ok(tick) => {
                                let _ = capture_app_handle.emit("activity-tick", serde_json::json!({
                                    "app_name": tick.app_name,
                                    "window_title": tick.window_title,
                                    "hash_distance": tick.hash_distance,
                                    "was_skipped": tick.was_skipped,
                                }));
                            }
                            Err(e) => {
                                log::error!("Screenshot capture error: {}", e);
                            }
                        }
                    }
                    _ = stop_rx_clone.changed() => {
                        if *stop_rx_clone.borrow() {
                            log::info!("Screenshot capture stopped");
                            break;
                        }
                    }
                }
            }
        });

        // Batch processing task
        let log_clone = activity_log.clone();
        let data_dir_clone = data_dir.clone();
        let mut stop_rx_clone = _stop_rx.clone();

        tauri::async_runtime::spawn(async move {
            let store = ScreenshotStore::new(data_dir_clone);
            let mut ticker = interval(Duration::from_secs(batch_interval));

            // Skip first tick (don't batch immediately)
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        log::info!("Batch tick fired, checking for unbatched entries...");
                        let config = app_handle.state::<AppState>().config.lock().unwrap().clone();
                        let llm = LlmClient::new(
                            &config.llm_provider,
                            &config.api_key,
                            &config.model,
                            &config.api_endpoint,
                            Some(config.data_path()),
                        );
                        match DigestGenerator::process_batch(&log_clone, &store, &llm).await {
                            Ok(Some(summary)) => {
                                log::info!("Batch processed: {}", &summary[..summary.len().min(100)]);
                                let _ = app_handle.emit("monitoring-status", serde_json::json!({
                                    "active": true,
                                    "last_batch_summary": summary,
                                }));
                            }
                            Ok(None) => {
                                log::info!("Batch tick: no unbatched entries to process");
                            }
                            Err(e) => log::error!("Batch processing error: {}", e),
                        }
                    }
                    _ = stop_rx_clone.changed() => {
                        if *stop_rx_clone.borrow() {
                            log::info!("Batch processing stopped");
                            break;
                        }
                    }
                }
            }
        });

        // Midnight rollover task
        let log_clone = activity_log.clone();
        let data_dir_clone = data_dir.clone();
        let mut stop_rx_clone = _stop_rx.clone();

        tauri::async_runtime::spawn(async move {
            let store = ScreenshotStore::new(data_dir_clone);
            loop {
                let wait = duration_until_midnight();
                log::info!("Midnight rollover scheduled in {} seconds", wait.as_secs());

                tokio::select! {
                    _ = sleep(wait) => {
                        let new_date = Local::now().format("%Y-%m-%d").to_string();
                        log::info!("Midnight rollover: transitioning to {}", new_date);

                        // Force activity_log to switch to new day's database
                        if let Err(e) = log_clone.ensure_today() {
                            log::error!("Midnight rollover: failed to switch database: {}", e);
                        }

                        // Ensure new day's screenshot directory exists
                        if let Err(e) = store.ensure_date_dir(&new_date) {
                            log::error!("Midnight rollover: failed to create date dir: {}", e);
                        }
                    }
                    _ = stop_rx_clone.changed() => {
                        if *stop_rx_clone.borrow() {
                            log::info!("Midnight rollover task stopped");
                            break;
                        }
                    }
                }
            }
        });
    }

    fn capture_tick(
        store: &ScreenshotStore,
        activity_log: &Arc<ActivityLog>,
        last_hash: &mut Option<DHash>,
        dedup_threshold: u32,
    ) -> Result<TickResult> {
        let (jpeg_data, hash) = ScreenshotCapture::capture()?;

        // Always get window info for the activity tick
        let window_info = window_info::get_active_window().unwrap_or_else(|_| {
            window_info::ActiveWindowInfo {
                title: String::new(),
                app_name: String::new(),
            }
        });

        // Dedup: skip saving if too similar to last screenshot
        let (hash_distance, was_skipped) = if let Some(ref prev_hash) = last_hash {
            let distance = prev_hash.distance(&hash);
            if distance < dedup_threshold {
                log::debug!("Screenshot skipped (hash distance: {})", distance);
                (distance, true)
            } else {
                (distance, false)
            }
        } else {
            (0, false)
        };

        if !was_skipped {
            *last_hash = Some(hash.clone());

            // Save screenshot
            let path = store.save_screenshot(&jpeg_data)?;
            let timestamp = Local::now().to_rfc3339();

            activity_log.insert_activity(
                &timestamp,
                &path.to_string_lossy(),
                &window_info.title,
                &window_info.app_name,
                &hash.to_hex(),
            )?;

            log::debug!(
                "Screenshot saved: {} ({} - {})",
                path.display(),
                window_info.app_name,
                window_info.title
            );
        }

        Ok(TickResult {
            app_name: window_info.app_name,
            window_title: window_info.title,
            hash_distance,
            was_skipped,
        })
    }
}

/// Compute how long until the next midnight (00:00:00).
fn duration_until_midnight() -> Duration {
    let now = Local::now();
    let elapsed_secs =
        now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    let secs_until = 86400 - elapsed_secs + 1; // +1 to land just past midnight
    Duration::from_secs(secs_until)
}
