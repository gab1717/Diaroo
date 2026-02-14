use anyhow::Result;

#[derive(Debug, Clone)]
pub struct ActiveWindowInfo {
    pub title: String,
    pub app_name: String,
}

#[cfg(target_os = "windows")]
pub fn get_active_window() -> Result<ActiveWindowInfo> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };
    use windows::Win32::Foundation::HWND;

    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0.is_null() {
            return Ok(ActiveWindowInfo {
                title: "Unknown".to_string(),
                app_name: "Unknown".to_string(),
            });
        }

        // Get window title
        let mut title_buf = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut title_buf);
        let title = String::from_utf16_lossy(&title_buf[..len as usize]);

        // Get process name
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        let app_name = get_process_name(pid).unwrap_or_else(|| "Unknown".to_string());

        Ok(ActiveWindowInfo { title, app_name })
    }
}

#[cfg(target_os = "windows")]
fn get_process_name(pid: u32) -> Option<String> {
    use windows::Win32::System::ProcessStatus::GetModuleBaseNameW;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
        let mut buf = [0u16; 260];
        let len = GetModuleBaseNameW(handle, None, &mut buf);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

#[cfg(target_os = "macos")]
pub fn get_active_window() -> Result<ActiveWindowInfo> {
    let app_name = get_frontmost_app_name().unwrap_or_else(|| "Unknown".to_string());
    let title = get_frontmost_window_title().unwrap_or_default();

    Ok(ActiveWindowInfo { app_name, title })
}

/// Get the frontmost application name via NSWorkspace (no special permissions needed).
#[cfg(target_os = "macos")]
fn get_frontmost_app_name() -> Option<String> {
    use objc2_app_kit::NSWorkspace;

    let workspace = NSWorkspace::sharedWorkspace();
    let app = workspace.frontmostApplication()?;
    let name = app.localizedName()?;
    Some(name.to_string())
}

/// Get the frontmost window title via CGWindowListCopyWindowInfo.
/// Requires Screen Recording permission (which is already needed for xcap screenshots).
#[cfg(target_os = "macos")]
fn get_frontmost_window_title() -> Option<String> {
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_graphics::display::{
        kCGNullWindowID, kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
        CGWindowListCopyWindowInfo,
    };
    use std::ffi::c_void;

    unsafe {
        let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
        let window_list = CGWindowListCopyWindowInfo(options, kCGNullWindowID);
        if window_list.is_null() {
            return None;
        }

        let cf_array = core_foundation::array::CFArray::<CFDictionary<*const c_void, *const c_void>>::wrap_under_create_rule(
            window_list as _,
        );

        let key_layer = CFString::new("kCGWindowLayer");
        let key_name = CFString::new("kCGWindowName");

        for i in 0..cf_array.len() {
            let dict = match cf_array.get(i) {
                Some(d) => d,
                None => continue,
            };

            // Only consider layer 0 (normal windows)
            if let Some(layer_val) = dict.find(key_layer.as_CFTypeRef()) {
                let layer: CFNumber = CFNumber::wrap_under_get_rule(*layer_val as *const _);
                if let Some(l) = layer.to_i32() {
                    if l != 0 {
                        continue;
                    }
                }
            }

            // Get the window name
            if let Some(name_val) = dict.find(key_name.as_CFTypeRef()) {
                let name: CFString = CFString::wrap_under_get_rule(*name_val as *const _);
                let title = name.to_string();
                if !title.is_empty() {
                    return Some(title);
                }
            }
        }
    }

    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn get_active_window() -> Result<ActiveWindowInfo> {
    Ok(ActiveWindowInfo {
        title: "Unknown".to_string(),
        app_name: "Unknown".to_string(),
    })
}
