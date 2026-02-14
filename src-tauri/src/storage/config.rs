use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Return the app-specific data directory without touching the parent directory.
/// On macOS `dirs::data_local_dir()` resolves `~/Library/Application Support/`
/// which triggers TCC prompts for "access data from other apps". By constructing
/// the path directly from `$HOME` we avoid that.
pub fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("diaroo");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(dir) = dirs::data_local_dir() {
            return dir.join("diaroo");
        }
    }
    PathBuf::from(".").join("diaroo")
}

/// Return the app-specific config directory without touching the parent directory.
fn app_config_dir() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("diaroo");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(dir) = dirs::config_dir() {
            return dir.join("diaroo");
        }
    }
    PathBuf::from(".").join("diaroo")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub llm_provider: String,
    pub api_key: String,
    pub model: String,
    pub api_endpoint: String,
    pub screenshot_interval_secs: u64,
    pub batch_interval_secs: u64,
    pub dedup_threshold: u32,
    pub data_dir: String,
    pub pet_name: String,
    pub pet_size: String,
    pub auto_report_enabled: bool,
    pub auto_report_time: String,
    pub wander_enabled: bool,
    pub pet_position_x: Option<f64>,
    pub pet_position_y: Option<f64>,
    pub auto_start_monitoring_time_enabled: bool,
    pub auto_start_monitoring_time: String,
    pub launch_at_startup: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let data_dir = app_data_dir().join("data");

        Self {
            llm_provider: "openrouter".to_string(),
            api_key: String::new(),
            model: "openai/gpt-4o-mini".to_string(),
            api_endpoint: String::new(),
            screenshot_interval_secs: 5,
            batch_interval_secs: 300,
            dedup_threshold: 5,
            data_dir: data_dir.to_string_lossy().to_string(),
            pet_name: "platypus".to_string(),
            pet_size: "medium".to_string(),
            auto_report_enabled: false,
            auto_report_time: "17:00".to_string(),
            wander_enabled: true,
            pet_position_x: None,
            pet_position_y: None,
            auto_start_monitoring_time_enabled: false,
            auto_start_monitoring_time: "09:00".to_string(),
            launch_at_startup: false,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        app_config_dir().join("config.json")
    }

    pub fn prompt_path() -> PathBuf {
        app_config_dir().join("digest_prompt.txt")
    }

    pub fn extract_prompt_path() -> PathBuf {
        app_config_dir().join("extract_prompt.txt")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_json::from_str(&contents)?;
            Ok(config)
        } else {
            let config = AppConfig::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, contents)?;
        Ok(())
    }

    pub fn data_path(&self) -> PathBuf {
        PathBuf::from(&self.data_dir)
    }
}
