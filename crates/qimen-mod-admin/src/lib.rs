use async_trait::async_trait;
use qimen_error::Result;
use qimen_plugin_api::Module;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdminConfig {
    pub super_admins: Vec<i64>,
    pub admins: Vec<i64>,
    pub blocked_users: Vec<i64>,
    pub blocked_groups: Vec<i64>,
}

pub struct AdminManager {
    config: RwLock<AdminConfig>,
}

impl AdminManager {
    pub fn new(config: AdminConfig) -> Self {
        Self {
            config: RwLock::new(config),
        }
    }

    pub fn is_super_admin(&self, user_id: i64) -> bool {
        let config = self.config.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        config.super_admins.contains(&user_id)
    }

    pub fn is_admin(&self, user_id: i64) -> bool {
        let config = self.config.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        config.super_admins.contains(&user_id) || config.admins.contains(&user_id)
    }

    pub fn is_blocked_user(&self, user_id: i64) -> bool {
        let config = self.config.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        config.blocked_users.contains(&user_id)
    }

    pub fn is_blocked_group(&self, group_id: i64) -> bool {
        let config = self.config.read().unwrap_or_else(|poisoned| poisoned.into_inner());
        config.blocked_groups.contains(&group_id)
    }

    pub fn check_permission(&self, user_id: i64, required_role: &str) -> bool {
        match required_role {
            "super_admin" => self.is_super_admin(user_id),
            "admin" => self.is_admin(user_id),
            _ => false,
        }
    }
}

pub struct AdminModule {
    manager: Arc<AdminManager>,
}

impl AdminModule {
    pub fn new(config: AdminConfig) -> Self {
        Self {
            manager: Arc::new(AdminManager::new(config)),
        }
    }

    pub fn manager(&self) -> Arc<AdminManager> {
        Arc::clone(&self.manager)
    }
}

impl Default for AdminModule {
    fn default() -> Self {
        Self::new(AdminConfig::default())
    }
}

#[async_trait]
impl Module for AdminModule {
    fn id(&self) -> &'static str {
        "admin"
    }

    async fn on_load(&self) -> Result<()> {
        tracing::info!("admin module loaded");
        Ok(())
    }
}
