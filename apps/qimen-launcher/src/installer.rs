use crate::DynError;
use crate::config::ResolvedConfig;
use crate::github::{GithubClient, ReleaseInfo};
use qimen_update_protocol::{UpdatePhase, UpdateStatus, write_status};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

const PENDING_PLAN_FILE: &str = "pending-plan";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePlan {
    pub schema_version: u32,
    pub previous_version: String,
    pub version: String,
    pub target: String,
    pub update_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub staged_daemon: PathBuf,
    pub daemon_target: PathBuf,
}

/// 下载 daemon 并生成只能指向当前安装目录的替换计划。
pub async fn stage_update(
    client: &GithubClient,
    release: &ReleaseInfo,
    config: &ResolvedConfig,
    target: &str,
    status: &mut UpdateStatus,
) -> Result<PathBuf, DynError> {
    let version_directory = format!("v{}", release.version);
    let staging_dir = config.update_dir.join("staging").join(version_directory);
    let backup_dir = config
        .update_dir
        .join("backups")
        .join(format!("v{}", release.version));
    tokio::fs::create_dir_all(&staging_dir).await?;
    tokio::fs::create_dir_all(&backup_dir).await?;

    let staged_daemon = staging_dir.join(crate::config::daemon_file_name());
    client
        .download_daemon(release, target, &staged_daemon, &config.update_dir, status)
        .await?;

    let plan = UpdatePlan {
        schema_version: 1,
        previous_version: status.current_version.clone(),
        version: release.version.to_string(),
        target: target.to_string(),
        update_dir: config.update_dir.clone(),
        staging_dir: staging_dir.clone(),
        backup_dir,
        staged_daemon,
        daemon_target: config.binary_path.clone(),
    };
    validate_plan(&plan)?;
    let plan_path = staging_dir.join("update-plan.json");
    write_plan(&plan_path, &plan)?;
    status.progress_percent = Some(100);
    status.set_phase(
        UpdatePhase::Ready,
        format!("版本 {} 已下载，等待重启安装", release.version),
    );
    write_status(&config.update_dir, status)?;
    Ok(plan_path)
}

pub fn read_plan(path: &Path) -> Result<UpdatePlan, DynError> {
    let plan = serde_json::from_slice::<UpdatePlan>(&fs::read(path)?)?;
    validate_plan(&plan)?;
    if path != plan.staging_dir.join("update-plan.json") {
        return Err("update plan path does not match its staging directory".into());
    }
    Ok(plan)
}

/// launcher 保持常驻，只在 daemon 停止后替换受监督的可执行文件。
pub async fn apply_plan(plan_path: &Path) -> Result<(), DynError> {
    let plan = read_plan(plan_path)?;
    let mut status = read_or_default_status(&plan);
    status.set_phase(
        UpdatePhase::Applying,
        format!("正在安装版本 {}", plan.version),
    );
    status.progress_percent = None;
    write_status(&plan.update_dir, &status)?;

    fs::create_dir_all(&plan.backup_dir)?;
    let daemon_backup = plan.backup_dir.join(crate::config::daemon_file_name());
    backup_file(&plan.daemon_target, &daemon_backup)?;
    fs::write(
        plan.update_dir.join(PENDING_PLAN_FILE),
        plan_path.to_string_lossy().as_bytes(),
    )?;
    if let Err(error) = replace_file_with_retry(&plan.staged_daemon, &plan.daemon_target).await {
        let replacement_error = error.to_string();
        let rollback_error = replace_file_with_retry(&daemon_backup, &plan.daemon_target)
            .await
            .err()
            .map(|error| error.to_string());
        if rollback_error.is_none() {
            let _ = remove_pending_marker(&plan.update_dir);
        }
        let message = rollback_error.map_or_else(
            || format!("daemon 替换失败，已恢复旧版本：{replacement_error}"),
            |rollback_error| {
                format!(
                    "daemon 替换失败且自动恢复失败：{replacement_error}；恢复错误：{rollback_error}"
                )
            },
        );
        status.set_phase(UpdatePhase::Error, message.clone());
        write_status(&plan.update_dir, &status)?;
        return Err(message.into());
    }
    Ok(())
}

/// 健康检查失败时由仍在运行的 launcher 原位恢复 daemon。
pub async fn rollback_plan(plan_path: &Path, reason: &str) -> Result<(), DynError> {
    let plan = read_plan(plan_path)?;
    let daemon_backup = plan.backup_dir.join(crate::config::daemon_file_name());
    replace_file_with_retry(&daemon_backup, &plan.daemon_target).await?;

    let mut status = read_or_default_status(&plan);
    status.current_version.clone_from(&plan.previous_version);
    status.set_phase(
        UpdatePhase::RolledBack,
        format!("新版本健康检查失败，已回滚：{reason}"),
    );
    status.available_version = Some(plan.version.clone());
    status.progress_percent = None;
    write_status(&plan.update_dir, &status)?;
    remove_pending_marker(&plan.update_dir)?;
    Ok(())
}

/// 更新成功后移除备份和暂存文件，状态文件继续保留给管理面板。
pub fn finalize_plan(plan_path: &Path) -> Result<(), DynError> {
    let plan = read_plan(plan_path)?;
    remove_pending_marker(&plan.update_dir)?;
    if plan.backup_dir.exists() {
        fs::remove_dir_all(&plan.backup_dir)?;
    }
    if plan.staging_dir.exists() {
        fs::remove_dir_all(&plan.staging_dir)?;
    }
    Ok(())
}

/// launcher 异常退出后从持久化标记恢复尚未完成的健康检查。
pub fn pending_plan(update_dir: &Path) -> Result<Option<PathBuf>, DynError> {
    let marker = update_dir.join(PENDING_PLAN_FILE);
    let raw = match fs::read_to_string(marker) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let path = PathBuf::from(raw.trim());
    read_plan(&path)?;
    Ok(Some(path))
}

fn write_plan(path: &Path, plan: &UpdatePlan) -> Result<(), DynError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(plan)?)?;
    Ok(())
}

fn remove_pending_marker(update_dir: &Path) -> Result<(), DynError> {
    match fs::remove_file(update_dir.join(PENDING_PLAN_FILE)) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn validate_plan(plan: &UpdatePlan) -> Result<(), DynError> {
    if plan.schema_version != 1 {
        return Err("unsupported update plan schema".into());
    }
    let install_dir = plan
        .daemon_target
        .parent()
        .ok_or("daemon target has no parent")?;
    if plan
        .daemon_target
        .file_name()
        .and_then(|value| value.to_str())
        != Some(crate::config::daemon_file_name())
        || plan.staged_daemon.parent() != Some(plan.staging_dir.as_path())
        || !is_managed_child(&plan.staging_dir, &plan.update_dir.join("staging"))
        || !is_managed_child(&plan.backup_dir, &plan.update_dir.join("backups"))
        || install_dir == plan.update_dir
    {
        return Err("update plan contains paths outside the managed installation".into());
    }
    Ok(())
}

fn is_managed_child(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        !relative.as_os_str().is_empty()
            && relative
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
    })
}

fn backup_file(source: &Path, destination: &Path) -> Result<(), DynError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, destination)?;
    set_executable(destination)?;
    Ok(())
}

async fn replace_file_with_retry(source: &Path, target: &Path) -> Result<(), DynError> {
    if !source.is_file() {
        return Err(format!("replacement source '{}' does not exist", source.display()).into());
    }
    let temporary = target.with_extension(format!("new-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    fs::copy(source, &temporary)?;
    set_executable(&temporary)?;

    let mut last_error = None;
    for _ in 0..300 {
        let result = (|| -> std::io::Result<()> {
            if target.exists() {
                fs::remove_file(target)?;
            }
            fs::rename(&temporary, target)
        })();
        match result {
            Ok(()) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
    let _ = fs::remove_file(temporary);
    Err(last_error
        .map(|error| format!("failed to replace '{}': {error}", target.display()))
        .unwrap_or_else(|| format!("failed to replace '{}'", target.display()))
        .into())
}

fn read_or_default_status(plan: &UpdatePlan) -> UpdateStatus {
    qimen_update_protocol::read_status(&plan.update_dir)
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            UpdateStatus::managed(
                &plan.version,
                env!("CARGO_PKG_VERSION"),
                &plan.target,
                "stable",
                false,
            )
        })
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), DynError> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), DynError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "qimen-launcher-installer-{}-{}",
            std::process::id(),
            qimen_update_protocol::epoch_millis()
        ))
    }

    #[tokio::test]
    async fn apply_and_rollback_restore_daemon_atomically() {
        let root = test_root();
        let update_dir = root.join(".qimen-update");
        let staging_dir = update_dir.join("staging/v0.2.0");
        let backup_dir = update_dir.join("backups/v0.2.0");
        let daemon_target = root.join(crate::config::daemon_file_name());
        let staged_daemon = staging_dir.join(crate::config::daemon_file_name());
        fs::create_dir_all(&staging_dir).unwrap();
        fs::write(&daemon_target, b"old-daemon").unwrap();
        fs::write(&staged_daemon, b"new-daemon").unwrap();

        let plan = UpdatePlan {
            schema_version: 1,
            previous_version: "0.1.0".to_string(),
            version: "0.2.0".to_string(),
            target: "test-target".to_string(),
            update_dir: update_dir.clone(),
            staging_dir: staging_dir.clone(),
            backup_dir,
            staged_daemon,
            daemon_target: daemon_target.clone(),
        };
        let plan_path = staging_dir.join("update-plan.json");
        write_plan(&plan_path, &plan).unwrap();
        write_status(
            &update_dir,
            &UpdateStatus::managed("0.1.0", "0.1.0", "test-target", "stable", false),
        )
        .unwrap();

        apply_plan(&plan_path).await.unwrap();
        assert_eq!(fs::read(&daemon_target).unwrap(), b"new-daemon");
        assert_eq!(pending_plan(&update_dir).unwrap(), Some(plan_path.clone()));

        // 模拟替换完成后 launcher 与文件系统同时中断，回滚仍须恢复缺失的目标文件。
        fs::remove_file(&daemon_target).unwrap();
        rollback_plan(&plan_path, "health timeout").await.unwrap();
        assert_eq!(fs::read(&daemon_target).unwrap(), b"old-daemon");
        assert!(pending_plan(&update_dir).unwrap().is_none());
        let status = qimen_update_protocol::read_status(&update_dir)
            .unwrap()
            .unwrap();
        assert_eq!(status.phase, UpdatePhase::RolledBack);
        assert_eq!(status.current_version, "0.1.0");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn update_plan_rejects_parent_directory_components() {
        let root = test_root();
        let update_dir = root.join(".qimen-update");
        let staging_dir = update_dir.join("staging/v0.2.0");
        let plan = UpdatePlan {
            schema_version: 1,
            previous_version: "0.1.0".to_string(),
            version: "0.2.0".to_string(),
            target: "test-target".to_string(),
            update_dir: update_dir.clone(),
            staging_dir: staging_dir.clone(),
            backup_dir: update_dir.join("backups/../../outside"),
            staged_daemon: staging_dir.join(crate::config::daemon_file_name()),
            daemon_target: root.join(crate::config::daemon_file_name()),
        };
        assert!(validate_plan(&plan).is_err());
    }
}
