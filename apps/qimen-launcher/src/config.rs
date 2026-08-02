use crate::DynError;
use serde::Deserialize;
use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    pub process: ProcessConfig,
    pub update: UpdateConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ProcessConfig {
    pub binary: Option<PathBuf>,
    pub working_dir: PathBuf,
    pub args: Vec<String>,
    pub restart_policy: RestartPolicy,
    pub restart_delay_secs: u64,
    pub max_crash_restarts: u32,
    pub graceful_shutdown_secs: u64,
    pub startup_grace_secs: u64,
    pub health_url: Option<String>,
    pub health_timeout_secs: u64,
}

impl Default for ProcessConfig {
    fn default() -> Self {
        Self {
            binary: None,
            working_dir: PathBuf::from("."),
            args: Vec::new(),
            restart_policy: RestartPolicy::OnFailure,
            restart_delay_secs: 3,
            max_crash_restarts: 5,
            graceful_shutdown_secs: 30,
            startup_grace_secs: 3,
            health_url: Some("http://127.0.0.1:3210/healthz".to_string()),
            health_timeout_secs: 45,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Never,
    OnFailure,
    Always,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub repository: String,
    pub channel: UpdateChannel,
    pub auto_install: bool,
    pub check_interval_secs: u64,
    pub request_timeout_secs: u64,
    pub update_dir: PathBuf,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            repository: "lvyunqi/QimenBot".to_string(),
            channel: UpdateChannel::Stable,
            auto_install: false,
            check_interval_secs: 21_600,
            request_timeout_secs: 30,
            update_dir: PathBuf::from(".qimen-update"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Beta,
}

impl UpdateChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
        }
    }

    pub fn allows_prerelease(self) -> bool {
        matches!(self, Self::Beta)
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub raw: LauncherConfig,
    pub working_dir: PathBuf,
    pub binary_path: PathBuf,
    pub update_dir: PathBuf,
}

impl ResolvedConfig {
    /// 加载启动配置，并把所有部署路径固定到绝对路径。
    pub fn load(requested_path: Option<PathBuf>) -> Result<Self, DynError> {
        let executable_path = std::env::current_exe()?.canonicalize()?;
        let install_dir = executable_path
            .parent()
            .ok_or("qimenbot executable has no parent directory")?
            .to_path_buf();
        let config_path = requested_path
            .or_else(|| std::env::var_os("QIMENBOT_LAUNCH_CONFIG").map(PathBuf::from))
            .or_else(|| std::env::var_os("QIMEN_LAUNCHER_CONFIG").map(PathBuf::from))
            .unwrap_or_else(default_config_path);
        let config_path = absolute_from(&std::env::current_dir()?, &config_path);

        if !config_path.exists() {
            let example_path = PathBuf::from(format!("{}.example", config_path.display()));
            if example_path.exists() {
                if let Some(parent) = config_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&example_path, &config_path)?;
                tracing::info!(
                    source = %example_path.display(),
                    destination = %config_path.display(),
                    "已从示例创建 qimenbot 配置"
                );
            }
        }

        let raw = if config_path.exists() {
            toml::from_str::<LauncherConfig>(&fs::read_to_string(&config_path)?)?
        } else {
            LauncherConfig::default()
        };
        validate(&raw)?;

        let working_dir = absolute_from(&install_dir, &raw.process.working_dir);
        let binary_path = raw
            .process
            .binary
            .as_ref()
            .map(|path| configured_binary_path(&install_dir, path))
            .unwrap_or_else(|| install_dir.join(daemon_file_name()));
        let update_dir = absolute_from(&install_dir, &raw.update.update_dir);

        Ok(Self {
            raw,
            working_dir,
            binary_path,
            update_dir,
        })
    }

    pub fn shutdown_timeout(&self) -> Duration {
        Duration::from_secs(self.raw.process.graceful_shutdown_secs)
    }
}

fn default_config_path() -> PathBuf {
    let preferred = PathBuf::from("config/qimenbot.toml");
    let preferred_example = PathBuf::from("config/qimenbot.toml.example");
    if preferred.exists() || preferred_example.exists() {
        preferred
    } else {
        PathBuf::from("config/launcher.toml")
    }
}

fn validate(config: &LauncherConfig) -> Result<(), DynError> {
    if config.process.restart_delay_secs == 0 {
        return Err("process.restart_delay_secs must be greater than zero".into());
    }
    if config.process.graceful_shutdown_secs == 0 || config.process.health_timeout_secs == 0 {
        return Err("process shutdown and health timeouts must be greater than zero".into());
    }
    if config.update.check_interval_secs < 60 {
        return Err("update.check_interval_secs must be at least 60".into());
    }
    if config.update.request_timeout_secs == 0 {
        return Err("update.request_timeout_secs must be greater than zero".into());
    }
    validate_repository(&config.update.repository)?;
    if let Some(url) = &config.process.health_url
        && !url.trim().is_empty()
    {
        validate_health_url(url)?;
    }
    Ok(())
}

fn validate_health_url(value: &str) -> Result<(), DynError> {
    let url = reqwest::Url::parse(value.trim())?;
    let host = url
        .host_str()
        .ok_or("process.health_url must include a host")?
        .trim_matches(['[', ']']);
    let is_loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .map(|address| address.is_loopback())
            .unwrap_or(false);
    if url.scheme() != "http"
        || !is_loopback
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("process.health_url must use a loopback HTTP address".into());
    }
    Ok(())
}

fn validate_repository(repository: &str) -> Result<(), DynError> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err("update.repository must use the owner/name format".into());
    };
    if owner.is_empty()
        || name.is_empty()
        || name.contains('/')
        || !owner.chars().all(valid_repository_character)
        || !name.chars().all(valid_repository_character)
    {
        return Err("update.repository contains unsupported characters".into());
    }
    Ok(())
}

fn valid_repository_character(value: char) -> bool {
    value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.')
}

fn absolute_from(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn configured_binary_path(base: &Path, path: &Path) -> PathBuf {
    let mut resolved = absolute_from(base, path);
    if cfg!(windows) && resolved.extension().is_none() {
        resolved.set_extension("exe");
    }
    resolved
}

pub fn daemon_file_name() -> &'static str {
    if cfg!(windows) {
        "qimenbotd.exe"
    } else {
        "qimenbotd"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_requires_one_safe_owner_and_name() {
        assert!(validate_repository("lvyunqi/QimenBot").is_ok());
        assert!(validate_repository("missing-name").is_err());
        assert!(validate_repository("owner/name/extra").is_err());
        assert!(validate_repository("owner/../name").is_err());
    }

    #[test]
    fn health_url_rejects_non_loopback_and_userinfo_bypasses() {
        assert!(validate_health_url("http://127.0.0.1:3210/healthz").is_ok());
        assert!(validate_health_url("http://[::1]:3210/healthz").is_ok());
        assert!(validate_health_url("http://localhost:3210/healthz").is_ok());
        assert!(validate_health_url("https://localhost:3210/healthz").is_err());
        assert!(validate_health_url("http://127.0.0.1:80@invalid.example").is_err());
    }

    #[test]
    fn configured_binary_uses_the_platform_executable_suffix() {
        let resolved =
            configured_binary_path(Path::new("install-root"), Path::new("runtime/qimenbotd"));
        if cfg!(windows) {
            assert_eq!(
                resolved.extension().and_then(|value| value.to_str()),
                Some("exe")
            );
        } else {
            assert!(resolved.extension().is_none());
        }
    }
}
