use crate::AppState;
use tauri::{Emitter, State};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[tauri::command]
pub async fn run_claude(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    prompt: String,
) -> Result<(), String> {
    let config = state.config.lock().unwrap().clone();
    let data_dir = config.data_path();
    std::fs::create_dir_all(&data_dir)
        .map_err(|e| format!("Failed to prepare data directory: {}", e))?;

    let mut cmd = Command::new("claude");
    crate::services::shell_path::apply_shell_path(&mut cmd);
    let mut child = cmd
        .args(["--print", &prompt])
        .arg("--add-dir")
        .arg(&data_dir)
        .current_dir(&data_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn claude: {}", e))?;

    let stdout = child.stdout.take().ok_or("No stdout")?;
    let mut reader = BufReader::new(stdout).lines();

    let handle = app_handle.clone();
    tokio::spawn(async move {
        while let Ok(Some(line)) = reader.next_line().await {
            let _ = handle.emit(
                "claude-output",
                serde_json::json!({ "text": line, "done": false }),
            );
        }
        let _ = handle.emit(
            "claude-output",
            serde_json::json!({ "text": "", "done": true }),
        );
    });

    Ok(())
}
