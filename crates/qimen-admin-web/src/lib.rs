mod assets;
mod audit;
mod config_store;
mod error;
mod marketplace;
mod plugin_config;
mod types;

use audit::{AuditLog, AuditPage};
use axum::extract::{DefaultBodyLimit, Path, Query, Request, State};
use axum::http::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{SecondsFormat, Utc};
use config_store::{ConfigStore, StoredConfig};
use error::AdminError;
use futures_util::stream;
use qimen_config::{AdminWebConfig, AppConfig};
use qimen_host_types::{
    MAX_PLUGIN_PRIORITY, PluginState, default_plugin_priority, load_plugin_state,
};
use qimen_observability::{LogEntry, LogStore};
use qimen_runtime::dynamic_runtime::scan_dynamic_plugins;
use qimen_runtime::{BotConnectionState, BotStatusSnapshot, Runtime};
use qimen_update_protocol::{
    DeploymentKind, LauncherCommandAction, deployment_kind, enqueue_launcher_command,
    managed_update_dir, read_status,
};
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::Path as FsPath;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::net::TcpListener;
use types::*;

#[derive(Clone)]
struct AdminState {
    runtime: Arc<Runtime>,
    config_store: ConfigStore,
    log_store: LogStore,
    audit: AuditLog,
    restart_required: Arc<AtomicBool>,
    marketplace_operations: Arc<tokio::sync::Mutex<()>>,
    plugin_operations: Arc<tokio::sync::Mutex<()>>,
}

#[derive(Clone)]
struct AuthState {
    token: Arc<str>,
}

pub struct AdminServer {
    state: AdminState,
    config: AdminWebConfig,
}

impl AdminServer {
    pub fn new(
        config_path: impl Into<std::path::PathBuf>,
        config: &AppConfig,
        runtime: Arc<Runtime>,
        log_store: LogStore,
    ) -> Self {
        Self {
            state: AdminState {
                runtime,
                config_store: ConfigStore::new(config_path),
                log_store,
                audit: AuditLog::open(&config.admin_web.audit_path),
                restart_required: Arc::new(AtomicBool::new(false)),
                marketplace_operations: Arc::new(tokio::sync::Mutex::new(())),
                plugin_operations: Arc::new(tokio::sync::Mutex::new(())),
            },
            config: config.admin_web.clone(),
        }
    }

    pub async fn serve(self) -> qimen_error::Result<()> {
        let bind = self.config.bind.parse::<SocketAddr>().map_err(|error| {
            qimen_error::QimenError::Config(format!(
                "invalid admin_web.bind '{}': {}",
                self.config.bind, error
            ))
        })?;
        let auth = AuthState {
            token: Arc::<str>::from(self.config.access_token.clone()),
        };
        let shutdown_runtime = Arc::clone(&self.state.runtime);
        let api = Router::new()
            .route("/health", get(health))
            .route("/snapshot", get(snapshot))
            .route("/bots", get(bots).post(create_bot))
            .route("/bots/{id}", put(update_bot).delete(delete_bot))
            .route("/bots/{id}/actions", post(bot_action))
            .route("/logs", get(logs))
            .route("/logs/stream", get(log_stream))
            .route("/plugins", get(plugins))
            .route("/plugins/reload", post(reload_plugins))
            .route(
                "/plugins/{id}/config",
                get(plugin_config::get).put(plugin_config::save),
            )
            .route(
                "/plugins/{id}/config/validate",
                post(plugin_config::validate),
            )
            .route("/plugins/{id}/priority", put(update_plugin_priority))
            .route("/plugins/{id}", put(toggle_plugin))
            .route("/marketplace", get(marketplace::catalog))
            .route("/marketplace/refresh", post(marketplace::refresh))
            .route(
                "/marketplace/plugins/{id}",
                get(marketplace::detail).delete(marketplace::uninstall),
            )
            .route(
                "/marketplace/plugins/{id}/install",
                post(marketplace::install),
            )
            .route("/marketplace/plugins/{id}/adopt", post(marketplace::adopt))
            .route("/marketplace/plugins/{id}/pin", put(marketplace::pin))
            .route(
                "/marketplace/plugins/{id}/rollback",
                post(marketplace::rollback),
            )
            .route("/config", get(configuration))
            .route("/config/general", put(update_general))
            .route("/config/revisions", get(revisions))
            .route("/config/rollback", post(rollback))
            .route("/updates", get(updates))
            .route("/updates/check", post(check_updates))
            .route("/updates/install", post(install_update))
            .route("/updates/restart", post(restart_runtime))
            .route("/audit", get(audit_entries))
            .layer(DefaultBodyLimit::max(512 * 1024))
            .route_layer(middleware::from_fn_with_state(auth, require_auth))
            .with_state(self.state);
        let app = Router::new()
            .nest("/api/v1", api)
            .route("/healthz", get(health))
            .route("/", get(assets::index))
            .route("/{*path}", get(assets::spa));

        let listener = TcpListener::bind(bind).await?;
        tracing::info!(bind = %bind, url = %format!("http://{bind}"), "QimenBot admin web panel listening");
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { shutdown_runtime.wait_for_shutdown().await })
            .await
            .map_err(|error| qimen_error::QimenError::Runtime(error.to_string()))
    }
}

async fn require_auth(State(auth): State<AuthState>, request: Request, next: Next) -> Response {
    if auth.token.is_empty() {
        return next.run(request).await;
    }
    let supplied = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if constant_time_eq(auth.token.as_bytes(), supplied.as_bytes()) {
        next.run(request).await
    } else {
        let mut response = AdminError::Unauthorized.into_response();
        response.headers_mut().insert(
            WWW_AUTHENTICATE,
            HeaderValue::from_static("Bearer realm=\"QimenBot Admin\""),
        );
        response
    }
}

fn constant_time_eq(expected: &[u8], actual: &[u8]) -> bool {
    if expected.len() != actual.len() {
        return false;
    }
    expected
        .iter()
        .zip(actual)
        .fold(0_u8, |difference, (left, right)| {
            difference | (left ^ right)
        })
        == 0
}

async fn health() -> Json<ApiEnvelope<HealthView>> {
    Json(ApiEnvelope::new(HealthView {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Serialize)]
struct HealthView {
    status: &'static str,
    version: &'static str,
}

async fn snapshot(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<AdminSnapshot>>, AdminError> {
    let stored = state.config_store.read().await?;
    let statuses = state.runtime.bot_statuses();
    let bots = merge_bot_views(&stored, &statuses);
    let runtime_metrics = state.runtime.metrics_snapshot();
    let dynamic_health = state.runtime.dynamic_plugin_health();
    let loaded_dynamic_plugins = state
        .runtime
        .host_plugin_report()
        .map(|report| report.dynamic_plugins.len())
        .unwrap_or_default();
    let warning_count = statuses
        .iter()
        .filter(|status| {
            matches!(
                status.state,
                BotConnectionState::Reconnecting | BotConnectionState::Error
            )
        })
        .count()
        + dynamic_health
            .iter()
            .filter(|entry| entry.failures > 0)
            .count();
    let mut recent_logs = state.log_store.entries();
    recent_logs.reverse();
    recent_logs.truncate(6);
    Ok(Json(ApiEnvelope::new(AdminSnapshot {
        server: ServerView {
            version: env!("CARGO_PKG_VERSION").to_string(),
            environment: stored.config.runtime.env.clone(),
            uptime_secs: current_epoch_millis().saturating_sub(runtime_metrics.started_at_epoch_ms)
                / 1_000,
            now: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            config_revision: stored.revision,
            restart_required: state.restart_required.load(Ordering::Relaxed),
        },
        metrics: MetricsView {
            events_total: runtime_metrics.events_total,
            replies_total: runtime_metrics.replies_total,
            online_bots: statuses
                .iter()
                .filter(|status| matches!(status.state, BotConnectionState::Online))
                .count(),
            configured_bots: bots.len(),
            loaded_dynamic_plugins,
            warning_count,
            throughput: runtime_metrics
                .throughput
                .into_iter()
                .map(|point| ThroughputView {
                    minute_epoch: point.minute_epoch,
                    events: point.events,
                    replies: point.replies,
                })
                .collect(),
        },
        resources: ResourcesView {
            log_entries: state.log_store.len(),
            log_capacity: state.log_store.capacity(),
            dynamic_plugin_failures: dynamic_health
                .iter()
                .map(|entry| u64::from(entry.failures))
                .sum(),
            active_bot_supervisors: statuses
                .iter()
                .filter(|status| status.desired_enabled)
                .count(),
        },
        bots,
        recent_logs,
    })))
}

async fn bots(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<Vec<BotView>>>, AdminError> {
    let stored = state.config_store.read().await?;
    Ok(Json(ApiEnvelope::new(merge_bot_views(
        &stored,
        &state.runtime.bot_statuses(),
    ))))
}

async fn configuration(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<ConfigView>>, AdminError> {
    let stored = state.config_store.read().await?;
    let bots = merge_bot_views(&stored, &state.runtime.bot_statuses());
    Ok(Json(ApiEnvelope::new(ConfigView {
        revision: stored.revision,
        restart_required: state.restart_required.load(Ordering::Relaxed),
        general: general_view(&stored.config),
        bots,
    })))
}

async fn update_general(
    State(state): State<AdminState>,
    Json(request): Json<GeneralUpdateRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let before = state.config_store.read().await?;
    let before_general = general_view(&before.config);
    let saved = state
        .config_store
        .update_general(&request.revision, &request.general)
        .await?;
    let saved_general = general_view(&saved.config);
    let command_changed =
        before.config.official_host.commands != saved.config.official_host.commands;

    let mut comparable_before = before_general;
    comparable_before.command_help_enabled = saved_general.command_help_enabled;
    comparable_before.command_help_page_size = saved_general.command_help_page_size;
    comparable_before.command_plugins_enabled = saved_general.command_plugins_enabled;
    comparable_before.command_registry_enabled = saved_general.command_registry_enabled;
    comparable_before.command_dynamic_errors_enabled = saved_general.command_dynamic_errors_enabled;
    comparable_before.command_prefixes = saved_general.command_prefixes.clone();
    comparable_before.command_private_bare_enabled = saved_general.command_private_bare_enabled;
    comparable_before.command_group_bare_enabled = saved_general.command_group_bare_enabled;
    comparable_before.command_mention_enabled = saved_general.command_mention_enabled;
    comparable_before.command_reply_enabled = saved_general.command_reply_enabled;
    let other_settings_changed = comparable_before != saved_general;

    if command_changed {
        state
            .runtime
            .set_command_config(saved.config.official_host.commands.clone())?;
    }
    let restart_required = state.restart_required.load(Ordering::Relaxed) || other_settings_changed;
    state
        .restart_required
        .store(restart_required, Ordering::Relaxed);
    let detail = match (command_changed, other_settings_changed) {
        (true, false) => "command settings updated live; bot reconnect requested",
        (true, true) => "command settings updated live; other host settings require restart",
        (false, true) => "general host settings updated; restart required",
        (false, false) => "general host settings saved without effective changes",
    };
    state
        .audit
        .record("config.update", "general", "success", detail)?;
    let message = match (command_changed, other_settings_changed, restart_required) {
        (true, false, false) => "命令配置已保存，Bot 正在重连并应用新入口".to_string(),
        (true, true, _) => "命令入口已动态应用；其余待处理配置仍需重启宿主".to_string(),
        (true, false, true) => "命令入口已动态应用；此前的修改仍在等待重启".to_string(),
        (false, true, _) => "配置已保存，重启宿主后全部生效".to_string(),
        (false, false, true) => "配置没有变化；此前的修改仍在等待重启".to_string(),
        (false, false, false) => "配置没有变化".to_string(),
    };
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(saved.revision),
        restart_required,
        message,
    })))
}

async fn create_bot(
    State(state): State<AdminState>,
    Json(request): Json<BotSaveRequest>,
) -> Result<(StatusCode, Json<ApiEnvelope<MutationResult>>), AdminError> {
    let saved = state
        .config_store
        .save_bot(None, &request.revision, &request.bot)
        .await?;
    state.restart_required.store(true, Ordering::Relaxed);
    state.audit.record(
        "bot.create",
        format!("bot:{}", request.bot.id),
        "success",
        "bot configuration created; restart required",
    )?;
    Ok((
        StatusCode::CREATED,
        Json(ApiEnvelope::new(MutationResult {
            revision: Some(saved.revision),
            restart_required: true,
            message: "机器人配置已创建，重启宿主后启动".to_string(),
        })),
    ))
}

async fn update_bot(
    State(state): State<AdminState>,
    Path(bot_id): Path<String>,
    Json(request): Json<BotSaveRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let before = state.config_store.read().await?;
    let old_enabled = before
        .config
        .bots
        .iter()
        .find(|bot| bot.id == bot_id)
        .map(|bot| bot.enabled)
        .ok_or_else(|| AdminError::NotFound(format!("bot '{}' was not found", bot_id)))?;
    let saved = state
        .config_store
        .save_bot(Some(&bot_id), &request.revision, &request.bot)
        .await?;
    state.restart_required.store(true, Ordering::Relaxed);
    if request.bot.id == bot_id && old_enabled != request.bot.enabled {
        state
            .runtime
            .set_bot_enabled(&bot_id, request.bot.enabled)?;
    }
    state.audit.record(
        "bot.update",
        format!("bot:{}", bot_id),
        "success",
        "bot configuration updated; connection switch applied live when changed",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(saved.revision),
        restart_required: true,
        message: "配置已保存；启停开关已即时应用，其余修改重启后生效".to_string(),
    })))
}

async fn delete_bot(
    State(state): State<AdminState>,
    Path(bot_id): Path<String>,
    Json(request): Json<DeleteRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _ = state.runtime.set_bot_enabled(&bot_id, false);
    let saved = state
        .config_store
        .delete_bot(&bot_id, &request.revision)
        .await?;
    state.restart_required.store(true, Ordering::Relaxed);
    state.audit.record(
        "bot.delete",
        format!("bot:{}", bot_id),
        "success",
        "bot stopped and removed from configuration; restart required",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(saved.revision),
        restart_required: true,
        message: "机器人已停止并从配置删除，重启后彻底移除".to_string(),
    })))
}

async fn bot_action(
    State(state): State<AdminState>,
    Path(bot_id): Path<String>,
    Json(request): Json<BotActionRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let message = match request.action.as_str() {
        "start" => {
            state.runtime.set_bot_enabled(&bot_id, true)?;
            "启动请求已发送"
        }
        "stop" => {
            state.runtime.set_bot_enabled(&bot_id, false)?;
            "停止请求已发送"
        }
        "reconnect" => {
            state.runtime.reconnect_bot(&bot_id)?;
            "重连请求已发送"
        }
        _ => {
            return Err(AdminError::BadRequest(
                "action must be start, stop, or reconnect".to_string(),
            ));
        }
    };
    state.audit.record(
        format!("bot.{}", request.action),
        format!("bot:{}", bot_id),
        "success",
        message,
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: state.restart_required.load(Ordering::Relaxed),
        message: message.to_string(),
    })))
}

async fn logs(
    State(state): State<AdminState>,
    Query(query): Query<LogQuery>,
) -> Json<ApiEnvelope<LogsView>> {
    let level = query.level.as_deref().map(str::to_ascii_lowercase);
    let needle = query.query.as_deref().map(str::to_ascii_lowercase);
    let limit = query.limit.unwrap_or(300).clamp(1, 2_000);
    let mut entries: Vec<LogEntry> = state
        .log_store
        .entries()
        .into_iter()
        .rev()
        .filter(|entry| level.as_ref().is_none_or(|level| entry.level == *level))
        .filter(|entry| {
            needle.as_ref().is_none_or(|needle| {
                entry.message.to_ascii_lowercase().contains(needle)
                    || entry.target.to_ascii_lowercase().contains(needle)
                    || entry
                        .fields
                        .values()
                        .any(|value| value.to_ascii_lowercase().contains(needle))
            })
        })
        .take(limit)
        .collect();
    entries.reverse();
    Json(ApiEnvelope::new(LogsView {
        entries,
        total_buffered: state.log_store.len(),
        capacity: state.log_store.capacity(),
    }))
}

async fn log_stream(
    State(state): State<AdminState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let receiver = state.log_store.subscribe();
    let stream = stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(entry) => {
                    let event = Event::default()
                        .id(entry.id.to_string())
                        .event("log")
                        .json_data(entry)
                        .unwrap_or_else(|_| Event::default().event("log").data("{}"));
                    return Some((Ok(event), receiver));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

async fn plugins(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<Vec<PluginView>>>, AdminError> {
    Ok(Json(ApiEnvelope::new(plugin_views(&state).await?)))
}

async fn reload_plugins(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    let count = state.runtime.reload_dynamic_plugins().await?;
    state.audit.record(
        "plugin.reload",
        "dynamic-plugins",
        "success",
        format!("{} dynamic plugins loaded", count),
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: state.restart_required.load(Ordering::Relaxed),
        message: format!("已重新载入 {} 个动态插件", count),
    })))
}

async fn toggle_plugin(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<PluginToggleRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    let stored = state.config_store.read().await?;
    let dynamic = scan_dynamic_plugins(&stored.config.official_host.plugin_bin_dir)?
        .into_iter()
        .any(|plugin| plugin.plugin_id == plugin_id);
    let static_plugin = stored
        .config
        .official_host
        .plugin_modules
        .iter()
        .any(|id| id == &plugin_id);
    if !dynamic && !static_plugin {
        return Err(AdminError::NotFound(format!(
            "plugin '{}' was not found",
            plugin_id
        )));
    }
    let path = &stored.config.official_host.plugin_state_path;
    let mut plugin_state = load_plugin_state(path)?;
    plugin_state.set_enabled(plugin_id.clone(), request.enabled);
    plugin_state.save_to_path(path)?;
    let restart_required = if dynamic {
        state.runtime.reload_dynamic_plugins().await?;
        state.restart_required.load(Ordering::Relaxed)
    } else {
        state.restart_required.store(true, Ordering::Relaxed);
        true
    };
    state.audit.record(
        if request.enabled {
            "plugin.enable"
        } else {
            "plugin.disable"
        },
        format!("plugin:{}", plugin_id),
        "success",
        if dynamic {
            "dynamic plugin state applied live"
        } else {
            "static plugin state saved; restart required"
        },
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required,
        message: if dynamic {
            "动态插件状态已即时应用".to_string()
        } else {
            "静态插件状态已保存，重启宿主后生效".to_string()
        },
    })))
}

async fn update_plugin_priority(
    State(state): State<AdminState>,
    Path(plugin_id): Path<String>,
    Json(request): Json<PluginPriorityRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let _operation = state.plugin_operations.lock().await;
    if request.priority > MAX_PLUGIN_PRIORITY {
        return Err(AdminError::BadRequest(format!(
            "优先级必须介于 0 和 {MAX_PLUGIN_PRIORITY} 之间"
        )));
    }

    let stored = state.config_store.read().await?;
    let dynamic = scan_dynamic_plugins(&stored.config.official_host.plugin_bin_dir)?
        .into_iter()
        .any(|plugin| plugin.plugin_id == plugin_id);
    let available_modules = state
        .runtime
        .host_plugin_report()
        .map(|report| report.available_modules)
        .unwrap_or_default();
    let static_plugin = stored
        .config
        .official_host
        .plugin_modules
        .iter()
        .any(|id| id == &plugin_id)
        || available_modules
            .iter()
            .any(|module| module.kind == "static" && module.id == plugin_id);
    let builtin = stored
        .config
        .official_host
        .builtin_modules
        .iter()
        .any(|id| id == &plugin_id)
        || available_modules
            .iter()
            .any(|module| module.kind == "builtin" && module.id == plugin_id);
    if builtin {
        return Err(AdminError::BadRequest(
            "内置模块的优先级由宿主保留，不能修改".to_string(),
        ));
    }
    if !dynamic && !static_plugin {
        return Err(AdminError::NotFound(format!(
            "plugin '{}' was not found",
            plugin_id
        )));
    }

    let kind = if dynamic { "dynamic" } else { "static" };
    let path = &stored.config.official_host.plugin_state_path;
    let mut plugin_state = load_plugin_state(path)?;
    let previous = plugin_state
        .priority(&plugin_id)
        .unwrap_or_else(|| default_plugin_priority(kind));
    plugin_state.set_priority(plugin_id.clone(), request.priority)?;
    plugin_state.save_to_path(path)?;
    state
        .runtime
        .set_plugin_priority(&plugin_id, request.priority)?;
    state.audit.record(
        "plugin.priority.update",
        format!("plugin:{}", plugin_id),
        "success",
        format!("priority changed from {} to {}", previous, request.priority),
    )?;

    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: format!(
            "插件优先级已更新为 {}；已启用 Bot 会重连并刷新命令路由",
            request.priority
        ),
    })))
}

async fn revisions(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<Vec<RevisionView>>>, AdminError> {
    Ok(Json(ApiEnvelope::new(
        state.config_store.revisions().await?,
    )))
}

async fn rollback(
    State(state): State<AdminState>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let saved = state.config_store.rollback(&request.revision).await?;
    state.restart_required.store(true, Ordering::Relaxed);
    state.audit.record(
        "config.rollback",
        format!("revision:{}", request.revision),
        "success",
        "configuration revision restored; restart required",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(saved.revision),
        restart_required: true,
        message: "配置版本已恢复，重启宿主后生效".to_string(),
    })))
}

async fn audit_entries(
    State(state): State<AdminState>,
    Query(query): Query<AuditQuery>,
) -> Json<ApiEnvelope<AuditPage>> {
    Json(ApiEnvelope::new(state.audit.page(
        query.page.unwrap_or(1),
        query.page_size.unwrap_or(20),
    )))
}

async fn updates() -> Result<Json<ApiEnvelope<UpdateView>>, AdminError> {
    let deployment = deployment_kind();
    let status = managed_update_dir()
        .as_deref()
        .map(read_status)
        .transpose()?
        .flatten();
    let message = match deployment {
        DeploymentKind::BinaryManaged => status
            .as_ref()
            .map(|value| value.message.clone())
            .unwrap_or_else(|| "qimenbot 尚未写入更新状态".to_string()),
        DeploymentKind::Docker => {
            "当前由 Docker 编排层管理，请使用 docker compose pull && docker compose up -d 更新"
                .to_string()
        }
        DeploymentKind::DirectBinary => {
            "当前未由 qimenbot 启动，请改用 qimenbot run 启用受控更新".to_string()
        }
    };
    Ok(Json(ApiEnvelope::new(UpdateView {
        deployment,
        managed: matches!(deployment, DeploymentKind::BinaryManaged),
        status,
        message,
    })))
}

async fn check_updates(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    queue_update_command(&state, LauncherCommandAction::Check, "更新检查请求已发送").await
}

async fn install_update(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    queue_update_command(&state, LauncherCommandAction::Install, "更新安装请求已发送").await
}

async fn restart_runtime(
    State(state): State<AdminState>,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    queue_update_command(&state, LauncherCommandAction::Restart, "重启请求已发送").await
}

async fn queue_update_command(
    state: &AdminState,
    action: LauncherCommandAction,
    message: &str,
) -> Result<Json<ApiEnvelope<MutationResult>>, AdminError> {
    let Some(update_dir) = managed_update_dir() else {
        return Err(AdminError::Conflict(
            "当前进程没有由 qimenbot 管理，无法执行受控更新或重启".to_string(),
        ));
    };
    let id = enqueue_launcher_command(&update_dir, action)?;
    state.audit.record(
        match action {
            LauncherCommandAction::Check => "update.check",
            LauncherCommandAction::Install => "update.install",
            LauncherCommandAction::Restart => "runtime.restart",
        },
        "qimenbot",
        "success",
        format!("{}（命令 {}）", message, id),
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: None,
        restart_required: false,
        message: message.to_string(),
    })))
}

async fn plugin_views(state: &AdminState) -> Result<Vec<PluginView>, AdminError> {
    let stored = state.config_store.read().await?;
    let persisted = load_plugin_state(&stored.config.official_host.plugin_state_path)?;
    let loaded =
        state
            .runtime
            .host_plugin_report()
            .unwrap_or_else(|| qimen_host_types::HostPluginReport {
                builtin_modules: Vec::new(),
                configured_plugins: Vec::new(),
                available_modules: Vec::new(),
                persisted_states: Default::default(),
                dynamic_plugins: Vec::new(),
            });
    let health = state.runtime.dynamic_plugin_health();
    let health_by_path: HashMap<_, _> = health
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut views = Vec::new();
    let mut represented_builtin = std::collections::HashSet::new();
    let mut represented_static = std::collections::HashSet::new();
    for module in &loaded.available_modules {
        let (priority, priority_custom) =
            plugin_priority_view(&persisted, &module.id, &module.kind);
        let configured = match module.kind.as_str() {
            "builtin" => stored
                .config
                .official_host
                .builtin_modules
                .iter()
                .any(|id| id == &module.id),
            "static" => stored
                .config
                .official_host
                .plugin_modules
                .iter()
                .any(|id| id == &module.id),
            _ => false,
        };
        let enabled = configured && (module.kind == "builtin" || persisted.is_enabled(&module.id));
        if module.kind == "builtin" {
            represented_builtin.insert(module.id.clone());
        } else if module.kind == "static" {
            represented_static.insert(module.id.clone());
        }
        views.push(PluginView {
            id: module.id.clone(),
            kind: module.kind.clone(),
            name: Some(module.name.clone()),
            description: (!module.description.is_empty()).then(|| module.description.clone()),
            version: Some(module.version.clone()),
            api_version: Some(module.api_version.clone()),
            configured,
            available: true,
            enabled,
            loaded: enabled,
            file_name: None,
            commands: module.commands.clone(),
            routes: Vec::new(),
            system_plugins: module.system_plugins.clone(),
            interceptors: module.interceptors,
            webhooks: Vec::new(),
            failures: 0,
            last_error: None,
            live_toggle: false,
            configurable: false,
            config_apply_mode: None,
            config_version: None,
            config_file_exists: false,
            priority,
            priority_custom,
        });
    }

    for id in &stored.config.official_host.builtin_modules {
        if represented_builtin.contains(id) {
            continue;
        }
        let (priority, priority_custom) = plugin_priority_view(&persisted, id, "builtin");
        views.push(PluginView {
            id: id.clone(),
            kind: "builtin".to_string(),
            name: None,
            description: None,
            version: None,
            api_version: None,
            configured: true,
            available: false,
            enabled: true,
            loaded: false,
            file_name: None,
            commands: Vec::new(),
            routes: Vec::new(),
            system_plugins: Vec::new(),
            interceptors: 0,
            webhooks: Vec::new(),
            failures: 0,
            last_error: Some("当前二进制未发现该内置模块".to_string()),
            live_toggle: false,
            configurable: false,
            config_apply_mode: None,
            config_version: None,
            config_file_exists: false,
            priority,
            priority_custom,
        });
    }
    for id in &stored.config.official_host.plugin_modules {
        if represented_static.contains(id) {
            continue;
        }
        let (priority, priority_custom) = plugin_priority_view(&persisted, id, "static");
        views.push(PluginView {
            id: id.clone(),
            kind: "static".to_string(),
            name: None,
            description: None,
            version: None,
            api_version: Some("0.1".to_string()),
            configured: true,
            available: false,
            enabled: persisted.is_enabled(id),
            loaded: false,
            file_name: None,
            commands: Vec::new(),
            routes: Vec::new(),
            system_plugins: Vec::new(),
            interceptors: 0,
            webhooks: Vec::new(),
            failures: 0,
            last_error: Some("当前二进制未发现该静态插件".to_string()),
            live_toggle: false,
            configurable: false,
            config_apply_mode: None,
            config_version: None,
            config_file_exists: false,
            priority,
            priority_custom,
        });
    }
    for plugin in scan_dynamic_plugins(&stored.config.official_host.plugin_bin_dir)? {
        let (priority, priority_custom) =
            plugin_priority_view(&persisted, &plugin.plugin_id, "dynamic");
        let health = health_by_path.get(&plugin.path);
        views.push(PluginView {
            id: plugin.plugin_id.clone(),
            kind: "dynamic".to_string(),
            name: Some(plugin.plugin_id.clone()),
            description: plugin
                .commands
                .iter()
                .find(|command| !command.description.trim().is_empty())
                .map(|command| command.description.clone()),
            version: Some(plugin.plugin_version.clone()),
            api_version: Some(plugin.api_version.clone()),
            configured: true,
            available: true,
            enabled: persisted.is_enabled(&plugin.plugin_id),
            loaded: loaded
                .dynamic_plugins
                .iter()
                .any(|loaded_plugin| loaded_plugin.plugin_id == plugin.plugin_id),
            file_name: FsPath::new(&plugin.path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string),
            commands: plugin
                .commands
                .iter()
                .map(|command| command.name.clone())
                .collect(),
            routes: plugin
                .routes
                .iter()
                .map(|route| format!("{}:{}", route.kind, route.route))
                .collect(),
            system_plugins: Vec::new(),
            interceptors: plugin.interceptors.len(),
            webhooks: plugin
                .webhooks
                .iter()
                .map(|route| format!("{} {}", route.method, route.path))
                .collect(),
            failures: health.map(|entry| entry.failures).unwrap_or_default(),
            last_error: health.and_then(|entry| entry.last_error.clone()),
            live_toggle: true,
            configurable: plugin.config.is_some(),
            config_apply_mode: plugin
                .config
                .as_ref()
                .map(|config| config.apply_mode.clone()),
            config_version: plugin.config.as_ref().map(|config| config.config_version),
            config_file_exists: FsPath::new(&stored.config.official_host.plugin_config_dir)
                .join(format!("{}.toml", plugin.plugin_id))
                .is_file(),
            priority,
            priority_custom,
        });
    }
    views.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then(left.id.cmp(&right.id))
    });
    Ok(views)
}

fn plugin_priority_view(state: &PluginState, plugin_id: &str, kind: &str) -> (u32, bool) {
    if kind == "builtin" {
        return (default_plugin_priority(kind), false);
    }
    state
        .priority(plugin_id)
        .map(|priority| (priority, true))
        .unwrap_or_else(|| (default_plugin_priority(kind), false))
}

fn merge_bot_views(stored: &StoredConfig, statuses: &[BotStatusSnapshot]) -> Vec<BotView> {
    let status_by_id: HashMap<_, _> = statuses
        .iter()
        .map(|status| (status.id.as_str(), status))
        .collect();
    stored
        .config
        .bots
        .iter()
        .map(|bot| {
            let mut view = configured_bot_view(bot);
            if let Some(status) = status_by_id.get(bot.id.as_str()) {
                view.desired_enabled = status.desired_enabled;
                view.state = connection_state_name(status.state).to_string();
                view.state_since_epoch_ms = status.state_since_epoch_ms;
                view.last_event_epoch_ms = status.last_event_epoch_ms;
                view.events_received = status.events_received;
                view.reconnect_count = status.reconnect_count;
                view.last_error.clone_from(&status.last_error);
            }
            view
        })
        .collect()
}

fn connection_state_name(state: BotConnectionState) -> &'static str {
    match state {
        BotConnectionState::Disabled => "disabled",
        BotConnectionState::Starting => "starting",
        BotConnectionState::Online => "online",
        BotConnectionState::Reconnecting => "reconnecting",
        BotConnectionState::Stopped => "stopped",
        BotConnectionState::Error => "error",
    }
}

fn current_epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_comparison_requires_equal_lengths_and_bytes() {
        assert!(constant_time_eq(b"token", b"token"));
        assert!(!constant_time_eq(b"token", b"tokens"));
        assert!(!constant_time_eq(b"token", b"taken"));
    }
}
