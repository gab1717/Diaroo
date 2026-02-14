use anyhow::Result;
use chrono::Local;
use std::path::PathBuf;
use std::sync::Arc;

use crate::services::activity_log::{ActivityEntry, ActivityLog};
use crate::services::llm_client::LlmClient;
use crate::storage::config::AppConfig;
use crate::storage::screenshot_store::ScreenshotStore;

/// OpenRouter free models limit image uploads to 10 per request.
const MAX_IMAGES_PER_REQUEST: usize = 10;

pub const DEFAULT_EXTRACT_PROMPT: &str = "\
You are analyzing screenshots from a computer activity monitoring system.

Activity log for this batch:
{activity_log}

TASK: Extract factual information from each screenshot, focusing on accurate identification.

For each screenshot, identify:
1. Application name (look for title bars, app icons, menu bars, taskbar/dock)
2. Window title or document name (if visible)
3. Timestamp (from activity log)
4. Brief description of what's visible on screen (1 sentence max)

After extracting information from all screenshots, create a timeline summary (2-4 sentences) showing the sequence of applications used and main activities.

CRITICAL: Focus on reading visible text accurately. If you cannot confidently identify an application, describe what you see instead (e.g., \"code editor with dark theme\" rather than guessing \"VSCode\").";

pub const DEFAULT_DIGEST_PROMPT: &str = "\
Generate a comprehensive daily activity digest report in markdown format based on the provided batch summaries and app usage statistics.

## Batch Summaries
{batch_summaries}

## App Usage
{app_usage}

## Report Requirements

### Structure
Format the report with the following sections:

1. **Daily Summary**
   - Write a substantial paragraph (4-6 sentences) that provides a holistic overview of the user's day
   - Blend productivity-focused tasks with reactive work patterns
   - Mention specific projects, tools, and key outcomes
   - Include quantifiable metrics where available (e.g., email counts, number of meetings)
   - Balance technical activities with administrative and collaborative work

2. **Timeline**
   - Present activities in chronological order using bullet points with time stamps
   - Use precise times (e.g., \"11:45\", \"13:04\", \"14:24–15:50\") from the batch summaries
   - For each time entry, describe what the user was doing in specific terms
   - Group related activities that occurred within the same time window
   - Include tool/application names and specific tasks (e.g., \"Tauri Pet development in Terminal\")
   - Mention concrete details like error codes, file types, or specific features being worked on

3. **Focus Analysis**
   - Divide into clear subsections with bold headers:
     - **Productive Phases**: Highlight periods of deep work and accomplishments
     - **Productivity Challenges**: Identify obstacles, distractions, or inefficiencies
     - **Behavioral Insights**: Observations about work patterns and preferences
     - **Optimization Opportunities**: Actionable suggestions for improvement
   - Use bullet points for each insight
   - Be specific and evidence-based, referencing actual activities from the day
   - Balance positive observations with constructive feedback

### Formatting Guidelines
- Use markdown heading levels: # for title, ## for main sections
- Include the date in the title: \"# Daily Activity Digest Report - {date}\"
- Use bold text (**text**) for subsection headers within Focus Analysis
- Use em dashes (\\u2013) for time ranges in the Timeline
- Maintain a professional, analytical tone throughout
- Include specific application names, project names, and technical details

### Content Guidelines
- Synthesize information across both batch summaries and app usage data
- Identify patterns and themes rather than listing every action
- Highlight context-switching behavior when present
- Note communication platforms used for meetings
- Reference specific technical work (coding, debugging, API integration)
- Acknowledge breaks and leisure activities naturally
- Provide actionable insights in the Focus Analysis section

Date: {date}";

pub struct DigestGenerator;

impl DigestGenerator {
    fn load_digest_prompt() -> String {
        let path = AppConfig::prompt_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, DEFAULT_DIGEST_PROMPT);
        }
        match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => content,
            _ => DEFAULT_DIGEST_PROMPT.to_string(),
        }
    }

    fn load_extract_prompt() -> String {
        let path = AppConfig::extract_prompt_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, DEFAULT_EXTRACT_PROMPT);
        }
        match std::fs::read_to_string(&path) {
            Ok(content) if !content.trim().is_empty() => content,
            _ => DEFAULT_EXTRACT_PROMPT.to_string(),
        }
    }

    /// Process unbatched screenshots in chunks of MAX_IMAGES_PER_REQUEST,
    /// sending each chunk as its own LLM request.
    pub async fn process_batch(
        activity_log: &Arc<ActivityLog>,
        screenshot_store: &ScreenshotStore,
        llm_client: &LlmClient,
    ) -> Result<Option<String>> {
        let entries = activity_log.get_unbatched_entries()?;
        if entries.is_empty() {
            return Ok(None);
        }

        let chunks: Vec<&[ActivityEntry]> = entries.chunks(MAX_IMAGES_PER_REQUEST).collect();
        let total_chunks = chunks.len();
        let mut last_summary = None;

        for (i, chunk) in chunks.into_iter().enumerate() {
            log::info!("Processing chunk {}/{} ({} entries)", i + 1, total_chunks, chunk.len());
            let summary = Self::process_chunk(activity_log, screenshot_store, llm_client, chunk).await?;
            last_summary = Some(summary);
        }

        Ok(last_summary)
    }

    /// Process a single chunk of activity entries: load images, call LLM, store summary,
    /// and delete the chunk's screenshots.
    async fn process_chunk(
        activity_log: &Arc<ActivityLog>,
        screenshot_store: &ScreenshotStore,
        llm_client: &LlmClient,
        entries: &[ActivityEntry],
    ) -> Result<String> {
        let batch_id = uuid::Uuid::new_v4().to_string();
        let entry_count = entries.len() as i64;

        // Load all images — each entry passed dedup so every screenshot is a valid keyframe
        let mut images: Vec<Vec<u8>> = Vec::new();
        for entry in entries {
            let path = PathBuf::from(&entry.screenshot_path);
            if path.exists() {
                if let Ok(data) = std::fs::read(&path) {
                    images.push(data);
                }
            }
        }

        // Build context from this chunk's window titles
        let mut context_lines: Vec<String> = Vec::new();
        for entry in entries {
            context_lines.push(format!(
                "[{}] {} - {}",
                entry.timestamp, entry.app_name, entry.window_title
            ));
        }
        let context = context_lines.join("\n");

        let prompt_template = Self::load_extract_prompt();
        let prompt = prompt_template.replace("{activity_log}", &context);

        let summary = if !images.is_empty() && !llm_client.api_key_is_empty() {
            llm_client.send_multimodal(&prompt, &images).await?
        } else {
            format!(
                "Batch of {} screenshots. Apps used: {}",
                entry_count,
                entries
                    .iter()
                    .map(|e| e.app_name.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };

        let timestamp = Local::now().to_rfc3339();
        activity_log.insert_batch_summary(&batch_id, &timestamp, &summary, entry_count)?;

        let entry_ids: Vec<i64> = entries.iter().map(|e| e.id).collect();
        activity_log.mark_entries_batched(&entry_ids, &batch_id)?;

        // Delete this chunk's screenshots
        for entry in entries {
            let path = PathBuf::from(&entry.screenshot_path);
            let _ = screenshot_store.delete_screenshot(&path);
        }

        log::info!("Batch {} processed: {} entries, screenshots cleaned up", batch_id, entry_count);
        Ok(summary)
    }

    /// Generate the daily digest for today: process any remaining screenshots first,
    /// then summarize all batches into report.md.
    pub async fn generate_daily_digest(
        activity_log: &Arc<ActivityLog>,
        screenshot_store: &ScreenshotStore,
        llm_client: &LlmClient,
    ) -> Result<PathBuf> {
        let date = Local::now().format("%Y-%m-%d").to_string();
        Self::generate_digest_for_date(activity_log, screenshot_store, llm_client, &date).await
    }

    /// Generate the daily digest for a specific date: process remaining screenshots first,
    /// then summarize all batches into report.md.
    pub async fn generate_digest_for_date(
        activity_log: &Arc<ActivityLog>,
        screenshot_store: &ScreenshotStore,
        llm_client: &LlmClient,
        date: &str,
    ) -> Result<PathBuf> {
        // Only process remaining unbatched screenshots when generating for today
        let today = Local::now().format("%Y-%m-%d").to_string();
        if date == today {
            if let Some(_) = Self::process_batch(activity_log, screenshot_store, llm_client).await?
            {
                log::info!("Processed remaining screenshots before generating digest");
            }
        }

        let batches = activity_log.get_batches_for_date(date)?;
        let app_usage = activity_log.get_app_usage_for_date(date)?;

        let mut batch_text = String::new();
        for batch in &batches {
            batch_text.push_str(&format!(
                "## Batch at {}\n{}\n\n",
                batch.timestamp, batch.summary
            ));
        }

        let mut usage_text = String::new();
        for (app, count) in &app_usage {
            let minutes = count * 5 / 60;
            usage_text.push_str(&format!("- {}: ~{} min\n", app, minutes));
        }

        let prompt_template = Self::load_digest_prompt();
        let prompt = prompt_template
            .replace("{batch_summaries}", &batch_text)
            .replace("{app_usage}", &usage_text)
            .replace("{date}", date);

        let report = if !llm_client.api_key_is_empty() {
            llm_client.send_multimodal(&prompt, &[]).await?
        } else {
            format!(
                "# Daily Activity Report - {}\n\n## Summary\nTracked {} activity batches.\n\n## App Usage\n{}\n\n## Batch Details\n{}",
                date,
                batches.len(),
                usage_text,
                batch_text
            )
        };

        let report_path = screenshot_store.save_report_for_date(&report, date)?;

        // Clean up any remaining screenshot files for this date
        match screenshot_store.cleanup_screenshots_for_date(date) {
            Ok(count) if count > 0 => log::info!("Cleaned up {} leftover screenshots", count),
            Err(e) => log::warn!("Failed to clean up screenshots: {}", e),
            _ => {}
        }

        log::info!("Daily digest saved to {:?}", report_path);
        Ok(report_path)
    }
}

