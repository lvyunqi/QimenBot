//! Per-plugin access control lists (user/group whitelist and blacklist).

use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;

/// Access control mode for a plugin
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AccessControlMode {
    /// No filtering - all events pass through
    #[default]
    Disabled,
    /// Only allow events from listed users/groups
    Whitelist,
    /// Block events from listed users/groups
    Blacklist,
}

/// Access control list for a single plugin
#[derive(Debug, Clone, Default)]
pub struct PluginAccessControl {
    pub mode: AccessControlMode,
    pub allowed_users: HashSet<i64>,
    pub allowed_groups: HashSet<i64>,
}

impl PluginAccessControl {
    /// Check if an event from this user/group should be processed
    pub fn should_process(&self, user_id: Option<i64>, group_id: Option<i64>) -> bool {
        match self.mode {
            AccessControlMode::Disabled => true,
            AccessControlMode::Whitelist => {
                let user_ok = user_id
                    .map(|id| self.allowed_users.contains(&id))
                    .unwrap_or(false);
                let group_ok = group_id
                    .map(|id| self.allowed_groups.contains(&id))
                    .unwrap_or(false);
                user_ok || group_ok
            }
            AccessControlMode::Blacklist => {
                let user_blocked = user_id
                    .map(|id| self.allowed_users.contains(&id))
                    .unwrap_or(false);
                let group_blocked = group_id
                    .map(|id| self.allowed_groups.contains(&id))
                    .unwrap_or(false);
                !user_blocked && !group_blocked
            }
        }
    }
}

/// Manages access control for all plugins
pub struct PluginAclManager {
    acls: RwLock<HashMap<String, PluginAccessControl>>,
}

impl PluginAclManager {
    pub fn new() -> Self {
        Self {
            acls: RwLock::new(HashMap::new()),
        }
    }

    /// Set access control for a plugin
    pub async fn set_acl(&self, plugin_id: &str, acl: PluginAccessControl) {
        let mut acls = self.acls.write().await;
        acls.insert(plugin_id.to_string(), acl);
    }

    /// Get access control for a plugin (returns Disabled if not set)
    pub async fn get_acl(&self, plugin_id: &str) -> PluginAccessControl {
        let acls = self.acls.read().await;
        acls.get(plugin_id).cloned().unwrap_or_default()
    }

    /// Set the mode for a plugin
    pub async fn set_mode(&self, plugin_id: &str, mode: AccessControlMode) {
        let mut acls = self.acls.write().await;
        acls.entry(plugin_id.to_string())
            .or_default()
            .mode = mode;
    }

    /// Add a user to a plugin's list
    pub async fn add_user(&self, plugin_id: &str, user_id: i64) {
        let mut acls = self.acls.write().await;
        acls.entry(plugin_id.to_string())
            .or_default()
            .allowed_users
            .insert(user_id);
    }

    /// Remove a user from a plugin's list
    pub async fn remove_user(&self, plugin_id: &str, user_id: i64) {
        let mut acls = self.acls.write().await;
        if let Some(acl) = acls.get_mut(plugin_id) {
            acl.allowed_users.remove(&user_id);
        }
    }

    /// Add a group to a plugin's list
    pub async fn add_group(&self, plugin_id: &str, group_id: i64) {
        let mut acls = self.acls.write().await;
        acls.entry(plugin_id.to_string())
            .or_default()
            .allowed_groups
            .insert(group_id);
    }

    /// Remove a group from a plugin's list
    pub async fn remove_group(&self, plugin_id: &str, group_id: i64) {
        let mut acls = self.acls.write().await;
        if let Some(acl) = acls.get_mut(plugin_id) {
            acl.allowed_groups.remove(&group_id);
        }
    }

    /// Check if an event should be processed by this plugin
    pub async fn should_process(
        &self,
        plugin_id: &str,
        user_id: Option<i64>,
        group_id: Option<i64>,
    ) -> bool {
        let acls = self.acls.read().await;
        match acls.get(plugin_id) {
            Some(acl) => acl.should_process(user_id, group_id),
            None => true,
        }
    }
}

impl Default for PluginAclManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_mode_allows_all() {
        let manager = PluginAclManager::new();
        // No ACL set => default Disabled mode
        assert!(manager.should_process("test_plugin", Some(123), Some(456)).await);
        assert!(manager.should_process("test_plugin", None, None).await);

        // Explicitly set Disabled
        manager
            .set_acl(
                "test_plugin",
                PluginAccessControl {
                    mode: AccessControlMode::Disabled,
                    allowed_users: HashSet::new(),
                    allowed_groups: HashSet::new(),
                },
            )
            .await;
        assert!(manager.should_process("test_plugin", Some(999), Some(888)).await);
    }

    #[tokio::test]
    async fn whitelist_mode_filters() {
        let manager = PluginAclManager::new();
        manager.set_mode("plugin_a", AccessControlMode::Whitelist).await;
        manager.add_user("plugin_a", 100).await;
        manager.add_group("plugin_a", 200).await;

        // Allowed user
        assert!(manager.should_process("plugin_a", Some(100), None).await);
        // Allowed group
        assert!(manager.should_process("plugin_a", None, Some(200)).await);
        // Both allowed
        assert!(manager.should_process("plugin_a", Some(100), Some(200)).await);
        // Not in list
        assert!(!manager.should_process("plugin_a", Some(999), None).await);
        assert!(!manager.should_process("plugin_a", None, Some(999)).await);
        assert!(!manager.should_process("plugin_a", None, None).await);

        // Remove user, now only group works
        manager.remove_user("plugin_a", 100).await;
        assert!(!manager.should_process("plugin_a", Some(100), None).await);
        assert!(manager.should_process("plugin_a", None, Some(200)).await);
    }

    #[tokio::test]
    async fn blacklist_mode_filters() {
        let manager = PluginAclManager::new();
        manager.set_mode("plugin_b", AccessControlMode::Blacklist).await;
        manager.add_user("plugin_b", 100).await;
        manager.add_group("plugin_b", 200).await;

        // Blocked user
        assert!(!manager.should_process("plugin_b", Some(100), None).await);
        // Blocked group
        assert!(!manager.should_process("plugin_b", None, Some(200)).await);
        // Not blocked
        assert!(manager.should_process("plugin_b", Some(999), None).await);
        assert!(manager.should_process("plugin_b", None, Some(999)).await);
        assert!(manager.should_process("plugin_b", None, None).await);

        // Remove blocked group
        manager.remove_group("plugin_b", 200).await;
        assert!(manager.should_process("plugin_b", None, Some(200)).await);
    }
}
