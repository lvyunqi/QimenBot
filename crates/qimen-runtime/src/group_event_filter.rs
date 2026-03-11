//! Group-level event filtering with whitelist/blacklist modes.

use std::collections::HashSet;
use tokio::sync::RwLock;

/// Filtering strategy for group events.
pub enum GroupFilterMode {
    /// Only process events from listed groups.
    Whitelist,
    /// Process events from all groups EXCEPT listed ones.
    Blacklist,
    /// No filtering, process all events.
    Disabled,
}

/// Runtime filter that decides whether events from a given group should be processed.
pub struct GroupEventFilter {
    mode: RwLock<GroupFilterMode>,
    groups: RwLock<HashSet<i64>>,
}

impl GroupEventFilter {
    pub fn new(mode: GroupFilterMode) -> Self {
        Self {
            mode: RwLock::new(mode),
            groups: RwLock::new(HashSet::new()),
        }
    }

    pub fn disabled() -> Self {
        Self::new(GroupFilterMode::Disabled)
    }

    /// Returns true if the event from this group should be processed.
    ///
    /// Private messages (`group_id = None`) always pass through regardless of filter mode.
    pub async fn should_process(&self, group_id: Option<i64>) -> bool {
        let gid = match group_id {
            Some(id) => id,
            None => return true, // private messages always pass
        };

        let mode = self.mode.read().await;
        let groups = self.groups.read().await;

        match *mode {
            GroupFilterMode::Disabled => true,
            GroupFilterMode::Whitelist => groups.contains(&gid),
            GroupFilterMode::Blacklist => !groups.contains(&gid),
        }
    }

    pub async fn add_group(&self, group_id: i64) {
        self.groups.write().await.insert(group_id);
    }

    pub async fn remove_group(&self, group_id: i64) {
        self.groups.write().await.remove(&group_id);
    }

    pub async fn set_mode(&self, mode: GroupFilterMode) {
        *self.mode.write().await = mode;
    }

    pub async fn set_groups(&self, groups: Vec<i64>) {
        *self.groups.write().await = groups.into_iter().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_mode_allows_all() {
        let filter = GroupEventFilter::disabled();
        assert!(filter.should_process(Some(12345)).await);
        assert!(filter.should_process(Some(99999)).await);
        assert!(filter.should_process(None).await);
    }

    #[tokio::test]
    async fn whitelist_mode_filters() {
        let filter = GroupEventFilter::new(GroupFilterMode::Whitelist);
        filter.add_group(100).await;
        filter.add_group(200).await;

        assert!(filter.should_process(Some(100)).await);
        assert!(filter.should_process(Some(200)).await);
        assert!(!filter.should_process(Some(300)).await);
    }

    #[tokio::test]
    async fn blacklist_mode_filters() {
        let filter = GroupEventFilter::new(GroupFilterMode::Blacklist);
        filter.add_group(100).await;

        assert!(!filter.should_process(Some(100)).await);
        assert!(filter.should_process(Some(200)).await);
    }

    #[tokio::test]
    async fn private_messages_always_pass() {
        let filter = GroupEventFilter::new(GroupFilterMode::Whitelist);
        // No groups added, but private messages should still pass
        assert!(filter.should_process(None).await);
    }

    #[tokio::test]
    async fn set_groups_replaces_all() {
        let filter = GroupEventFilter::new(GroupFilterMode::Whitelist);
        filter.add_group(100).await;
        filter.set_groups(vec![200, 300]).await;

        assert!(!filter.should_process(Some(100)).await);
        assert!(filter.should_process(Some(200)).await);
        assert!(filter.should_process(Some(300)).await);
    }

    #[tokio::test]
    async fn remove_group_works() {
        let filter = GroupEventFilter::new(GroupFilterMode::Whitelist);
        filter.add_group(100).await;
        assert!(filter.should_process(Some(100)).await);

        filter.remove_group(100).await;
        assert!(!filter.should_process(Some(100)).await);
    }

    #[tokio::test]
    async fn set_mode_changes_behavior() {
        let filter = GroupEventFilter::new(GroupFilterMode::Whitelist);
        filter.add_group(100).await;

        assert!(filter.should_process(Some(100)).await);
        assert!(!filter.should_process(Some(200)).await);

        filter.set_mode(GroupFilterMode::Blacklist).await;

        assert!(!filter.should_process(Some(100)).await);
        assert!(filter.should_process(Some(200)).await);
    }
}
