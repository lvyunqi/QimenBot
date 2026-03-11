use qimen_error::{QimenError, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct HostPluginReport {
    pub builtin_modules: Vec<String>,
    pub configured_plugins: Vec<String>,
    pub persisted_states: BTreeMap<String, bool>,
    pub dynamic_plugins: Vec<DynamicPluginReportEntry>,
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
}

impl PluginState {
    pub fn is_enabled(&self, module: &str) -> bool {
        self.modules.get(module).copied().unwrap_or(true)
    }

    pub fn set_enabled(&mut self, module: impl Into<String>, enabled: bool) {
        self.modules.insert(module.into(), enabled);
    }

    pub fn save_to_path(&self, path: &str) -> Result<()> {
        let target = Path::new(path);
        if let Some(parent) = target.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)?;
            }
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
        let tmp_path = format!("{}.tmp", path);
        fs::write(
            &tmp_path,
            toml::to_string(&toml::Value::Table(root))
                .map_err(|err| QimenError::Config(err.to_string()))?,
        )?;
        fs::rename(&tmp_path, path)?;
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

    Ok(state)
}
