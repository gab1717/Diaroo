mod commands;
mod services;
mod storage;

use services::activity_log::ActivityLog;
use storage::config::AppConfig;

use std::path::Path;
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use std::sync::atomic::{AtomicIsize, Ordering};
use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, Submenu},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    Emitter, LogicalPosition, LogicalSize, Manager,
};
use tauri_plugin_notification::NotificationExt;


fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dest_path)?;
        } else {
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub activity_log: Arc<ActivityLog>,
    pub is_monitoring: Mutex<bool>,
    pub stop_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    pub auto_report_stop_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    pub scheduled_monitoring_stop_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
    pub quitting: std::sync::atomic::AtomicBool,
}

/// Create a 32x32 RGBA icon with a green dot indicator in the bottom-right corner.
fn create_monitoring_icon(base_icon: &Image<'_>) -> Image<'static> {
    let width = base_icon.width();
    let height = base_icon.height();
    let mut rgba = base_icon.rgba().to_vec();

    // Draw a green filled circle (radius 5) at bottom-right
    let cx = (width - 7) as i32;
    let cy = (height - 7) as i32;
    let radius = 5i32;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let dx = x - cx;
            let dy = y - cy;
            if dx * dx + dy * dy <= radius * radius {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                if idx + 3 < rgba.len() {
                    // Green dot
                    rgba[idx] = 0x4C;     // R
                    rgba[idx + 1] = 0xD9; // G
                    rgba[idx + 2] = 0x64; // B
                    rgba[idx + 3] = 0xFF; // A
                }
            }
        }
    }

    Image::new_owned(rgba, width, height)
}

fn size_to_render(size: &str) -> u32 {
    match size {
        "small" => 48,
        "large" => 96,
        _ => 64, // medium
    }
}

fn size_to_window(size: &str) -> (f64, f64) {
    let r = size_to_render(size) as f64;
    (r, r)
}

/// Window labels that should cause the Dock icon to appear.
#[cfg(target_os = "macos")]
const DOCK_WINDOWS: &[&str] = &["settings", "reports", "digest", "pet-picker"];

/// Show Dock icon when a secondary window opens.
#[cfg(target_os = "macos")]
fn show_dock_icon(app: &tauri::AppHandle) {
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
}

/// Hide Dock icon if no secondary windows are visible.
#[cfg(target_os = "macos")]
fn hide_dock_icon_if_no_windows(app: &tauri::AppHandle) {
    let any_visible = DOCK_WINDOWS.iter().any(|label| {
        app.get_webview_window(label)
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false)
    });
    if !any_visible {
        let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    }
}

fn show_and_focus_window(window: &tauri::WebviewWindow<tauri::Wry>) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn build_size_submenu(app: &impl Manager<tauri::Wry>, current_size: &str) -> Submenu<tauri::Wry> {
    let small = CheckMenuItem::with_id(app, "size_small", "Small", true, current_size == "small", None::<&str>).unwrap();
    let medium = CheckMenuItem::with_id(app, "size_medium", "Medium", true, current_size == "medium", None::<&str>).unwrap();
    let large = CheckMenuItem::with_id(app, "size_large", "Large", true, current_size == "large", None::<&str>).unwrap();
    Submenu::with_items(app, "Pet Size", true, &[&small, &medium, &large]).unwrap()
}

pub(crate) fn rebuild_tray_menu(app: &tauri::AppHandle, is_monitoring: bool) {
    let label = if is_monitoring {
        "Stop Monitoring"
    } else {
        "Start Monitoring"
    };

    let (current_size, wander_enabled) = {
        let state = app.state::<AppState>();
        let config = state.config.lock().unwrap();
        (config.pet_size.clone(), config.wander_enabled)
    };

    let pet_visible = app
        .get_webview_window("pet")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(true);
    let hide_show_label = if pet_visible { "Hide Pet" } else { "Show Pet" };

    let toggle_item = MenuItem::with_id(app, "toggle_monitor", label, true, None::<&str>).unwrap();
    let digest_item = MenuItem::with_id(app, "digest", "Generate Report", true, None::<&str>).unwrap();
    let reports_item = MenuItem::with_id(app, "view_reports", "View Reports", true, None::<&str>).unwrap();
    let size_submenu = build_size_submenu(app, &current_size);
    let wander_item = CheckMenuItem::with_id(app, "wander", "Wander", true, wander_enabled, None::<&str>).unwrap();
    let hide_show_item = MenuItem::with_id(app, "hide_show_pet", hide_show_label, true, None::<&str>).unwrap();
    let change_pet_item = MenuItem::with_id(app, "change_pet", "Switch Pet", true, None::<&str>).unwrap();
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>).unwrap();
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>).unwrap();

    let menu = Menu::with_items(
        app,
        &[&toggle_item, &digest_item, &reports_item, &size_submenu, &wander_item, &hide_show_item, &change_pet_item, &settings_item, &quit_item],
    )
    .unwrap();

    if let Some(tray) = app.tray_by_id("main-tray") {
        let _ = tray.set_menu(Some(menu));
    }
}

pub(crate) fn update_tray_icon(app: &tauri::AppHandle, is_monitoring: bool) {
    if let Some(tray) = app.tray_by_id("main-tray") {
        if is_monitoring {
            let base = app.default_window_icon().unwrap();
            let active_icon = create_monitoring_icon(base);
            let _ = tray.set_icon(Some(active_icon));
            let _ = tray.set_tooltip(Some("Diaroo - Monitoring"));
        } else {
            let _ = tray.set_icon(Some(app.default_window_icon().unwrap().clone()));
            let _ = tray.set_tooltip(Some("Diaroo"));
        }
    }
}

#[cfg(target_os = "windows")]
static PET_HWND: AtomicIsize = AtomicIsize::new(0);

#[cfg(target_os = "windows")]
unsafe extern "system" fn on_foreground_change(
    _hook: windows::Win32::UI::Accessibility::HWINEVENTHOOK,
    _event: u32,
    _hwnd: windows::Win32::Foundation::HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };
    let val = PET_HWND.load(Ordering::Relaxed);
    if val != 0 {
        let _ = SetWindowPos(
            HWND(val as *mut _),
            HWND_TOPMOST,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = AppConfig::load().unwrap_or_default();
    let data_dir = config.data_path();
    let activity_log =
        Arc::new(ActivityLog::new(&data_dir).expect("Failed to initialize activity log"));

    let app_state = AppState {
        config: Mutex::new(config.clone()),
        activity_log: activity_log.clone(),
        is_monitoring: Mutex::new(false),
        stop_tx: Mutex::new(None),
        auto_report_stop_tx: Mutex::new(None),
        scheduled_monitoring_stop_tx: Mutex::new(None),
        quitting: std::sync::atomic::AtomicBool::new(false),
    };

    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .max_file_size(5_000_000) // 5 MB
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("diaroo.log".into()),
                    },
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stderr,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .build(),
        )
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .manage(app_state)
        .setup(|app| {
            // Hide from Dock — only show in system tray
            #[cfg(target_os = "macos")]
            let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // Copy bundled pets to user data dir on first run.
            // In production, resources are at resource_dir()/pets/.
            // In dev, they're at the source tree: src-tauri/resources/pets/.
            let resource_path = app.path().resource_dir().ok().map(|p| p.join("pets"));
            let dev_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("resources")
                .join("pets");
            let res_pets = match &resource_path {
                Some(p) if p.is_dir() => Some(p.clone()),
                _ if dev_path.is_dir() => Some(dev_path),
                _ => None,
            };
            let user_pets = storage::pets::user_pets_dir();
            if let Some(res_pets) = res_pets {
                if let Ok(entries) = std::fs::read_dir(&res_pets) {
                    for entry in entries.flatten() {
                        let src = entry.path();
                        if !src.is_dir() {
                            continue;
                        }
                        let name = entry.file_name();
                        let dest = user_pets.join(&name);
                        if !dest.exists() {
                            if let Err(e) = copy_dir_all(&src, &dest) {
                                log::error!("Failed to copy bundled pet {:?}: {}", name, e);
                            } else {
                                // Mark as built-in
                                let _ = std::fs::write(dest.join(".builtin"), "");
                                log::info!("Copied bundled pet: {:?}", name);
                            }
                        }
                    }
                }
            }

            // Build initial tray menu
            let (current_size, wander_enabled, saved_position) = {
                let state = app.state::<AppState>();
                let config = state.config.lock().unwrap();
                (config.pet_size.clone(), config.wander_enabled, (config.pet_position_x, config.pet_position_y))
            };

            let toggle_item = MenuItem::with_id(app, "toggle_monitor", "Start Monitoring", true, None::<&str>)?;
            let digest_item = MenuItem::with_id(app, "digest", "Generate Report", true, None::<&str>)?;
            let reports_item = MenuItem::with_id(app, "view_reports", "View Reports", true, None::<&str>)?;
            let size_submenu = build_size_submenu(app, &current_size);
            let wander_item = CheckMenuItem::with_id(app, "wander", "Wander", true, wander_enabled, None::<&str>)?;
            let hide_show_item = MenuItem::with_id(app, "hide_show_pet", "Hide Pet", true, None::<&str>)?;
            let change_pet_item = MenuItem::with_id(app, "change_pet", "Switch Pet", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

            let menu = Menu::with_items(
                app,
                &[&toggle_item, &digest_item, &reports_item, &size_submenu, &wander_item, &hide_show_item, &change_pet_item, &settings_item, &quit_item],
            )?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(true)
                .tooltip("Diaroo")
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "toggle_monitor" => {
                            let state = app.state::<AppState>();
                            let is_monitoring = *state.is_monitoring.lock().unwrap();

                            if is_monitoring {
                                // Stop monitoring
                                if let Some(tx) = state.stop_tx.lock().unwrap().take() {
                                    let _ = tx.send(true);
                                }
                                *state.is_monitoring.lock().unwrap() = false;
                                log::info!("Monitoring stopped from tray");

                                rebuild_tray_menu(app, false);
                                update_tray_icon(app, false);

                                let _ = app
                                    .notification()
                                    .builder()
                                    .title("Diaroo")
                                    .body("Monitoring stopped")
                                    .show();
                            } else {
                                // Start monitoring
                                let app_handle = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let state = app_handle.state::<AppState>();
                                    let config = state.config.lock().unwrap().clone();
                                    let activity_log = state.activity_log.clone();
                                    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                                    *state.stop_tx.lock().unwrap() = Some(stop_tx);
                                    *state.is_monitoring.lock().unwrap() = true;
                                    services::scheduler::Scheduler::start(
                                        config,
                                        activity_log,
                                        stop_rx,
                                        app_handle.clone(),
                                    );
                                    log::info!("Monitoring started from tray");

                                    rebuild_tray_menu(&app_handle, true);
                                    update_tray_icon(&app_handle, true);

                                    let _ = app_handle
                                        .notification()
                                        .builder()
                                        .title("Diaroo")
                                        .body("Monitoring started")
                                        .show();
                                });
                            }
                        }
                        "digest" => {
                            #[cfg(target_os = "macos")]
                            show_dock_icon(app);

                            if let Some(window) = app.get_webview_window("digest") {
                                show_and_focus_window(&window);
                            } else {
                                let digest_window = tauri::WebviewWindowBuilder::new(
                                    app,
                                    "digest",
                                    tauri::WebviewUrl::App("digest.html".into()),
                                )
                                .title("Diaroo - Generate Digest")
                                .inner_size(500.0, 520.0)
                                .resizable(true)
                                .build();

                                if let Ok(window) = digest_window {
                                    show_and_focus_window(&window);
                                }
                            }
                        }
                        "size_small" | "size_medium" | "size_large" => {
                            let new_size = match event.id.as_ref() {
                                "size_small" => "small",
                                "size_large" => "large",
                                _ => "medium",
                            };

                            let state = app.state::<AppState>();
                            {
                                let mut config = state.config.lock().unwrap();
                                config.pet_size = new_size.to_string();
                                let _ = config.save();
                            }

                            if let Some(pet_window) = app.get_webview_window("pet") {
                                let (w, h) = size_to_window(new_size);
                                let _ = pet_window.set_size(LogicalSize::new(w, h));
                            }

                            let scale = size_to_render(new_size);
                            let _ = app.emit("pet-size-changed", scale);

                            let is_monitoring = *state.is_monitoring.lock().unwrap();
                            rebuild_tray_menu(app, is_monitoring);
                        }
                        "wander" => {
                            let state = app.state::<AppState>();
                            let new_val = {
                                let mut config = state.config.lock().unwrap();
                                config.wander_enabled = !config.wander_enabled;
                                let _ = config.save();
                                config.wander_enabled
                            };
                            let _ = app.emit("wander-toggled", new_val);

                            let is_monitoring = *state.is_monitoring.lock().unwrap();
                            rebuild_tray_menu(app, is_monitoring);
                        }
                        "hide_show_pet" => {
                            if let Some(window) = app.get_webview_window("pet") {
                                if window.is_visible().unwrap_or(true) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                }
                            }
                            let state = app.state::<AppState>();
                            let is_monitoring = *state.is_monitoring.lock().unwrap();
                            rebuild_tray_menu(app, is_monitoring);
                        }
                        "change_pet" => {
                            #[cfg(target_os = "macos")]
                            show_dock_icon(app);

                            if let Some(window) = app.get_webview_window("pet-picker") {
                                show_and_focus_window(&window);
                            } else {
                                let picker_window = tauri::WebviewWindowBuilder::new(
                                    app,
                                    "pet-picker",
                                    tauri::WebviewUrl::App("pet-picker.html".into()),
                                )
                                .title("Diaroo - Pets")
                                .inner_size(450.0, 400.0)
                                .resizable(true)
                                .build();

                                if let Ok(window) = picker_window {
                                    show_and_focus_window(&window);
                                }
                            }
                        }
                        "view_reports" => {
                            #[cfg(target_os = "macos")]
                            show_dock_icon(app);

                            if let Some(window) = app.get_webview_window("reports") {
                                show_and_focus_window(&window);
                            } else {
                                let reports_window = tauri::WebviewWindowBuilder::new(
                                    app,
                                    "reports",
                                    tauri::WebviewUrl::App("reports.html".into()),
                                )
                                .title("Diaroo - Reports")
                                .inner_size(800.0, 600.0)
                                .resizable(true)
                                .build();

                                if let Ok(window) = reports_window {
                                    show_and_focus_window(&window);
                                }
                            }
                        }
                        "settings" => {
                            #[cfg(target_os = "macos")]
                            show_dock_icon(app);

                            if let Some(window) = app.get_webview_window("settings") {
                                show_and_focus_window(&window);
                            } else {
                                let settings_window = tauri::WebviewWindowBuilder::new(
                                    app,
                                    "settings",
                                    tauri::WebviewUrl::App("settings.html".into()),
                                )
                                .title("Diaroo - Settings")
                                .inner_size(600.0, 500.0)
                                .resizable(true)
                                .build();

                                if let Ok(window) = settings_window {
                                    show_and_focus_window(&window);
                                }
                            }
                        }
                        "quit" => {
                            // Save pet position before quitting
                            if let Some(pet_window) = app.get_webview_window("pet") {
                                if let Ok(pos) = pet_window.outer_position() {
                                    if let Ok(sf) = pet_window.scale_factor() {
                                        let state = app.state::<AppState>();
                                        let mut config = state.config.lock().unwrap();
                                        config.pet_position_x = Some(pos.x as f64 / sf);
                                        config.pet_position_y = Some(pos.y as f64 / sf);
                                        let _ = config.save();
                                    }
                                }
                            }

                            let state = app.state::<AppState>();
                            if let Some(tx) = state.stop_tx.lock().unwrap().take() {
                                let _ = tx.send(true);
                            }
                            if let Some(tx) = state.auto_report_stop_tx.lock().unwrap().take() {
                                let _ = tx.send(true);
                            }
                            if let Some(tx) = state.scheduled_monitoring_stop_tx.lock().unwrap().take() {
                                let _ = tx.send(true);
                            }
                            state.quitting.store(true, std::sync::atomic::Ordering::SeqCst);
                            app.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        #[cfg(target_os = "macos")]
                        show_dock_icon(app);

                        if let Some(window) = app.get_webview_window("reports") {
                            show_and_focus_window(&window);
                        } else {
                            let reports_window = tauri::WebviewWindowBuilder::new(
                                app,
                                "reports",
                                tauri::WebviewUrl::App("reports.html".into()),
                            )
                            .title("Diaroo - Reports")
                            .inner_size(800.0, 600.0)
                            .resizable(true)
                            .build();

                            if let Ok(window) = reports_window {
                                show_and_focus_window(&window);
                            }
                        }
                    }
                })
                .build(app)?;

            // Hide Dock icon when all secondary windows are closed
            // (handled via on_window_event below)

            // Apply persisted pet size on startup
            if current_size != "medium" {
                if let Some(pet_window) = app.get_webview_window("pet") {
                    let (w, h) = size_to_window(&current_size);
                    let _ = pet_window.set_size(LogicalSize::new(w, h));
                }
            }

            // Apply persisted pet position on startup
            if let (Some(x), Some(y)) = saved_position {
                if let Some(pet_window) = app.get_webview_window("pet") {
                    let _ = pet_window.set_position(LogicalPosition::new(x, y));
                }
            }

            // Fix Windows 11 taskbar Z-order: use a WinEvent hook to instantly
            // re-assert HWND_TOPMOST whenever any window comes to the foreground.
            #[cfg(target_os = "windows")]
            if let Some(pet_window) = app.get_webview_window("pet") {
                use windows::Win32::UI::Accessibility::SetWinEventHook;
                use windows::Win32::UI::WindowsAndMessaging::{
                    GetMessageW, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
                };

                let hwnd = pet_window.hwnd().unwrap();
                PET_HWND.store(hwnd.0 as isize, Ordering::Relaxed);

                std::thread::spawn(move || unsafe {
                    let _hook = SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(on_foreground_change),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    );
                    // Pump messages so the hook callback fires
                    let mut msg = MSG::default();
                    while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
                });
            }

            // Start auto-report scheduler if enabled
            {
                let state = app.state::<AppState>();
                let cfg = state.config.lock().unwrap().clone();
                if cfg.auto_report_enabled {
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    *state.auto_report_stop_tx.lock().unwrap() = Some(tx);
                    services::auto_report::AutoReportScheduler::start(
                        cfg,
                        state.activity_log.clone(),
                        rx,
                        app.handle().clone(),
                    );
                }
            }

            // Sync autostart plugin state with config
            {
                use tauri_plugin_autostart::ManagerExt;
                let state = app.state::<AppState>();
                let cfg = state.config.lock().unwrap().clone();
                let autostart = app.handle().autolaunch();
                if cfg.launch_at_startup {
                    let _ = autostart.enable();
                } else {
                    let _ = autostart.disable();
                }
            }

            // Start scheduled monitoring scheduler if enabled
            {
                let state = app.state::<AppState>();
                let cfg = state.config.lock().unwrap().clone();
                if cfg.auto_start_monitoring_time_enabled {
                    // If the scheduled time has already passed today, start monitoring now
                    let target_time = chrono::NaiveTime::parse_from_str(
                        &cfg.auto_start_monitoring_time,
                        "%H:%M",
                    )
                    .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap());
                    let now = chrono::Local::now().time();

                    if now >= target_time {
                        let app_handle = app.handle().clone();
                        let activity_log = state.activity_log.clone();
                        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
                        *state.stop_tx.lock().unwrap() = Some(stop_tx);
                        *state.is_monitoring.lock().unwrap() = true;
                        services::scheduler::Scheduler::start(
                            cfg.clone(),
                            activity_log,
                            stop_rx,
                            app_handle.clone(),
                        );
                        log::info!("Monitoring auto-started (scheduled time already passed)");
                        rebuild_tray_menu(&app_handle, true);
                        update_tray_icon(&app_handle, true);

                        let _ = app_handle
                            .notification()
                            .builder()
                            .title("Diaroo")
                            .body("Monitoring started (scheduled)")
                            .show();
                    }

                    // Start the scheduler for future triggers (next day if already passed)
                    let (tx, rx) = tokio::sync::watch::channel(false);
                    *state.scheduled_monitoring_stop_tx.lock().unwrap() = Some(tx);
                    services::scheduled_monitoring::ScheduledMonitoringScheduler::start(
                        cfg,
                        rx,
                        app.handle().clone(),
                    );
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::monitor::start_monitoring,
            commands::monitor::stop_monitoring,
            commands::digest::generate_digest,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::save_pet_position,
            commands::claude::run_claude,
            commands::pets::list_pets,
            commands::pets::get_pet_info,
            commands::pets::install_pet,
            commands::pets::remove_pet,
            commands::pets::read_sprite,
            commands::reports::list_data_dates,
            commands::reports::list_reports,
            commands::reports::read_report,
            commands::reports::open_report_file,
            commands::reports::open_prompt_file,
            commands::reports::open_extract_prompt_file,
        ])
        .on_window_event(|_window, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::Destroyed = _event {
                let label = _window.label();
                if DOCK_WINDOWS.contains(&label) {
                    hide_dock_icon_if_no_windows(_window.app_handle());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let quitting = _app.state::<AppState>().quitting.load(std::sync::atomic::Ordering::SeqCst);
                if !quitting {
                    api.prevent_exit();
                }
            }
        });
}
