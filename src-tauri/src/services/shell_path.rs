use std::sync::OnceLock;

/// On macOS, .app bundles inherit a minimal PATH that doesn't include
/// paths from the user's shell profile (e.g. nvm, homebrew, cargo).
/// This builds an extended PATH by prepending common tool directories
/// without scanning the filesystem (to avoid triggering macOS permission prompts).
fn full_path() -> &'static Option<String> {
    static FULL_PATH: OnceLock<Option<String>> = OnceLock::new();
    FULL_PATH.get_or_init(|| {
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").ok()?;
            let current_path = std::env::var("PATH").unwrap_or_default();

            let extra_dirs = [
                format!("{home}/.nvm/versions/node/default/bin"),
                format!("{home}/.local/bin"),
                format!("{home}/.cargo/bin"),
                "/opt/homebrew/bin".to_string(),
                "/usr/local/bin".to_string(),
                format!("{home}/.npm/bin"),
                format!("{home}/.volta/bin"),
                format!("{home}/.fnm/aliases/default/bin"),
            ];

            let mut all_paths: Vec<String> = extra_dirs
                .into_iter()
                .filter(|dir| !current_path.contains(dir.as_str()))
                .collect();

            if all_paths.is_empty() {
                return None;
            }

            all_paths.push(current_path);
            Some(all_paths.join(":"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    })
}

/// Apply the user's full shell PATH to a tokio `Command`.
pub fn apply_shell_path(cmd: &mut tokio::process::Command) {
    if let Some(path) = full_path() {
        cmd.env("PATH", path);
    }
}
