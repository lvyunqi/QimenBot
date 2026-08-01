mod assets;
mod audit;
mod config_store;
mod error;
mod types;

use audit::{AuditEntry, AuditLog};
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
use qimen_host_types::load_plugin_state;
use qimen_observability::{LogEntry, LogStore};
use qimen_runtime::dynamic_runtime::scan_dynamic_plugins;
use qimen_runtime::{BotConnectionState, BotStatusSnapshot, Runtime};
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
            .route("/plugins/{id}", put(toggle_plugin))
            .route("/config", get(configuration))
            .route("/config/general", put(update_general))
            .route("/config/revisions", get(revisions))
            .route("/config/rollback", post(rollback))
            .route("/audit", get(audit_entries))
            .layer(DefaultBodyLimit::max(512 * 1024))
            .route_layer(middleware::from_fn_with_state(auth, require_auth))
            .with_state(self.state);
        let app = Router::new()
            .nest("/api/v1", api)
            .route("/", get(assets::index))
            .route("/{*path}", get(assets::spa));

        let listener = TcpListener::bind(bind).await?;
        tracing::info!(bind = %bind, url = %format!("http://{bind}"), "QimenBot admin web panel listening");
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|error| qimen_error::QimenError::Runtime(error.to_string()))
    }
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
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
    let saved = state
        .config_store
        .update_general(&request.revision, &request.general)
        .await?;
    state.restart_required.store(true, Ordering::Relaxed);
    state.audit.record(
        "config.update",
        "general",
        "success",
        "general host settings updated; restart required",
    )?;
    Ok(Json(ApiEnvelope::new(MutationResult {
        revision: Some(saved.revision),
        restart_required: true,
        message: "配置已保存，重启宿主后全部生效".to_string(),
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

async fn audit_entries(State(state): State<AdminState>) -> Json<ApiEnvelope<Vec<AuditEntry>>> {
    Json(ApiEnvelope::new(state.audit.entries(500)))
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
                persisted_states: Default::default(),
                dynamic_plugins: Vec::new(),
            });
    let health = state.runtime.dynamic_plugin_health();
    let health_by_path: HashMap<_, _> = health
        .into_iter()
        .map(|entry| (entry.path.clone(), entry))
        .collect();
    let mut views = Vec::new();
    for id in &stored.config.official_host.builtin_modules {
        views.push(PluginView {
            id: id.clone(),
            kind: "builtin".to_string(),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            api_version: None,
            enabled: true,
            loaded: true,
            file_name: None,
            commands: Vec::new(),
            routes: Vec::new(),
            webhooks: Vec::new(),
            failures: 0,
            last_error: None,
            live_toggle: false,
        });
    }
    for id in &stored.config.official_host.plugin_modules {
        views.push(PluginView {
            id: id.clone(),
            kind: "static".to_string(),
            version: None,
            api_version: Some("0.1".to_string()),
            enabled: persisted.is_enabled(id),
            loaded: loaded
                .configured_plugins
                .iter()
                .any(|loaded_id| loaded_id == id)
                && persisted.is_enabled(id),
            file_name: None,
            commands: Vec::new(),
            routes: Vec::new(),
            webhooks: Vec::new(),
            failures: 0,
            last_error: None,
            live_toggle: false,
        });
    }
    for plugin in scan_dynamic_plugins(&stored.config.official_host.plugin_bin_dir)? {
        let health = health_by_path.get(&plugin.path);
        views.push(PluginView {
            id: plugin.plugin_id.clone(),
            kind: "dynamic".to_string(),
            version: Some(plugin.plugin_version.clone()),
            api_version: Some(plugin.api_version.clone()),
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
            webhooks: plugin
                .webhooks
                .iter()
                .map(|route| format!("{} {}", route.method, route.path))
                .collect(),
            failures: health.map(|entry| entry.failures).unwrap_or_default(),
            last_error: health.and_then(|entry| entry.last_error.clone()),
            live_toggle: true,
        });
    }
    views.sort_by(|left, right| left.kind.cmp(&right.kind).then(left.id.cmp(&right.id)));
    Ok(views)
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
