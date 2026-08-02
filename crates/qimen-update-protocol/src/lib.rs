use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const UPDATE_DIR_ENV: &str = "QIMEN_UPDATE_DIR";
pub const DEPLOYMENT_ENV: &str = "QIMEN_DEPLOYMENT";

const STATUS_FILE: &str = "status.json";
const LAUNCHER_COMMAND_DIR: &str = "launcher-commands";
const RUNTIME_COMMAND_DIR: &str = "runtime-commands";
const SCHEMA_VERSION: u32 = 1;
static COMMAND_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentKind {
    BinaryManaged,
    Docker,
    DirectBinary,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePhase {
    Idle,
    Checking,
    UpToDate,
    Available,
    Downloading,
    Ready,
    Applying,
    Restarting,
    RolledBack,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LauncherCommandAction {
    Check,
    Install,
    Restart,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeCommandAction {
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LauncherCommand {
    pub schema_version: u32,
    pub id: String,
    pub action: LauncherCommandAction,
    pub created_at_epoch_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeCommand {
    pub schema_version: u32,
    pub id: String,
    pub action: RuntimeCommandAction,
    pub created_at_epoch_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateStatus {
    pub schema_version: u32,
    pub deployment: DeploymentKind,
    pub phase: UpdatePhase,
    pub current_version: String,
    pub launcher_version: String,
    pub target: String,
    pub channel: String,
    pub auto_install: bool,
    pub available_version: Option<String>,
    pub release_url: Option<String>,
    pub progress_percent: Option<u8>,
    pub message: String,
    pub checked_at_epoch_ms: Option<u64>,
    pub updated_at_epoch_ms: u64,
}

impl UpdateStatus {
    /// 创建由 launcher 管理的初始状态。
    pub fn managed(
        current_version: impl Into<String>,
        launcher_version: impl Into<String>,
        target: impl Into<String>,
        channel: impl Into<String>,
        auto_install: bool,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            deployment: DeploymentKind::BinaryManaged,
            phase: UpdatePhase::Idle,
            current_version: current_version.into(),
            launcher_version: launcher_version.into(),
            target: target.into(),
            channel: channel.into(),
            auto_install,
            available_version: None,
            release_url: None,
            progress_percent: None,
            message: "等待检查更新".to_string(),
            checked_at_epoch_ms: None,
            updated_at_epoch_ms: epoch_millis(),
        }
    }

    /// 更新时间和操作阶段，避免各调用方遗漏状态时间戳。
    pub fn set_phase(&mut self, phase: UpdatePhase, message: impl Into<String>) {
        self.phase = phase;
        self.message = message.into();
        self.updated_at_epoch_ms = epoch_millis();
    }
}

/// 返回 launcher 注入的受控更新目录。
pub fn managed_update_dir() -> Option<PathBuf> {
    std::env::var_os(UPDATE_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 根据显式环境标记判断部署类型，不尝试猜测任意服务管理器。
pub fn deployment_kind() -> DeploymentKind {
    if managed_update_dir().is_some() {
        DeploymentKind::BinaryManaged
    } else if std::env::var(DEPLOYMENT_ENV)
        .map(|value| value.eq_ignore_ascii_case("docker"))
        .unwrap_or(false)
        || cfg!(unix) && Path::new("/.dockerenv").exists()
    {
        DeploymentKind::Docker
    } else {
        DeploymentKind::DirectBinary
    }
}

/// 原子写入 launcher 状态文件，供管理面板轮询读取。
pub fn write_status(update_dir: &Path, status: &UpdateStatus) -> io::Result<()> {
    write_json_atomic(&update_dir.join(STATUS_FILE), status)
}

pub fn read_status(update_dir: &Path) -> io::Result<Option<UpdateStatus>> {
    let path = update_dir.join(STATUS_FILE);
    match fs::read(path) {
        Ok(raw) => serde_json::from_slice(&raw)
            .map(Some)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// 向常驻 launcher 投递受限控制命令。
pub fn enqueue_launcher_command(
    update_dir: &Path,
    action: LauncherCommandAction,
) -> io::Result<String> {
    let id = next_command_id();
    let command = LauncherCommand {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        action,
        created_at_epoch_ms: epoch_millis(),
    };
    enqueue_command(update_dir, LAUNCHER_COMMAND_DIR, &id, &command)?;
    Ok(id)
}

/// launcher 通过该命令要求子进程进入统一优雅关闭流程。
pub fn enqueue_runtime_command(
    update_dir: &Path,
    action: RuntimeCommandAction,
) -> io::Result<String> {
    let id = next_command_id();
    let command = RuntimeCommand {
        schema_version: SCHEMA_VERSION,
        id: id.clone(),
        action,
        created_at_epoch_ms: epoch_millis(),
    };
    enqueue_command(update_dir, RUNTIME_COMMAND_DIR, &id, &command)?;
    Ok(id)
}

pub fn take_launcher_commands(update_dir: &Path) -> io::Result<Vec<LauncherCommand>> {
    take_commands(update_dir, LAUNCHER_COMMAND_DIR)
}

pub fn take_runtime_commands(update_dir: &Path) -> io::Result<Vec<RuntimeCommand>> {
    take_commands(update_dir, RUNTIME_COMMAND_DIR)
}

pub fn epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn next_command_id() -> String {
    let sequence = COMMAND_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("{}-{}-{sequence}", epoch_millis(), std::process::id())
}

fn enqueue_command<T: Serialize>(
    update_dir: &Path,
    directory: &str,
    id: &str,
    command: &T,
) -> io::Result<()> {
    let command_dir = update_dir.join(directory);
    fs::create_dir_all(&command_dir)?;
    write_json_atomic(&command_dir.join(format!("{id}.json")), command)
}

fn take_commands<T: for<'de> Deserialize<'de>>(
    update_dir: &Path,
    directory: &str,
) -> io::Result<Vec<T>> {
    let command_dir = update_dir.join(directory);
    let mut paths = match fs::read_dir(&command_dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
            .collect::<Vec<_>>(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    paths.sort();

    let mut commands = Vec::with_capacity(paths.len());
    for path in paths {
        let raw = fs::read(&path)?;
        match serde_json::from_slice(&raw) {
            Ok(command) => {
                commands.push(command);
                fs::remove_file(path)?;
            }
            Err(_) => {
                let invalid = path.with_extension("invalid");
                let _ = fs::rename(path, invalid);
            }
        }
    }
    Ok(commands)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", next_command_id()));
    let raw = serde_json::to_vec_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(&temporary, raw)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "qimen-update-protocol-{name}-{}-{}",
            std::process::id(),
            next_command_id()
        ))
    }

    #[test]
    fn status_round_trip() {
        let directory = test_dir("status");
        let status = UpdateStatus::managed("0.1.0", "0.1.0", "test-target", "stable", false);
        write_status(&directory, &status).unwrap();
        assert_eq!(read_status(&directory).unwrap(), Some(status));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn launcher_commands_are_consumed_once() {
        let directory = test_dir("commands");
        enqueue_launcher_command(&directory, LauncherCommandAction::Check).unwrap();
        let commands = take_launcher_commands(&directory).unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].action, LauncherCommandAction::Check);
        assert!(take_launcher_commands(&directory).unwrap().is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
