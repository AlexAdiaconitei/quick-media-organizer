use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub timestamp: String,
    pub source: String,
    pub level: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack: Option<String>,
}

/// The log is a debugging aid, not an audit trail: it keeps the most recent
/// entries and forgets the rest, so a long session cannot grow it without
/// bound in memory or on disk.
const MAX_ENTRIES: usize = 500;
const MAX_LOG_BYTES: u64 = 1_000_000;

pub struct ErrorLog {
    paths: Vec<PathBuf>,
    entries: Mutex<Vec<ErrorEntry>>,
}

impl ErrorLog {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let mut paths = vec![app_data_dir.join("error-log.jsonl")];

        if cfg!(debug_assertions) {
            if let Ok(cwd) = std::env::current_dir() {
                paths.push(cwd.join("logs").join("app-errors.jsonl"));
            }
        }

        for path in &paths {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let mut loaded = Vec::new();
        if let Some(primary) = paths.first() {
            loaded = Self::read_file(primary);
        }

        Self {
            paths,
            entries: Mutex::new(loaded),
        }
    }

    pub fn report(
        &self,
        source: &str,
        level: &str,
        message: &str,
        context: Option<serde_json::Value>,
        stack: Option<String>,
    ) -> Result<ErrorEntry, String> {
        let entry = ErrorEntry {
            timestamp: Utc::now().to_rfc3339(),
            source: source.to_string(),
            level: level.to_string(),
            message: message.to_string(),
            context,
            stack,
        };

        let line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;

        for path in &self.paths {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| e.to_string())?;
            writeln!(file, "{line}").map_err(|e| e.to_string())?;
        }

        {
            let mut entries = self.entries.lock().map_err(|e| e.to_string())?;
            entries.push(entry.clone());
            if entries.len() > MAX_ENTRIES {
                let overflow = entries.len() - MAX_ENTRIES;
                entries.drain(0..overflow);
            }
            self.rotate_oversized_files(&entries);
        }

        eprintln!("[QMO][{}] {}: {}", entry.source, entry.level, entry.message);

        Ok(entry)
    }

    /// Rewrites any log file that outgrew the cap from the entries still held
    /// in memory. Best effort: a log that cannot be trimmed is not worth
    /// failing the command that reported the error.
    fn rotate_oversized_files(&self, entries: &[ErrorEntry]) {
        let oversized = self
            .paths
            .iter()
            .any(|path| fs::metadata(path).map(|meta| meta.len()).unwrap_or(0) > MAX_LOG_BYTES);
        if !oversized {
            return;
        }

        let mut content = String::new();
        for entry in entries {
            if let Ok(line) = serde_json::to_string(entry) {
                content.push_str(&line);
                content.push('\n');
            }
        }
        for path in &self.paths {
            let _ = fs::write(path, &content);
        }
    }

    pub fn list(&self) -> Result<Vec<ErrorEntry>, String> {
        Ok(self.entries.lock().map_err(|e| e.to_string())?.clone())
    }

    pub fn clear(&self) -> Result<(), String> {
        for path in &self.paths {
            if path.exists() {
                fs::write(path, "").map_err(|e| e.to_string())?;
            }
        }
        self.entries.lock().map_err(|e| e.to_string())?.clear();
        Ok(())
    }

    pub fn log_path(&self) -> String {
        self.paths
            .last()
            .or_else(|| self.paths.first())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn read_file(path: &PathBuf) -> Vec<ErrorEntry> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut entries: Vec<ErrorEntry> = content
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    return None;
                }
                serde_json::from_str(trimmed).ok()
            })
            .collect();

        if entries.len() > MAX_ENTRIES {
            let overflow = entries.len() - MAX_ENTRIES;
            entries.drain(0..overflow);
        }
        entries
    }
}

pub type SharedErrorLog = Mutex<ErrorLog>;
