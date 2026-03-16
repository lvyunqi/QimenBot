use qimen_error::{QimenError, Result};
use qimen_plugin_api::RateLimiterConfig;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub runtime: RuntimeConfig,
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub official_host: OfficialHostConfig,
    #[serde(default)]
    pub bots: Vec<BotConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub env: String,
    pub shutdown_timeout_secs: u64,
    pub task_grace_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub level: String,
    pub json_logs: bool,
    pub metrics_bind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfficialHostConfig {
    #[serde(default = "default_builtin_modules")]
    pub builtin_modules: Vec<String>,
    #[serde(default)]
    pub plugin_modules: Vec<String>,
    #[serde(default = "default_plugin_state_path")]
    pub plugin_state_path: String,
    #[serde(default = "default_plugin_bin_dir")]
    pub plugin_bin_dir: String,
    /// Timeout in seconds for dynamic plugin FFI calls (default: 30).
    #[serde(default = "default_dynamic_plugin_timeout_secs")]
    pub dynamic_plugin_timeout_secs: u64,
}

impl Default for OfficialHostConfig {
    fn default() -> Self {
        Self {
            builtin_modules: default_builtin_modules(),
            plugin_modules: Vec::new(),
            plugin_state_path: default_plugin_state_path(),
            plugin_bin_dir: default_plugin_bin_dir(),
            dynamic_plugin_timeout_secs: default_dynamic_plugin_timeout_secs(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub id: String,
    pub protocol: String,
    pub transport: String,
    pub endpoint: Option<String>,
    pub bind: Option<String>,
    pub path: Option<String>,
    pub access_token: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub enabled_modules: Vec<String>,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub admins: Vec<String>,
    #[serde(default)]
    pub auto_approve_friend_requests: bool,
    #[serde(default)]
    pub auto_approve_group_invites: bool,
    #[serde(default)]
    pub auto_approve_friend_request_user_whitelist: Vec<String>,
    #[serde(default)]
    pub auto_approve_friend_request_user_blacklist: Vec<String>,
    #[serde(default)]
    pub auto_approve_friend_request_comment_keywords: Vec<String>,
    #[serde(default)]
    pub auto_reject_friend_request_comment_keywords: Vec<String>,
    #[serde(default)]
    pub auto_approve_friend_request_remark: Option<String>,
    #[serde(default)]
    pub auto_approve_group_invite_user_whitelist: Vec<String>,
    #[serde(default)]
    pub auto_approve_group_invite_user_blacklist: Vec<String>,
    #[serde(default)]
    pub auto_approve_group_invite_group_whitelist: Vec<String>,
    #[serde(default)]
    pub auto_approve_group_invite_group_blacklist: Vec<String>,
    #[serde(default)]
    pub auto_approve_group_invite_comment_keywords: Vec<String>,
    #[serde(default)]
    pub auto_reject_group_invite_comment_keywords: Vec<String>,
    #[serde(default)]
    pub auto_reject_group_invite_reason: Option<String>,
    #[serde(default)]
    pub auto_reply_poke_enabled: bool,
    #[serde(default)]
    pub auto_reply_poke_message: Option<String>,
    #[serde(default)]
    pub limiter: RateLimiterConfig,
}

impl AppConfig {
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self> {
        let raw = fs::read_to_string(path)?;
        let expanded = expand_env_placeholders(&raw);
        let config = toml::from_str::<Self>(&expanded)?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<()> {
        if self.bots.is_empty() {
            return Err(QimenError::Config(
                "at least one [[bots]] entry is required".to_string(),
            ));
        }

        for bot in &self.bots {
            if bot.id.trim().is_empty() {
                return Err(QimenError::Config("bot id cannot be empty".to_string()));
            }
            if bot.protocol.trim().is_empty() {
                return Err(QimenError::Config(format!(
                    "bot '{}' must declare protocol",
                    bot.id
                )));
            }
            if bot.transport.trim().is_empty() {
                return Err(QimenError::Config(format!(
                    "bot '{}' must declare transport",
                    bot.id
                )));
            }
        }

        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn default_builtin_modules() -> Vec<String> {
    vec![
        "command".to_string(),
        "admin".to_string(),
        "scheduler".to_string(),
        "bridge".to_string(),
    ]
}

fn default_plugin_state_path() -> String {
    "config/plugin-state.toml".to_string()
}

fn default_plugin_bin_dir() -> String {
    "plugins/bin".to_string()
}

fn default_dynamic_plugin_timeout_secs() -> u64 {
    30
}

fn expand_env_placeholders(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut index = 0;

    while index < chars.len() {
        if chars[index] == '$' && index + 1 < chars.len() && chars[index + 1] == '{' {
            index += 2;
            let start = index;
            while index < chars.len() && chars[index] != '}' {
                index += 1;
            }
            if index < chars.len() {
                let key: String = chars[start..index].iter().collect();
                let value = std::env::var(&key).unwrap_or_default();
                output.push_str(&value);
                index += 1;
                continue;
            }
            output.push_str("${");
            output.extend(chars[start..].iter());
            break;
        }
        output.push(chars[index]);
        index += 1;
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[official_host]
builtin_modules = ["command"]
plugin_modules = []

[[bots]]
id = "test-bot"
protocol = "onebot11"
transport = "ws-forward"
endpoint = "ws://127.0.0.1:3001"
"#;
        let config: std::result::Result<AppConfig, _> = toml::from_str(toml_str);
        assert!(config.is_ok(), "failed to parse: {:?}", config.err());
        let config = config.unwrap();
        assert_eq!(config.bots.len(), 1);
        assert_eq!(config.bots[0].id, "test-bot");
        assert_eq!(config.bots[0].protocol, "onebot11");
        assert_eq!(config.bots[0].transport, "ws-forward");
        assert!(config.bots[0].enabled);
    }

    #[test]
    fn parse_config_with_rate_limiter() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[official_host]
builtin_modules = []
plugin_modules = []

[[bots]]
id = "bot1"
protocol = "onebot11"
transport = "ws-forward"
endpoint = "ws://127.0.0.1:3001"

[bots.limiter]
enable = true
rate = 2.0
capacity = 10
"#;
        let config: std::result::Result<AppConfig, _> = toml::from_str(toml_str);
        assert!(config.is_ok(), "failed to parse: {:?}", config.err());
        let config = config.unwrap();
        assert!(config.bots[0].limiter.enable);
        assert_eq!(config.bots[0].limiter.rate, 2.0);
        assert_eq!(config.bots[0].limiter.capacity, 10);
    }

    #[test]
    fn parse_multiple_bots() {
        let toml_str = r#"
[runtime]
env = "production"
shutdown_timeout_secs = 30
task_grace_secs = 10

[observability]
level = "warn"
json_logs = true
metrics_bind = "0.0.0.0:9090"

[[bots]]
id = "bot-alpha"
protocol = "onebot11"
transport = "ws-forward"
endpoint = "ws://127.0.0.1:3001"

[[bots]]
id = "bot-beta"
protocol = "onebot11"
transport = "ws-reverse"
bind = "0.0.0.0:8080"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.bots.len(), 2);
        assert_eq!(config.bots[0].id, "bot-alpha");
        assert_eq!(config.bots[1].id, "bot-beta");
    }

    #[test]
    fn official_host_defaults_applied() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[[bots]]
id = "test"
protocol = "onebot11"
transport = "ws-forward"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.official_host.builtin_modules,
            vec!["command", "admin", "scheduler", "bridge"]
        );
        assert_eq!(config.official_host.plugin_state_path, "config/plugin-state.toml");
        assert_eq!(config.official_host.plugin_bin_dir, "plugins/bin");
    }

    #[test]
    fn validate_rejects_empty_bots() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_empty_bot_id() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[[bots]]
id = ""
protocol = "onebot11"
transport = "ws-forward"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_protocol() {
        let toml_str = r#"
[runtime]
env = "development"
shutdown_timeout_secs = 10
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[[bots]]
id = "bot1"
protocol = ""
transport = "ws-forward"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn expand_env_replaces_placeholder() {
        unsafe { std::env::set_var("QIMEN_TEST_VAR", "replaced_value"); }
        let input = "key = \"${QIMEN_TEST_VAR}\"";
        let output = expand_env_placeholders(input);
        assert_eq!(output, "key = \"replaced_value\"");
        unsafe { std::env::remove_var("QIMEN_TEST_VAR"); }
    }

    #[test]
    fn expand_env_missing_var_becomes_empty() {
        unsafe { std::env::remove_var("QIMEN_NONEXISTENT_VAR_XYZ"); }
        let input = "value = \"${QIMEN_NONEXISTENT_VAR_XYZ}\"";
        let output = expand_env_placeholders(input);
        assert_eq!(output, "value = \"\"");
    }

    #[test]
    fn expand_env_no_placeholders_unchanged() {
        let input = "plain text without placeholders";
        let output = expand_env_placeholders(input);
        assert_eq!(output, input);
    }

    #[test]
    fn bot_enabled_defaults_to_true() {
        let toml_str = r#"
[runtime]
env = "dev"
shutdown_timeout_secs = 5
task_grace_secs = 2

[observability]
level = "debug"
json_logs = false
metrics_bind = "0.0.0.0:9090"

[[bots]]
id = "default-bot"
protocol = "onebot11"
transport = "ws-forward"
"#;
        let config: AppConfig = toml::from_str(toml_str).unwrap();
        assert!(config.bots[0].enabled);
    }
}
