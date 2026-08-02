use crate::DynError;
use crate::config::{ResolvedConfig, RestartPolicy};
use crate::github::{GithubClient, ReleaseInfo};
use crate::installer::{
    apply_plan, finalize_plan, pending_plan, read_plan, rollback_plan, stage_update,
};
use qimen_update_protocol::{
    LauncherCommandAction, RuntimeCommandAction, UpdatePhase, UpdateStatus,
    enqueue_runtime_command, take_launcher_commands, take_runtime_commands, write_status,
};
use semver::Version;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc};

enum SupervisorAction {
    Apply(std::path::PathBuf),
    Restart,
}

/// 常驻监督 daemon，并协调检查、安装、重启和回滚。
pub async fn run(config: ResolvedConfig) -> Result<(), DynError> {
    tokio::fs::create_dir_all(&config.update_dir).await?;
    let mut post_update_plan = pending_plan(&config.update_dir)?;
    if !config.binary_path.is_file()
        && let Some(plan_path) = post_update_plan.take()
    {
        tracing::warn!(
            plan = %plan_path.display(),
            "检测到更新中断且 daemon 文件缺失，正在恢复旧版本"
        );
        rollback_plan(&plan_path, "launcher 在文件替换期间意外中断").await?;
    }
    if !config.binary_path.is_file() {
        return Err(format!(
            "daemon executable '{}' was not found",
            config.binary_path.display()
        )
        .into());
    }
    let installed_version = installed_version(&config);
    let target = current_target().ok_or("current platform is not supported by the updater")?;
    let status = Arc::new(Mutex::new(UpdateStatus::managed(
        installed_version.to_string(),
        env!("CARGO_PKG_VERSION"),
        target,
        config.raw.update.channel.as_str(),
        config.raw.update.auto_install,
    )));
    persist_status(&config, &status).await?;

    let mut crash_restarts = 0_u32;
    loop {
        let stale_commands = take_runtime_commands(&config.update_dir)?;
        if !stale_commands.is_empty() {
            tracing::warn!(
                count = stale_commands.len(),
                "已清理上次进程退出后遗留的关闭命令"
            );
        }
        let mut child = spawn_daemon(&config).await?;
        tracing::info!(pid = child.id(), binary = %config.binary_path.display(), "daemon 已启动");

        if let Some(plan_path) = post_update_plan.take() {
            let plan = read_plan(&plan_path)?;
            match wait_for_health(&mut child, &config, Some(&plan.version)).await {
                Ok(()) => {
                    {
                        let mut current = status.lock().await;
                        current.current_version.clone_from(&plan.version);
                        current.launcher_version = env!("CARGO_PKG_VERSION").to_string();
                        current.available_version = None;
                        current.release_url = None;
                        current.progress_percent = None;
                        current.checked_at_epoch_ms = Some(qimen_update_protocol::epoch_millis());
                        current.set_phase(
                            UpdatePhase::UpToDate,
                            format!("版本 {} 已安装并通过健康检查", plan.version),
                        );
                        write_status(&config.update_dir, &current)?;
                    }
                    finalize_plan(&plan_path)?;
                }
                Err(error) => {
                    let reason = error.to_string();
                    tracing::error!(error = %reason, "新版本健康检查失败，准备回滚");
                    graceful_stop(&mut child, &config).await?;
                    rollback_plan(&plan_path, &reason).await?;
                    if let Some(rolled_back) =
                        qimen_update_protocol::read_status(&config.update_dir)?
                    {
                        *status.lock().await = rolled_back;
                    }
                    crash_restarts = 0;
                    continue;
                }
            }
        }

        let (action_sender, mut action_receiver) = mpsc::channel(2);
        let worker = tokio::spawn(update_loop(
            config.clone(),
            Arc::clone(&status),
            action_sender,
        ));

        enum ExitReason {
            Child(std::process::ExitStatus),
            Termination,
            Action(SupervisorAction),
        }
        let reason = tokio::select! {
            result = child.wait() => ExitReason::Child(result?),
            _ = termination_signal() => ExitReason::Termination,
            action = action_receiver.recv() => match action {
                Some(action) => ExitReason::Action(action),
                None => return Err("update worker stopped unexpectedly".into()),
            },
        };
        worker.abort();

        match reason {
            ExitReason::Termination => {
                tracing::info!("launcher 收到关闭信号");
                graceful_stop(&mut child, &config).await?;
                return Ok(());
            }
            ExitReason::Action(SupervisorAction::Restart) => {
                set_phase(
                    &config,
                    &status,
                    UpdatePhase::Restarting,
                    "正在优雅重启 daemon",
                )
                .await?;
                graceful_stop(&mut child, &config).await?;
                crash_restarts = 0;
                tokio::time::sleep(Duration::from_secs(config.raw.process.restart_delay_secs))
                    .await;
            }
            ExitReason::Action(SupervisorAction::Apply(plan_path)) => {
                graceful_stop(&mut child, &config).await?;
                apply_plan(&plan_path).await?;
                post_update_plan = Some(plan_path);
                crash_restarts = 0;
            }
            ExitReason::Child(exit_status) => {
                let should_restart = match config.raw.process.restart_policy {
                    RestartPolicy::Never => false,
                    RestartPolicy::OnFailure => !exit_status.success(),
                    RestartPolicy::Always => true,
                };
                if !should_restart {
                    tracing::info!(status = %exit_status, "daemon 已退出，当前策略不再重启");
                    return Ok(());
                }
                crash_restarts = crash_restarts.saturating_add(1);
                if config.raw.process.max_crash_restarts > 0
                    && crash_restarts > config.raw.process.max_crash_restarts
                {
                    set_phase(
                        &config,
                        &status,
                        UpdatePhase::Error,
                        format!("daemon 连续退出 {} 次，已停止自动拉起", crash_restarts),
                    )
                    .await?;
                    return Err(format!("daemon restart limit exceeded: {exit_status}").into());
                }
                tracing::warn!(
                    status = %exit_status,
                    attempt = crash_restarts,
                    "daemon 异常退出，等待重新启动"
                );
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(config.raw.process.restart_delay_secs)) => {}
                    _ = termination_signal() => return Ok(()),
                }
            }
        }
    }
}

async fn update_loop(
    config: ResolvedConfig,
    status: Arc<Mutex<UpdateStatus>>,
    action_sender: mpsc::Sender<SupervisorAction>,
) {
    let mut command_poll = tokio::time::interval(Duration::from_millis(500));
    command_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut scheduled_check =
        tokio::time::interval(Duration::from_secs(config.raw.update.check_interval_secs));
    scheduled_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            _ = command_poll.tick() => {
                match take_launcher_commands(&config.update_dir) {
                    Ok(commands) => {
                        for command in commands {
                            let result = match command.action {
                                LauncherCommandAction::Check => check_only(&config, &status).await.map(|_| false),
                                LauncherCommandAction::Install => prepare_install(&config, &status, &action_sender).await,
                                LauncherCommandAction::Restart => action_sender.send(SupervisorAction::Restart).await.map(|_| true).map_err(|error| error.into()),
                            };
                            match result {
                                Ok(true) => return,
                                Ok(false) => {}
                                Err(error) => record_update_error(&config, &status, &error).await,
                            }
                        }
                    }
                    Err(error) => tracing::warn!(error = %error, "读取 launcher 控制命令失败"),
                }
            }
            _ = scheduled_check.tick(), if config.raw.update.enabled => {
                match check_only(&config, &status).await {
                    Ok(Some(release)) if config.raw.update.auto_install => {
                        if let Err(error) = install_release(&config, &status, &action_sender, &release).await {
                            record_update_error(&config, &status, &error).await;
                        } else {
                            return;
                        }
                    }
                    Ok(_) => {}
                    Err(error) => record_update_error(&config, &status, &error).await,
                }
            }
        }
    }
}

async fn check_only(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
) -> Result<Option<ReleaseInfo>, DynError> {
    if !config.raw.update.enabled {
        return Err("launcher update checks are disabled in config/launcher.toml".into());
    }
    set_phase(
        config,
        status,
        UpdatePhase::Checking,
        "正在检查 GitHub Release",
    )
    .await?;
    let current_version = {
        let current = status.lock().await;
        Version::parse(&current.current_version)?
    };
    let client = github_client(config)?;
    let release = client.find_update(&current_version).await?;
    let mut current = status.lock().await;
    current.checked_at_epoch_ms = Some(qimen_update_protocol::epoch_millis());
    match &release {
        Some(release) => {
            current.available_version = Some(release.version.to_string());
            current.release_url = Some(release.html_url.clone());
            current.set_phase(
                UpdatePhase::Available,
                format!("发现新版本 {}", release.version),
            );
        }
        None => {
            current.available_version = None;
            current.release_url = None;
            current.progress_percent = None;
            current.set_phase(UpdatePhase::UpToDate, "当前已经是最新版本");
        }
    }
    write_status(&config.update_dir, &current)?;
    Ok(release)
}

async fn prepare_install(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
    action_sender: &mpsc::Sender<SupervisorAction>,
) -> Result<bool, DynError> {
    let Some(release) = check_only(config, status).await? else {
        return Ok(false);
    };
    install_release(config, status, action_sender, &release).await?;
    Ok(true)
}

/// 下载指定 Release，并在文件准备完成后通知监督循环执行替换。
async fn install_release(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
    action_sender: &mpsc::Sender<SupervisorAction>,
    release: &ReleaseInfo,
) -> Result<(), DynError> {
    let target = current_target().ok_or("current platform is not supported by the updater")?;
    let client = github_client(config)?;
    let plan_path = {
        let mut current = status.lock().await;
        current.progress_percent = Some(0);
        current.set_phase(
            UpdatePhase::Downloading,
            format!("正在下载版本 {}", release.version),
        );
        write_status(&config.update_dir, &current)?;
        stage_update(&client, release, config, target, &mut current).await?
    };
    action_sender
        .send(SupervisorAction::Apply(plan_path))
        .await?;
    Ok(())
}

fn github_client(config: &ResolvedConfig) -> Result<GithubClient, DynError> {
    GithubClient::new(
        &config.raw.update.repository,
        config.raw.update.channel,
        Duration::from_secs(config.raw.update.request_timeout_secs),
    )
}

/// 启动 daemon，同时隔离只应由 updater 使用的 GitHub 凭据。
async fn spawn_daemon(config: &ResolvedConfig) -> Result<Child, DynError> {
    let mut command = Command::new(&config.binary_path);
    command
        .args(&config.raw.process.args)
        .current_dir(&config.working_dir)
        .env(qimen_update_protocol::UPDATE_DIR_ENV, &config.update_dir)
        .env(qimen_update_protocol::DEPLOYMENT_ENV, "binary-managed")
        .env_remove("GITHUB_TOKEN")
        .kill_on_drop(true)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    Ok(command.spawn()?)
}

fn installed_version(config: &ResolvedConfig) -> Version {
    if let Ok(Some(status)) = qimen_update_protocol::read_status(&config.update_dir)
        && let Ok(version) = Version::parse(&status.current_version)
    {
        return version;
    }
    Version::parse(env!("CARGO_PKG_VERSION")).expect("workspace version must be valid semver")
}

async fn graceful_stop(child: &mut Child, config: &ResolvedConfig) -> Result<(), DynError> {
    if child.try_wait()?.is_some() {
        return Ok(());
    }
    enqueue_runtime_command(&config.update_dir, RuntimeCommandAction::Shutdown)?;
    match tokio::time::timeout(config.shutdown_timeout(), child.wait()).await {
        Ok(result) => {
            result?;
            Ok(())
        }
        Err(_) => {
            tracing::warn!("daemon 未在优雅关闭期限内退出，执行强制终止");
            child.kill().await?;
            let _ = child.wait().await;
            Ok(())
        }
    }
}

async fn wait_for_health(
    child: &mut Child,
    config: &ResolvedConfig,
    expected_version: Option<&str>,
) -> Result<(), DynError> {
    let grace = Duration::from_secs(config.raw.process.startup_grace_secs);
    let health_url = config
        .raw
        .process
        .health_url
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(health_url) = health_url else {
        tokio::time::sleep(grace).await;
        return match child.try_wait()? {
            Some(status) => Err(format!("daemon exited during startup: {status}").into()),
            None => Ok(()),
        };
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()?;
    let deadline = Instant::now() + Duration::from_secs(config.raw.process.health_timeout_secs);
    loop {
        if let Some(status) = child.try_wait()? {
            return Err(format!("daemon exited before becoming healthy: {status}").into());
        }
        let attempt_error = match client.get(health_url).send().await {
            Ok(response) if response.status().is_success() => {
                if let Some(expected) = expected_version {
                    match response.json::<serde_json::Value>().await {
                        Ok(body)
                            if body
                                .pointer("/data/version")
                                .and_then(serde_json::Value::as_str)
                                == Some(expected) =>
                        {
                            return Ok(());
                        }
                        Ok(body) => format!(
                            "health endpoint reported version {:?}, expected {expected}",
                            body.pointer("/data/version")
                        ),
                        Err(error) => format!("invalid health response: {error}"),
                    }
                } else {
                    return Ok(());
                }
            }
            Ok(response) => format!("health endpoint returned {}", response.status()),
            Err(error) => error.to_string(),
        };
        if Instant::now() >= deadline {
            return Err(attempt_error.into());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

async fn set_phase(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
    phase: UpdatePhase,
    message: impl Into<String>,
) -> Result<(), DynError> {
    let mut current = status.lock().await;
    current.set_phase(phase, message);
    write_status(&config.update_dir, &current)?;
    Ok(())
}

async fn persist_status(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
) -> Result<(), DynError> {
    let current = status.lock().await;
    write_status(&config.update_dir, &current)?;
    Ok(())
}

async fn record_update_error(
    config: &ResolvedConfig,
    status: &Arc<Mutex<UpdateStatus>>,
    error: &DynError,
) {
    tracing::warn!(error = %error, "更新操作失败");
    let _ = set_phase(
        config,
        status,
        UpdatePhase::Error,
        format!("更新操作失败：{error}"),
    )
    .await;
}

async fn termination_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

pub fn current_target() -> Option<&'static str> {
    let environment = if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        ""
    };
    release_target(std::env::consts::OS, std::env::consts::ARCH, environment)
}

/// 只返回 Release 工作流实际发布、可由 launcher 安装的目标。
fn release_target(os: &str, architecture: &str, environment: &str) -> Option<&'static str> {
    match (os, architecture, environment) {
        ("windows", "x86_64", "msvc") => Some("x86_64-pc-windows-msvc"),
        ("linux", "x86_64", "gnu") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64", "gnu") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64", "musl") => Some("x86_64-unknown-linux-musl"),
        ("macos", "x86_64", _) => Some("x86_64-apple-darwin"),
        ("macos", "aarch64", _) => Some("aarch64-apple-darwin"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_build_has_a_target_triple() {
        assert!(current_target().is_some());
    }

    #[test]
    fn release_target_matches_published_assets_only() {
        assert_eq!(
            release_target("linux", "aarch64", "gnu"),
            Some("aarch64-unknown-linux-gnu")
        );
        assert_eq!(
            release_target("macos", "aarch64", ""),
            Some("aarch64-apple-darwin")
        );
        assert_eq!(release_target("windows", "aarch64", "msvc"), None);
        assert_eq!(release_target("linux", "aarch64", "musl"), None);
    }
}
