use anyhow::{anyhow, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: Vec<ContentPart>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Option<Vec<Choice>>,
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

/// Strip wrapping code fences (```markdown ... ```) that LLMs often add.
/// Handles cases where the LLM adds extra text after the closing fence.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    // Check if the text starts with a code fence
    if !trimmed.starts_with("```") {
        return text.to_string();
    }
    let lines: Vec<&str> = trimmed.lines().collect();
    // Find the opening fence (first line) and closing fence (last line starting with ```)
    if lines.len() < 3 {
        return text.to_string();
    }
    // Find the last line that is exactly ``` (possibly with trailing whitespace)
    let close_idx = lines.iter().rposition(|l| l.trim() == "```");
    if let Some(idx) = close_idx {
        if idx > 0 {
            // Extract content between opening and closing fence
            return lines[1..idx].join("\n").trim().to_string();
        }
    }
    text.to_string()
}

/// Resolve the path to the `codex` CLI.
/// On Windows the npm global bin (`%APPDATA%\npm`) is often missing from the
/// PATH inherited by GUI processes, so we check there explicitly.
fn resolve_codex_path() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let candidate = std::path::PathBuf::from(&appdata).join("npm").join("codex.cmd");
            if candidate.exists() {
                return candidate.to_string_lossy().to_string();
            }
        }
    }
    "codex".to_string()
}

pub struct LlmClient {
    client: reqwest::Client,
    provider: String,
    api_key: String,
    model: String,
    endpoint: String,
    workspace_dir: Option<PathBuf>,
}

impl LlmClient {
    pub fn new(
        provider: &str,
        api_key: &str,
        model: &str,
        endpoint: &str,
        workspace_dir: Option<PathBuf>,
    ) -> Self {
        let trimmed_key = api_key.trim().to_string();
        let resolved_endpoint = match provider {
            "ollama" => {
                if endpoint.is_empty() {
                    "http://localhost:11434/v1/chat/completions".to_string()
                } else {
                    let base = endpoint.trim_end_matches('/');
                    format!("{}/v1/chat/completions", base)
                }
            }
            "claude-code" | "codex" => String::new(),
            _ => "https://openrouter.ai/api/v1/chat/completions".to_string(),
        };
        log::debug!(
            "LlmClient created: provider={}, model={}, endpoint={}, key_len={}, workspace_dir={}",
            provider,
            model,
            resolved_endpoint,
            trimmed_key.len(),
            workspace_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".to_string())
        );
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(900))
                .connect_timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            provider: provider.to_string(),
            api_key: trimmed_key,
            model: model.to_string(),
            endpoint: resolved_endpoint,
            workspace_dir,
        }
    }

    fn prepare_workspace_dir(&self) -> Option<PathBuf> {
        let dir = self.workspace_dir.clone()?;
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("Failed to create CLI workspace dir {}: {}", dir.display(), e);
            return None;
        }
        Some(dir)
    }

    pub fn api_key_is_empty(&self) -> bool {
        if self.provider == "ollama" || self.provider == "claude-code" || self.provider == "codex" {
            return false;
        }
        self.api_key.is_empty()
    }

    pub async fn send_multimodal(
        &self,
        prompt: &str,
        images: &[Vec<u8>],
    ) -> Result<String> {
        log::info!(
            "Sending request to {} ({}): model={}, images={}, key_len={}",
            self.provider,
            self.endpoint,
            self.model,
            images.len(),
            self.api_key.len()
        );

        if self.provider == "claude-code" {
            return self.send_via_claude_cli(prompt).await;
        }

        if self.provider == "codex" {
            return self.send_via_codex_cli(prompt).await;
        }

        let mut content_parts = vec![ContentPart::Text {
            text: prompt.to_string(),
        }];

        for img in images {
            let b64 = base64::engine::general_purpose::STANDARD.encode(img);
            content_parts.push(ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: format!("data:image/jpeg;base64,{}", b64),
                },
            });
        }

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: content_parts,
            }],
            max_tokens: 2048,
        };

        let mut req = self.client.post(&self.endpoint).json(&request);
        if self.provider != "ollama" {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }
        let response = match req.send().await {
            Ok(resp) => resp,
            Err(e) => {
                // Walk the error chain to find the root cause
                let mut root = e.to_string();
                let mut source: Option<&dyn std::error::Error> = e.source();
                while let Some(cause) = source {
                    root = format!("{}: {}", root, cause);
                    source = cause.source();
                }
                log::error!(
                    "Failed to send request to {} ({}): {}",
                    self.provider,
                    self.endpoint,
                    root
                );
                return Err(e.into());
            }
        };

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("API error ({}): {}", status, text));
        }

        let body: OpenRouterResponse = response.json().await?;

        if let Some(err) = body.error {
            return Err(anyhow!("API error: {}", err.message));
        }

        let text = body
            .choices
            .and_then(|c| c.into_iter().next())
            .map(|c| c.message.content)
            .unwrap_or_default();

        if text.trim().is_empty() {
            return Err(anyhow!("LLM returned empty response"));
        }

        Ok(strip_code_fence(&text))
    }

    async fn send_via_claude_cli(&self, prompt: &str) -> Result<String> {
        log::info!("Sending prompt to claude CLI ({} bytes)", prompt.len());

        let mut cmd = Command::new("claude");
        super::shell_path::apply_shell_path(&mut cmd);
        cmd.arg("--print");

        if let Some(workspace_dir) = self.prepare_workspace_dir() {
            cmd.arg("--add-dir").arg(&workspace_dir).current_dir(&workspace_dir);
        }

        cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn()
            .map_err(|e| anyhow!("Failed to spawn claude CLI: {}. Is it installed and in PATH?", e))?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open stdin for claude CLI"))?;
        let prompt_owned = prompt.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(prompt_owned.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });

        let output = tokio::time::timeout(
            Duration::from_secs(900),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| anyhow!("claude CLI timed out after 900 seconds"))?
        .map_err(|e| anyhow!("claude CLI process error: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "claude CLI exited with {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if text.trim().is_empty() {
            return Err(anyhow!("claude CLI returned empty response"));
        }

        Ok(strip_code_fence(&text))
    }

    async fn send_via_codex_cli(&self, prompt: &str) -> Result<String> {
        log::info!("Sending prompt to codex CLI ({} bytes)", prompt.len());

        let output_file = std::env::temp_dir().join(format!("diaroo_codex_{}.txt", std::process::id()));
        let output_path = output_file.to_string_lossy().to_string();

        let codex_bin = resolve_codex_path();
        let mut cmd = Command::new(&codex_bin);
        super::shell_path::apply_shell_path(&mut cmd);
        cmd.arg("exec");

        if let Some(workspace_dir) = self.prepare_workspace_dir() {
            cmd.arg("--cd")
                .arg(&workspace_dir)
                .arg("--add-dir")
                .arg(&workspace_dir)
                .current_dir(&workspace_dir);
        }

        cmd.arg("--full-auto")
            .arg("--skip-git-repo-check")
            .arg("--output-last-message")
            .arg(&output_path)
            .arg("-")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        let mut child = cmd.spawn()
            .map_err(|e| anyhow!("Failed to spawn codex CLI: {}. Is it installed and in PATH?", e))?;

        let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("Failed to open stdin for codex CLI"))?;
        let prompt_owned = prompt.to_string();
        tokio::spawn(async move {
            let _ = stdin.write_all(prompt_owned.as_bytes()).await;
            let _ = stdin.shutdown().await;
        });

        let result = tokio::time::timeout(
            Duration::from_secs(900),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| anyhow!("codex CLI timed out after 900 seconds"))?
        .map_err(|e| anyhow!("codex CLI process error: {}", e))?;

        if !result.status.success() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let _ = tokio::fs::remove_file(&output_file).await;
            return Err(anyhow!(
                "codex CLI exited with {}: {}",
                result.status,
                stderr.trim()
            ));
        }

        let text = tokio::fs::read_to_string(&output_file).await
            .map_err(|e| anyhow!("Failed to read codex output file: {}", e))?;
        let _ = tokio::fs::remove_file(&output_file).await;

        if text.trim().is_empty() {
            return Err(anyhow!("codex CLI returned empty response"));
        }

        Ok(strip_code_fence(&text))
    }
}
