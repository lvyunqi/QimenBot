use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use qimen_error::{QimenError, Result};
use qimen_plugin_api::Module;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Enabled,
    Disabled,
}

pub struct PluginEntry {
    pub module: Box<dyn Module>,
    pub state: PluginState,
}

/// Manages plugin lifecycle at runtime.
pub struct PluginManager {
    plugins: RwLock<HashMap<String, PluginEntry>>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// Register a module. The module is stored as [`PluginState::Enabled`] by default.
    pub async fn register(&self, module: Box<dyn Module>) {
        let id = module.id().to_string();
        tracing::info!(plugin_id = %id, "registering plugin");
        let mut plugins = self.plugins.write().await;
        plugins.insert(
            id,
            PluginEntry {
                module,
                state: PluginState::Enabled,
            },
        );
    }

    /// Enable a disabled plugin (calls `on_load`).
    pub async fn enable_plugin(&self, id: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .get_mut(id)
            .ok_or_else(|| QimenError::Runtime(format!("plugin not found: {}", id)))?;

        if entry.state == PluginState::Enabled {
            return Err(QimenError::Runtime(format!(
                "plugin already enabled: {}",
                id
            )));
        }

        entry.module.on_load().await?;
        entry.state = PluginState::Enabled;
        tracing::info!(plugin_id = %id, "plugin enabled");
        Ok(())
    }

    /// Disable an enabled plugin (calls `on_unload`).
    pub async fn disable_plugin(&self, id: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        let entry = plugins
            .get_mut(id)
            .ok_or_else(|| QimenError::Runtime(format!("plugin not found: {}", id)))?;

        if entry.state == PluginState::Disabled {
            return Err(QimenError::Runtime(format!(
                "plugin already disabled: {}",
                id
            )));
        }

        entry.module.on_unload().await?;
        entry.state = PluginState::Disabled;
        tracing::info!(plugin_id = %id, "plugin disabled");
        Ok(())
    }

    /// Restart a plugin (disable then enable).
    pub async fn restart_plugin(&self, id: &str) -> Result<()> {
        {
            let plugins = self.plugins.read().await;
            let entry = plugins
                .get(id)
                .ok_or_else(|| QimenError::Runtime(format!("plugin not found: {}", id)))?;

            if !entry.module.supports_hot_reload() {
                return Err(QimenError::Runtime(format!(
                    "plugin does not support hot reload: {}",
                    id
                )));
            }
        }

        // Disable then enable. If the plugin is already disabled we just enable it.
        {
            let mut plugins = self.plugins.write().await;
            let entry = plugins.get_mut(id).ok_or_else(|| {
                QimenError::Runtime(format!("plugin disappeared during restart: {}", id))
            })?;
            if entry.state == PluginState::Enabled {
                entry.module.on_unload().await?;
                entry.state = PluginState::Disabled;
            }
        }

        {
            let mut plugins = self.plugins.write().await;
            let entry = plugins.get_mut(id).ok_or_else(|| {
                QimenError::Runtime(format!("plugin disappeared during restart: {}", id))
            })?;
            entry.module.on_load().await?;
            entry.state = PluginState::Enabled;
        }

        tracing::info!(plugin_id = %id, "plugin restarted");
        Ok(())
    }

    /// Check if a plugin is enabled.
    pub async fn is_enabled(&self, id: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins
            .get(id)
            .map(|entry| entry.state == PluginState::Enabled)
            .unwrap_or(false)
    }

    /// List all plugins with their states.
    pub async fn list_plugins(&self) -> Vec<(String, PluginState)> {
        let plugins = self.plugins.read().await;
        let mut list: Vec<(String, PluginState)> = plugins
            .iter()
            .map(|(id, entry)| (id.clone(), entry.state.clone()))
            .collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Get all enabled plugins' command plugins.
    pub async fn collect_command_plugins(&self) -> Vec<Arc<dyn qimen_plugin_api::CommandPlugin>> {
        let plugins = self.plugins.read().await;
        let mut result = Vec::new();
        for entry in plugins.values() {
            if entry.state == PluginState::Enabled {
                result.extend(entry.module.command_plugins());
            }
        }
        result
    }

    /// Get all enabled plugins' system plugins.
    pub async fn collect_system_plugins(&self) -> Vec<Arc<dyn qimen_plugin_api::SystemPlugin>> {
        let plugins = self.plugins.read().await;
        let mut result = Vec::new();
        for entry in plugins.values() {
            if entry.state == PluginState::Enabled {
                result.extend(entry.module.system_plugins());
            }
        }
        result
    }
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use qimen_error::Result;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct TestModule {
        loaded: AtomicBool,
    }

    impl TestModule {
        fn new() -> Self {
            Self {
                loaded: AtomicBool::new(false),
            }
        }
    }

    #[async_trait]
    impl Module for TestModule {
        fn id(&self) -> &'static str {
            "test-module"
        }

        async fn on_load(&self) -> Result<()> {
            self.loaded.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn on_unload(&self) -> Result<()> {
            self.loaded.store(false, Ordering::SeqCst);
            Ok(())
        }

        fn supports_hot_reload(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn register_and_list() {
        let manager = PluginManager::new();
        manager.register(Box::new(TestModule::new())).await;

        let list = manager.list_plugins().await;
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "test-module");
        assert_eq!(list[0].1, PluginState::Enabled);
    }

    #[tokio::test]
    async fn disable_and_enable() {
        let manager = PluginManager::new();
        manager.register(Box::new(TestModule::new())).await;

        manager.disable_plugin("test-module").await.unwrap();
        assert!(!manager.is_enabled("test-module").await);

        manager.enable_plugin("test-module").await.unwrap();
        assert!(manager.is_enabled("test-module").await);
    }

    #[tokio::test]
    async fn restart_plugin() {
        let manager = PluginManager::new();
        manager.register(Box::new(TestModule::new())).await;

        manager.restart_plugin("test-module").await.unwrap();
        assert!(manager.is_enabled("test-module").await);
    }

    #[tokio::test]
    async fn disable_already_disabled_returns_error() {
        let manager = PluginManager::new();
        manager.register(Box::new(TestModule::new())).await;

        manager.disable_plugin("test-module").await.unwrap();
        assert!(manager.disable_plugin("test-module").await.is_err());
    }

    #[tokio::test]
    async fn enable_already_enabled_returns_error() {
        let manager = PluginManager::new();
        manager.register(Box::new(TestModule::new())).await;

        assert!(manager.enable_plugin("test-module").await.is_err());
    }

    #[tokio::test]
    async fn nonexistent_plugin_returns_error() {
        let manager = PluginManager::new();
        assert!(manager.enable_plugin("nope").await.is_err());
        assert!(manager.disable_plugin("nope").await.is_err());
        assert!(manager.restart_plugin("nope").await.is_err());
    }
}
