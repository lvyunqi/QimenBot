//! Message deduplication based on message IDs with TTL-based expiry.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// TTL-based message deduplication filter that prevents duplicate events
/// from being processed within a configurable time window.
pub struct MessageDedup {
    seen: Mutex<HashMap<String, Instant>>,
    ttl: Duration,
    max_entries: usize,
}

impl MessageDedup {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_secs),
            max_entries,
        }
    }

    /// Returns true if this message_id has NOT been seen before (i.e., should be processed).
    /// Returns false if duplicate (should be skipped).
    pub async fn check_and_mark(&self, message_id: &str) -> bool {
        let mut seen = self.seen.lock().await;
        let now = Instant::now();

        // Check if already seen and not expired
        if let Some(timestamp) = seen.get(message_id) {
            if now.duration_since(*timestamp) < self.ttl {
                return false;
            }
        }

        // Evict expired entries if we've hit the cap
        if seen.len() >= self.max_entries {
            seen.retain(|_, ts| now.duration_since(*ts) < self.ttl);
        }

        // If still at capacity after cleanup, remove the oldest entry
        if seen.len() >= self.max_entries {
            if let Some(oldest_key) = seen
                .iter()
                .min_by_key(|(_, ts)| *ts)
                .map(|(k, _)| k.clone())
            {
                seen.remove(&oldest_key);
            }
        }

        seen.insert(message_id.to_string(), now);
        true
    }

    /// Periodically clean up expired entries.
    pub async fn cleanup(&self) {
        let mut seen = self.seen.lock().await;
        let now = Instant::now();
        let ttl = self.ttl;
        seen.retain(|_, ts| now.duration_since(*ts) < ttl);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn first_message_passes() {
        let dedup = MessageDedup::new(60, 1000);
        assert!(dedup.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn duplicate_message_blocked() {
        let dedup = MessageDedup::new(60, 1000);
        assert!(dedup.check_and_mark("msg-1").await);
        assert!(!dedup.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn different_messages_both_pass() {
        let dedup = MessageDedup::new(60, 1000);
        assert!(dedup.check_and_mark("msg-1").await);
        assert!(dedup.check_and_mark("msg-2").await);
    }

    #[tokio::test]
    async fn max_entries_evicts_oldest() {
        let dedup = MessageDedup::new(60, 2);
        assert!(dedup.check_and_mark("msg-1").await);
        assert!(dedup.check_and_mark("msg-2").await);
        // This should evict msg-1 (oldest) since all are still within TTL
        assert!(dedup.check_and_mark("msg-3").await);

        // msg-1 was evicted, so it should pass again
        assert!(dedup.check_and_mark("msg-1").await);
    }

    #[tokio::test]
    async fn cleanup_removes_expired() {
        let dedup = MessageDedup::new(0, 1000); // 0-second TTL = instant expiry
        assert!(dedup.check_and_mark("msg-1").await);
        dedup.cleanup().await;

        let seen = dedup.seen.lock().await;
        assert!(seen.is_empty());
    }
}
