use qimen_error::{QimenError, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const BUILTIN_PLUGIN_PRIORITY: u32 = 10;
pub const DYNAMIC_PLUGIN_PRIORITY: u32 = 20;
pub const STATIC_PLUGIN_PRIORITY: u32 = 30;
pub const MAX_PLUGIN_PRIORITY: u32 = 1_000;

pub fn default_plugin_priority(kind: &str) -> u32 {
    match kind {
        "builtin" => BUILTIN_PLUGIN_PRIORITY,
        "dynamic" => DYNAMIC_PLUGIN_PRIORITY,
        _ => STATIC_PLUGIN_PRIORITY,
    }
}

#[derive(Debug, Clone)]
pub struct HostPluginReport {
    pub builtin_modules: Vec<String>,
    pub configured_plugins: Vec<String>,
    pub available_modules: Vec<HostModuleReportEntry>,
    pub persisted_states: BTreeMap<String, bool>,
    pub dynamic_plugins: Vec<DynamicPluginReportEntry>,
}

#[derive(Debug, Clone)]
pub struct HostModuleReportEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub api_version: String,
    pub commands: Vec<String>,
    pub system_plugins: Vec<String>,
    pub interceptors: usize,
}

#[derive(Debug, Clone)]
pub struct DynamicPluginReportEntry {
    pub path: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub api_version: String,
    /// v0.2: Multiple commands per plugin.
    pub commands: Vec<DynamicCommandEntry>,
    /// v0.2: Multiple event routes per plugin.
    pub routes: Vec<DynamicRouteEntry>,
    /// Interceptor entries registered by this plugin.
    pub interceptors: Vec<DynamicInterceptorEntry>,
    /// API 0.5+ exact HTTP webhook routes exported by this plugin.
    pub webhooks: Vec<DynamicWebhookEntry>,
    /// API 0.6 可选的在线配置表单契约。
    pub config: Option<DynamicPluginConfigEntry>,

    // ── v0.1 legacy fields (kept for backward compatibility) ──
    pub command_name: String,
    pub command_description: String,
    pub callback_symbol: String,
    pub notice_route: String,
    pub notice_callback_symbol: String,
    pub request_route: String,
    pub request_callback_symbol: String,
    pub meta_route: String,
    pub meta_callback_symbol: String,
}

/// A single command registered by a dynamic plugin.
#[derive(Debug, Clone)]
pub struct DynamicCommandEntry {
    pub name: String,
    pub description: String,
    pub callback_symbol: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub required_role: String,
    pub scope: String,
}

/// A single event route registered by a dynamic plugin.
#[derive(Debug, Clone)]
pub struct DynamicRouteEntry {
    /// "notice", "request", or "meta".
    pub kind: String,
    /// Route name(s), e.g. "GroupPoke" or "GroupPoke,PrivatePoke".
    pub route: String,
    pub callback_symbol: String,
}

/// A single interceptor entry registered by a dynamic plugin.
#[derive(Debug, Clone)]
pub struct DynamicInterceptorEntry {
    pub pre_handle_symbol: String,
    pub after_completion_symbol: String,
}

/// 动态插件导出的配置 Schema 与回调能力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicPluginConfigEntry {
    pub config_version: u32,
    pub apply_mode: String,
    pub schema_json: String,
    pub ui_schema_json: String,
    pub validates_config: bool,
    pub applies_live: bool,
}

/// A framework-hosted HTTP webhook exported by an API 0.5+ dynamic plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicWebhookEntry {
    pub method: String,
    pub path: String,
    pub callback_symbol: String,
}

/// Runtime-ready webhook descriptor including its owning library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicWebhookDescriptor {
    pub plugin_id: String,
    pub library_path: String,
    pub method: String,
    pub path: String,
    pub callback_symbol: String,
}

/// Descriptor for a dynamic plugin interceptor, used by the runtime.
#[derive(Debug, Clone)]
pub struct DynamicInterceptorDescriptor {
    pub plugin_id: String,
    pub library_path: String,
    pub pre_handle_symbol: String,
    pub after_completion_symbol: String,
}

#[derive(Debug, Clone)]
pub struct DynamicCommandDescriptor {
    pub plugin_id: String,
    pub command_name: String,
    pub command_description: String,
    pub callback_symbol: String,
    pub library_path: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub required_role: String,
    pub scope: String,
}

#[derive(Debug, Clone)]
pub struct DynamicNoticeDescriptor {
    pub plugin_id: String,
    pub notice_route: String,
    pub callback_symbol: String,
    pub library_path: String,
}

#[derive(Debug, Clone)]
pub struct DynamicRequestDescriptor {
    pub plugin_id: String,
    pub request_route: String,
    pub callback_symbol: String,
    pub library_path: String,
}

#[derive(Debug, Clone)]
pub struct DynamicMetaDescriptor {
    pub plugin_id: String,
    pub meta_route: String,
    pub callback_symbol: String,
    pub library_path: String,
}

#[derive(Debug, Clone)]
pub struct DynamicRuntimeHealthEntry {
    pub path: String,
    pub failures: u32,
    pub isolated_until_epoch_ms: Option<u128>,
    pub last_error: Option<String>,
    pub recent_errors: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct PluginState {
    modules: BTreeMap<String, bool>,
    priorities: BTreeMap<String, u32>,
}

impl PluginState {
    pub fn is_enabled(&self, module: &str) -> bool {
        self.modules.get(module).copied().unwrap_or(true)
    }

    pub fn set_enabled(&mut self, module: impl Into<String>, enabled: bool) {
        self.modules.insert(module.into(), enabled);
    }

    pub fn priority(&self, module: &str) -> Option<u32> {
        self.priorities.get(module).copied()
    }

    pub fn set_priority(&mut self, module: impl Into<String>, priority: u32) -> Result<()> {
        if priority > MAX_PLUGIN_PRIORITY {
            return Err(QimenError::Config(format!(
                "plugin priority must be between 0 and {MAX_PLUGIN_PRIORITY}"
            )));
        }
        self.priorities.insert(module.into(), priority);
        Ok(())
    }

    pub fn priorities(&self) -> &BTreeMap<String, u32> {
        &self.priorities
    }

    pub fn save_to_path(&self, path: &str) -> Result<()> {
        let target = Path::new(path);
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        if target.exists() {
            let backup = format!("{}.bak", path);
            fs::copy(target, &backup)?;
        }

        let mut table = toml::map::Map::new();
        for (module, enabled) in &self.modules {
            table.insert(module.clone(), toml::Value::Boolean(*enabled));
        }
        let mut root = toml::map::Map::new();
        root.insert("modules".to_string(), toml::Value::Table(table));
        if !self.priorities.is_empty() {
            let mut priorities = toml::map::Map::new();
            for (module, priority) in &self.priorities {
                priorities.insert(module.clone(), toml::Value::Integer(i64::from(*priority)));
            }
            root.insert("priorities".to_string(), toml::Value::Table(priorities));
        }
        let tmp_path = format!("{}.{}.tmp", path, std::process::id());
        fs::write(
            &tmp_path,
            toml::to_string(&toml::Value::Table(root))
                .map_err(|err| QimenError::Config(err.to_string()))?,
        )?;
        if let Err(first_error) = fs::rename(&tmp_path, path) {
            if target.exists() {
                fs::remove_file(target)?;
                fs::rename(&tmp_path, path)?;
            } else {
                return Err(first_error.into());
            }
        }
        Ok(())
    }

    pub fn modules(&self) -> &BTreeMap<String, bool> {
        &self.modules
    }
}

pub fn load_plugin_state(path: &str) -> Result<PluginState> {
    if !Path::new(path).exists() {
        return Ok(PluginState::default());
    }

    let raw = fs::read_to_string(path)?;
    let value: toml::Value = toml::from_str(&raw)?;
    let mut state = PluginState::default();

    if let Some(table) = value.get("modules").and_then(toml::Value::as_table) {
        for (key, value) in table {
            state.set_enabled(key.clone(), value.as_bool().unwrap_or(true));
        }
    }

    if let Some(table) = value.get("priorities").and_then(toml::Value::as_table) {
        for (key, value) in table {
            let priority = value.as_integer().ok_or_else(|| {
                QimenError::Config(format!("plugin priority for '{key}' must be an integer"))
            })?;
            let priority = u32::try_from(priority).map_err(|_| {
                QimenError::Config(format!("plugin priority for '{key}' is out of range"))
            })?;
            state.set_priority(key.clone(), priority)?;
        }
    }

    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::{PluginState, load_plugin_state};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("qimen-host-types-{label}-{nonce}.toml"))
    }

    #[test]
    fn plugin_state_round_trips_priorities_and_legacy_modules() {
        let path = temp_path("priority-roundtrip");
        let mut state = PluginState::default();
        state.set_enabled("example-plugin", false);
        state
            .set_priority("example-plugin", 420)
            .expect("priority should be valid");
        state.save_to_path(path.to_str().unwrap()).unwrap();

        let loaded = load_plugin_state(path.to_str().unwrap()).unwrap();
        assert!(!loaded.is_enabled("example-plugin"));
        assert_eq!(loaded.priority("example-plugin"), Some(420));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn plugin_state_rejects_out_of_range_priority() {
        let mut state = PluginState::default();
        assert!(state.set_priority("example-plugin", 1_001).is_err());
    }
}
