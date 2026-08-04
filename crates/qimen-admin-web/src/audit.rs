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

#[derive(Debug, Clone, Serialize)]
pub struct AuditPage {
    pub entries: Vec<AuditEntry>,
    pub pagination: AuditPagination,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditPagination {
    pub page: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub total_pages: usize,
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

    pub fn page(&self, requested_page: usize, requested_page_size: usize) -> AuditPage {
        let page_size = requested_page_size.clamp(1, 100);
        let (total_items, entries) = self
            .inner
            .entries
            .read()
            .map(|entries| {
                let total_items = entries.len();
                let total_pages = total_items.div_ceil(page_size).max(1);
                let page = requested_page.max(1).min(total_pages);
                let offset = (page - 1).saturating_mul(page_size).min(total_items);
                let page_entries = entries
                    .iter()
                    .rev()
                    .skip(offset)
                    .take(page_size)
                    .cloned()
                    .collect();
                (total_items, page_entries)
            })
            .unwrap_or_else(|_| (0, Vec::new()));
        let total_pages = total_items.div_ceil(page_size).max(1);
        let page = requested_page.max(1).min(total_pages);

        AuditPage {
            entries,
            pagination: AuditPagination {
                page,
                page_size,
                total_items,
                total_pages,
            },
        }
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

#[cfg(test)]
mod tests {
    use super::AuditLog;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("qimen-audit-{label}-{nonce}.jsonl"))
    }

    #[test]
    fn page_returns_newest_entries_and_clamps_page() {
        let path = temp_path("pagination");
        let log = AuditLog::open(&path);
        for index in 0..5 {
            log.record("test", format!("resource-{index}"), "success", "detail")
                .unwrap();
        }

        let page = log.page(2, 2);
        assert_eq!(page.pagination.page, 2);
        assert_eq!(page.pagination.total_items, 5);
        assert_eq!(page.pagination.total_pages, 3);
        assert_eq!(page.entries.len(), 2);
        assert_eq!(page.entries[0].resource, "resource-2");

        let last = log.page(99, 2);
        assert_eq!(last.pagination.page, 3);
        assert_eq!(last.entries.len(), 1);

        let minimum = log.page(1, 0);
        assert_eq!(minimum.pagination.page_size, 1);
        assert_eq!(minimum.pagination.total_pages, 5);

        let maximum = log.page(1, usize::MAX);
        assert_eq!(maximum.pagination.page_size, 100);
        assert_eq!(maximum.entries.len(), 5);
        let _ = fs::remove_file(path);
    }
}
