use anyhow::Result;
use chrono::Local;
use std::path::PathBuf;

pub struct ScreenshotStore {
    base_dir: PathBuf,
}

impl ScreenshotStore {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    pub fn today_dir(&self) -> PathBuf {
        let date = Local::now().format("%Y-%m-%d").to_string();
        self.base_dir.join(&date)
    }

    pub fn date_dir(&self, date: &str) -> PathBuf {
        self.base_dir.join(date)
    }

    pub fn ensure_today_dir(&self) -> Result<PathBuf> {
        let dir = self.today_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn ensure_date_dir(&self, date: &str) -> Result<PathBuf> {
        let dir = self.date_dir(date);
        std::fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    pub fn save_screenshot(&self, jpeg_data: &[u8]) -> Result<PathBuf> {
        let dir = self.ensure_today_dir()?;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S%.3f").to_string();
        let filename = format!("screenshot_{}.jpg", timestamp);
        let path = dir.join(&filename);
        std::fs::write(&path, jpeg_data)?;
        Ok(path)
    }

    pub fn delete_screenshot(&self, path: &PathBuf) -> Result<()> {
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    pub fn save_report_for_date(&self, markdown: &str, date: &str) -> Result<PathBuf> {
        let dir = self.ensure_date_dir(date)?;
        let path = dir.join("report.md");
        std::fs::write(&path, markdown)?;
        Ok(path)
    }

    /// Delete all screenshot .jpg files in a date's folder.
    pub fn cleanup_screenshots_for_date(&self, date: &str) -> Result<u32> {
        let dir = self.date_dir(date);
        let mut deleted = 0u32;
        if dir.exists() {
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |ext| ext == "jpg") {
                    std::fs::remove_file(&path)?;
                    deleted += 1;
                }
            }
        }
        Ok(deleted)
    }
}
