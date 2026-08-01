use crate::error::AdminError;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: String,
    pub action: String,
    pub resource: String,
    pub outcome: String,
    pub detail: String,
}

#[derive(Clone)]
pub struct AuditLog {
    inner: Arc<AuditLogInner>,
}

struct AuditLogInner {
    path: PathBuf,
    entries: RwLock<VecDeque<AuditEntry>>,
    file_lock: Mutex<()>,
}

impl AuditLog {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let entries = load_entries(&path);
        Self {
            inner: Arc::new(AuditLogInner {
                path,
                entries: RwLock::new(entries),
                file_lock: Mutex::new(()),
            }),
        }
    }

    pub fn entries(&self, limit: usize) -> Vec<AuditEntry> {
        self.inner
            .entries
            .read()
            .map(|entries| entries.iter().rev().take(limit).cloned().collect())
            .unwrap_or_default()
    }

    pub fn record(
        &self,
        action: impl Into<String>,
        resource: impl Into<String>,
        outcome: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<AuditEntry, AdminError> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let entry = AuditEntry {
            id: format!("audit-{}", Utc::now().timestamp_micros()),
            timestamp,
            action: action.into(),
            resource: resource.into(),
            outcome: outcome.into(),
            detail: detail.into(),
        };

        let _guard = self
            .inner
            .file_lock
            .lock()
            .map_err(|_| AdminError::Internal("audit log lock is poisoned".to_string()))?;
        if let Some(parent) = self.inner.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inner.path)?;
        serde_json::to_writer(&mut file, &entry).map_err(AdminError::internal)?;
        file.write_all(b"\n")?;
        file.flush()?;

        if let Ok(mut entries) = self.inner.entries.write() {
            entries.push_back(entry.clone());
            while entries.len() > 1_000 {
                entries.pop_front();
            }
        }
        Ok(entry)
    }
}

fn load_entries(path: &PathBuf) -> VecDeque<AuditEntry> {
    let Ok(file) = fs::File::open(path) else {
        return VecDeque::new();
    };
    let entries: Vec<_> = BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<AuditEntry>(&line).ok())
        .collect();
    entries
        .into_iter()
        .rev()
        .take(1_000)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
