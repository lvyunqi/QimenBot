//! Message deduplication based on message IDs with TTL-based expiry.

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// TTL-based message deduplication filter that prevents duplicate events
/// from being processed within a configurable time window.
pub struct MessageDedup {
    seen: Mutex<SeenMessages>,
    ttl: Duration,
    max_entries: usize,
}

#[derive(Default)]
struct SeenMessages {
    ids: HashSet<Arc<str>>,
    // Insertion order is also expiry order because every entry has the same TTL.
    order: VecDeque<(Arc<str>, Instant)>,
}

impl SeenMessages {
    fn evict_oldest(&mut self) {
        if let Some((id, _)) = self.order.pop_front() {
            self.ids.remove(&id);
        }
    }

    fn evict_expired(&mut self, now: Instant, ttl: Duration) {
        while self
            .order
            .front()
            .is_some_and(|(_, inserted)| now.duration_since(*inserted) >= ttl)
        {
            self.evict_oldest();
        }
    }

    fn check_and_mark(
        &mut self,
        id: &str,
        now: Instant,
        ttl: Duration,
        max_entries: usize,
    ) -> bool {
        self.evict_expired(now, ttl);
        if self.ids.contains(id) {
            return false;
        }
        if self.ids.len() >= max_entries {
            self.evict_oldest();
        }

        let id = Arc::<str>::from(id);
        self.ids.insert(Arc::clone(&id));
        self.order.push_back((id, now));
        true
    }
}

impl MessageDedup {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            seen: Mutex::new(SeenMessages::default()),
            ttl: Duration::from_secs(ttl_secs),
            max_entries,
        }
    }

    /// Returns true if this message_id has NOT been seen before (i.e., should be processed).
    /// Returns false if duplicate (should be skipped).
    pub async fn check_and_mark(&self, message_id: &str) -> bool {
        self.seen.lock().await.check_and_mark(
            message_id,
            Instant::now(),
            self.ttl,
            self.max_entries,
        )
    }

    /// Periodically clean up expired entries.
    pub async fn cleanup(&self) {
        self.seen
            .lock()
            .await
            .evict_expired(Instant::now(), self.ttl);
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
        assert!(seen.ids.is_empty());
        assert!(seen.order.is_empty());
    }

    #[test]
    fn duplicate_does_not_refresh_ttl_and_reinserted_id_gets_new_expiry() {
        let mut seen = SeenMessages::default();
        let start = Instant::now();
        let ttl = Duration::from_secs(10);
        let later = start + ttl / 2;

        assert!(seen.check_and_mark("first", start, ttl, 2));
        assert!(!seen.check_and_mark("first", later, ttl, 2));
        assert!(seen.check_and_mark("second", later, ttl, 2));
        assert!(seen.check_and_mark("first", start + ttl, ttl, 2));

        seen.evict_expired(later + ttl, ttl);
        assert!(!seen.check_and_mark("first", later + ttl, ttl, 2));
        assert!(seen.check_and_mark("second", later + ttl, ttl, 2));
    }
}
