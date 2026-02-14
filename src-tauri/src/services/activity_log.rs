use anyhow::Result;
use chrono::Local;
use rusqlite::{params, Connection};
use std::path::PathBuf;
use std::sync::Mutex;

const SCHEMA: &str = "
    CREATE TABLE IF NOT EXISTS activity_log (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp TEXT NOT NULL,
        screenshot_path TEXT NOT NULL,
        window_title TEXT NOT NULL DEFAULT '',
        app_name TEXT NOT NULL DEFAULT '',
        image_hash TEXT NOT NULL DEFAULT '',
        batch_id TEXT
    );
    CREATE TABLE IF NOT EXISTS llm_batches (
        id TEXT PRIMARY KEY,
        timestamp TEXT NOT NULL,
        summary TEXT NOT NULL DEFAULT '',
        entry_count INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX IF NOT EXISTS idx_activity_batch ON activity_log(batch_id);
    CREATE INDEX IF NOT EXISTS idx_activity_timestamp ON activity_log(timestamp);
";

/// Per-day activity database. Each day folder (`data/YYYY-MM-DD/`) gets its own `activity.db`.
pub struct ActivityLog {
    base_dir: PathBuf,
    conn: Mutex<Connection>,
    current_date: Mutex<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ActivityEntry {
    pub id: i64,
    pub timestamp: String,
    pub screenshot_path: String,
    pub window_title: String,
    pub app_name: String,
    pub image_hash: String,
    pub batch_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchSummary {
    pub id: String,
    pub timestamp: String,
    pub summary: String,
    pub entry_count: i64,
}

fn open_day_db(base_dir: &PathBuf, date: &str) -> Result<Connection> {
    let day_dir = base_dir.join(date);
    std::fs::create_dir_all(&day_dir)?;
    let db_path = day_dir.join("activity.db");
    let conn = Connection::open(db_path)?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

fn today_str() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

impl ActivityLog {
    pub fn new(base_dir: &PathBuf) -> Result<Self> {
        std::fs::create_dir_all(base_dir)?;
        let date = today_str();
        let conn = open_day_db(base_dir, &date)?;

        Ok(Self {
            base_dir: base_dir.clone(),
            conn: Mutex::new(conn),
            current_date: Mutex::new(date),
        })
    }

    /// Ensure we're using today's database; roll over if the date changed.
    pub fn ensure_today(&self) -> Result<()> {
        let today = today_str();
        let mut current = self.current_date.lock().unwrap();
        if *current != today {
            let new_conn = open_day_db(&self.base_dir, &today)?;
            let mut conn = self.conn.lock().unwrap();
            *conn = new_conn;
            *current = today;
            log::info!("Rolled over to new day database: {}", &*current);
        }
        Ok(())
    }

    /// Open (or create) the database for a specific date.
    pub fn open_for_date(&self, date: &str) -> Result<Connection> {
        open_day_db(&self.base_dir, date)
    }

    pub fn insert_activity(
        &self,
        timestamp: &str,
        screenshot_path: &str,
        window_title: &str,
        app_name: &str,
        image_hash: &str,
    ) -> Result<i64> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO activity_log (timestamp, screenshot_path, window_title, app_name, image_hash)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![timestamp, screenshot_path, window_title, app_name, image_hash],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_unbatched_entries(&self) -> Result<Vec<ActivityEntry>> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, screenshot_path, window_title, app_name, image_hash, batch_id
             FROM activity_log WHERE batch_id IS NULL ORDER BY timestamp ASC",
        )?;
        let entries = stmt
            .query_map([], |row| {
                Ok(ActivityEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    screenshot_path: row.get(2)?,
                    window_title: row.get(3)?,
                    app_name: row.get(4)?,
                    image_hash: row.get(5)?,
                    batch_id: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn mark_entries_batched(&self, entry_ids: &[i64], batch_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        for id in entry_ids {
            conn.execute(
                "UPDATE activity_log SET batch_id = ?1 WHERE id = ?2",
                params![batch_id, id],
            )?;
        }
        Ok(())
    }

    pub fn insert_batch_summary(
        &self,
        batch_id: &str,
        timestamp: &str,
        summary: &str,
        entry_count: i64,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO llm_batches (id, timestamp, summary, entry_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![batch_id, timestamp, summary, entry_count],
        )?;
        Ok(())
    }

    pub fn get_batches(&self) -> Result<Vec<BatchSummary>> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, summary, entry_count
             FROM llm_batches ORDER BY timestamp ASC",
        )?;
        let batches = stmt
            .query_map([], |row| {
                Ok(BatchSummary {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    summary: row.get(2)?,
                    entry_count: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(batches)
    }

    /// Get batches from a specific date's database.
    pub fn get_batches_for_date(&self, date: &str) -> Result<Vec<BatchSummary>> {
        let conn = self.open_for_date(date)?;
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, summary, entry_count
             FROM llm_batches ORDER BY timestamp ASC",
        )?;
        let batches = stmt
            .query_map([], |row| {
                Ok(BatchSummary {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    summary: row.get(2)?,
                    entry_count: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(batches)
    }

    pub fn get_app_usage(&self) -> Result<Vec<(String, i64)>> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT app_name, COUNT(*) as count
             FROM activity_log GROUP BY app_name ORDER BY count DESC",
        )?;
        let usage = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(usage)
    }

    /// Get app usage from a specific date's database.
    pub fn get_app_usage_for_date(&self, date: &str) -> Result<Vec<(String, i64)>> {
        let conn = self.open_for_date(date)?;
        let mut stmt = conn.prepare(
            "SELECT app_name, COUNT(*) as count
             FROM activity_log GROUP BY app_name ORDER BY count DESC",
        )?;
        let usage = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(usage)
    }

    pub fn get_screenshot_count(&self) -> Result<i64> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM activity_log",
            [],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    pub fn get_last_batch_time(&self) -> Result<Option<String>> {
        self.ensure_today()?;
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT timestamp FROM llm_batches ORDER BY timestamp DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(ts) => Ok(Some(ts)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}
