use chrono::{SecondsFormat, Utc};
use qimen_error::{QimenError, Result};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::io::IsTerminal;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt};

#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone)]
pub struct LogStore {
    inner: Arc<LogStoreInner>,
}

struct LogStoreInner {
    capacity: usize,
    next_id: AtomicU64,
    entries: RwLock<VecDeque<LogEntry>>,
    sender: broadcast::Sender<LogEntry>,
}

impl LogStore {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let (sender, _) = broadcast::channel(capacity.min(4_096));
        Self {
            inner: Arc::new(LogStoreInner {
                capacity,
                next_id: AtomicU64::new(1),
                entries: RwLock::new(VecDeque::with_capacity(capacity)),
                sender,
            }),
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity
    }

    pub fn len(&self) -> usize {
        self.inner
            .entries
            .read()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.inner
            .entries
            .read()
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.inner.sender.subscribe()
    }

    fn push(&self, mut entry: LogEntry) {
        entry.id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut entries) = self.inner.entries.write() {
            if entries.len() == self.inner.capacity {
                entries.pop_front();
            }
            entries.push_back(entry.clone());
        }
        let _ = self.inner.sender.send(entry);
    }
}

#[derive(Clone)]
struct LogStoreLayer {
    store: LogStore,
}

impl<S> Layer<S> for LogStoreLayer
where
    S: Subscriber,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        let mut visitor = FieldVisitor::default();
        event.record(&mut visitor);
        let message = visitor
            .fields
            .remove("message")
            .unwrap_or_else(|| metadata.name().to_string());
        self.store.push(LogEntry {
            id: 0,
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            level: metadata.level().as_str().to_ascii_lowercase(),
            target: metadata.target().to_string(),
            message,
            fields: visitor.fields,
        });
    }
}

#[derive(Default)]
struct FieldVisitor {
    fields: BTreeMap<String, String>,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        let value = format!("{value:?}");
        self.fields
            .insert(field.name().to_string(), trim_debug_string(value));
    }
}

fn trim_debug_string(value: String) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(&value)
        .replace("\\\"", "\"")
}

pub fn init(level: &str, json_logs: bool) -> Result<()> {
    init_with_log_store(level, json_logs, None)
}

pub fn init_with_log_store(
    level: &str,
    json_logs: bool,
    log_store: Option<LogStore>,
) -> Result<()> {
    let filter =
        EnvFilter::try_new(level).map_err(|error| QimenError::Runtime(error.to_string()))?;
    let capture_layer = log_store.map(|store| LogStoreLayer { store });

    if json_logs {
        Registry::default()
            .with(filter)
            .with(fmt::layer().json())
            .with(capture_layer)
            .try_init()
            .map_err(|error| QimenError::Runtime(error.to_string()))
    } else {
        Registry::default()
            .with(filter)
            .with(fmt::layer().with_ansi(terminal_ansi_enabled()))
            .with(capture_layer)
            .try_init()
            .map_err(|error| QimenError::Runtime(error.to_string()))
    }
}

fn terminal_ansi_enabled() -> bool {
    should_emit_ansi(
        std::io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    )
}

fn should_emit_ansi(is_terminal: bool, no_color: bool) -> bool {
    is_terminal && !no_color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_keeps_the_newest_entries() {
        let store = LogStore::new(2);
        for message in ["one", "two", "three"] {
            store.push(LogEntry {
                id: 0,
                timestamp: String::new(),
                level: "info".to_string(),
                target: "test".to_string(),
                message: message.to_string(),
                fields: BTreeMap::new(),
            });
        }
        let messages: Vec<_> = store
            .entries()
            .into_iter()
            .map(|entry| entry.message)
            .collect();
        assert_eq!(messages, ["two", "three"]);
    }

    #[test]
    fn ansi_is_used_only_for_an_interactive_terminal() {
        assert!(should_emit_ansi(true, false));
        assert!(!should_emit_ansi(false, false));
        assert!(!should_emit_ansi(true, true));
    }
}
