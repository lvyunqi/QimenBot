use qimen_config::{AppConfig, BotConfig};
use qimen_observability::LogEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiEnvelope<T> {
    pub data: T,
}

impl<T> ApiEnvelope<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Debug, Serialize)]
pub struct AdminSnapshot {
    pub server: ServerView,
    pub metrics: MetricsView,
    pub resources: ResourcesView,
    pub bots: Vec<BotView>,
    pub recent_logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
pub struct ServerView {
    pub version: String,
    pub environment: String,
    pub uptime_secs: u64,
    pub now: String,
    pub config_revision: String,
    pub restart_required: bool,
}

#[derive(Debug, Serialize)]
pub struct MetricsView {
    pub events_total: u64,
    pub replies_total: u64,
    pub online_bots: usize,
    pub configured_bots: usize,
    pub loaded_dynamic_plugins: usize,
    pub warning_count: usize,
    pub throughput: Vec<ThroughputView>,
}

#[derive(Debug, Serialize)]
pub struct ThroughputView {
    pub minute_epoch: u64,
    pub events: u64,
    pub replies: u64,
}

#[derive(Debug, Serialize)]
pub struct ResourcesView {
    pub log_entries: usize,
    pub log_capacity: usize,
    pub dynamic_plugin_failures: u64,
    pub active_bot_supervisors: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BotView {
    pub id: String,
    pub account_id: Option<String>,
    pub protocol: String,
    pub transport: String,
    pub endpoint: Option<String>,
    pub bind: Option<String>,
    pub path: Option<String>,
    pub appid: Option<String>,
    pub secret_configured: bool,
    pub access_token_configured: bool,
    pub intents: Vec<String>,
    pub sandbox: bool,
    pub configured_enabled: bool,
    pub desired_enabled: bool,
    pub state: String,
    pub state_since_epoch_ms: u64,
    pub last_event_epoch_ms: Option<u64>,
    pub events_received: u64,
    pub reconnect_count: u64,
    pub last_error: Option<String>,
    pub enabled_modules: Vec<String>,
    pub owners: Vec<String>,
    pub admins: Vec<String>,
    pub auto_approve_friend_requests: bool,
    pub auto_approve_group_invites: bool,
    pub auto_reply_poke_enabled: bool,
    pub auto_reply_poke_message: Option<String>,
    pub limiter: RateLimiterView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimiterView {
    pub enable: bool,
    pub rate: f64,
    pub capacity: u32,
    pub timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct ConfigView {
    pub revision: String,
    pub restart_required: bool,
    pub general: GeneralConfigView,
    pub bots: Vec<BotView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfigView {
    pub environment: String,
    pub shutdown_timeout_secs: u64,
    pub task_grace_secs: u64,
    pub log_level: String,
    pub json_logs: bool,
    pub admin_enabled: bool,
    pub admin_bind: String,
    pub admin_token_configured: bool,
    pub log_capacity: usize,
    pub audit_path: String,
    pub builtin_modules: Vec<String>,
    pub plugin_modules: Vec<String>,
    pub plugin_state_path: String,
    pub plugin_bin_dir: String,
    pub dynamic_plugin_timeout_secs: u64,
    pub proactive_queue_capacity: usize,
    pub proactive_offline_ttl_secs: u64,
    pub webhook_enabled: bool,
    pub webhook_bind: String,
    pub webhook_base_path: String,
    pub webhook_max_body_bytes: usize,
    pub webhook_request_timeout_ms: u64,
    pub webhook_max_in_flight: usize,
    pub webhook_token_configured: bool,
}

#[derive(Debug, Deserialize)]
pub struct GeneralUpdateRequest {
    pub revision: String,
    pub general: GeneralMutation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneralMutation {
    pub environment: String,
    pub shutdown_timeout_secs: u64,
    pub task_grace_secs: u64,
    pub log_level: String,
    pub json_logs: bool,
    pub admin_enabled: bool,
    pub admin_bind: String,
    pub admin_access_token: Option<String>,
    pub log_capacity: usize,
    pub audit_path: String,
    pub builtin_modules: Vec<String>,
    pub plugin_modules: Vec<String>,
    pub plugin_state_path: String,
    pub plugin_bin_dir: String,
    pub dynamic_plugin_timeout_secs: u64,
    pub proactive_queue_capacity: usize,
    pub proactive_offline_ttl_secs: u64,
    pub webhook_enabled: bool,
    pub webhook_bind: String,
    pub webhook_base_path: String,
    pub webhook_max_body_bytes: usize,
    pub webhook_request_timeout_ms: u64,
    pub webhook_max_in_flight: usize,
    pub webhook_access_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BotSaveRequest {
    pub revision: String,
    pub bot: BotMutation,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotMutation {
    pub id: String,
    pub account_id: Option<String>,
    pub protocol: String,
    pub transport: String,
    pub endpoint: Option<String>,
    pub bind: Option<String>,
    pub path: Option<String>,
    pub access_token: Option<String>,
    pub appid: Option<String>,
    pub secret: Option<String>,
    pub intents: Vec<String>,
    pub sandbox: bool,
    pub enabled: bool,
    pub enabled_modules: Vec<String>,
    pub owners: Vec<String>,
    pub admins: Vec<String>,
    pub auto_approve_friend_requests: bool,
    pub auto_approve_group_invites: bool,
    pub auto_reply_poke_enabled: bool,
    pub auto_reply_poke_message: Option<String>,
    pub limiter: RateLimiterView,
}

#[derive(Debug, Deserialize)]
pub struct DeleteRequest {
    pub revision: String,
}

#[derive(Debug, Deserialize)]
pub struct BotActionRequest {
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct MutationResult {
    pub revision: Option<String>,
    pub restart_required: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginView {
    pub id: String,
    pub kind: String,
    pub version: Option<String>,
    pub api_version: Option<String>,
    pub enabled: bool,
    pub loaded: bool,
    pub file_name: Option<String>,
    pub commands: Vec<String>,
    pub routes: Vec<String>,
    pub webhooks: Vec<String>,
    pub failures: u32,
    pub last_error: Option<String>,
    pub live_toggle: bool,
}

#[derive(Debug, Deserialize)]
pub struct PluginToggleRequest {
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct RevisionView {
    pub revision: String,
    pub created_at: String,
    pub size_bytes: u64,
    pub current: bool,
}

#[derive(Debug, Deserialize)]
pub struct RollbackRequest {
    pub revision: String,
}

#[derive(Debug, Deserialize)]
pub struct LogQuery {
    pub limit: Option<usize>,
    pub level: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogsView {
    pub entries: Vec<LogEntry>,
    pub total_buffered: usize,
    pub capacity: usize,
}

pub fn general_view(config: &AppConfig) -> GeneralConfigView {
    GeneralConfigView {
        environment: config.runtime.env.clone(),
        shutdown_timeout_secs: config.runtime.shutdown_timeout_secs,
        task_grace_secs: config.runtime.task_grace_secs,
        log_level: config.observability.level.clone(),
        json_logs: config.observability.json_logs,
        admin_enabled: config.admin_web.enabled,
        admin_bind: config.admin_web.bind.clone(),
        admin_token_configured: !config.admin_web.access_token.trim().is_empty(),
        log_capacity: config.admin_web.log_capacity,
        audit_path: config.admin_web.audit_path.clone(),
        builtin_modules: config.official_host.builtin_modules.clone(),
        plugin_modules: config.official_host.plugin_modules.clone(),
        plugin_state_path: config.official_host.plugin_state_path.clone(),
        plugin_bin_dir: config.official_host.plugin_bin_dir.clone(),
        dynamic_plugin_timeout_secs: config.official_host.dynamic_plugin_timeout_secs,
        proactive_queue_capacity: config.official_host.proactive_send.queue_capacity,
        proactive_offline_ttl_secs: config.official_host.proactive_send.offline_ttl_secs,
        webhook_enabled: config.official_host.webhook.enabled,
        webhook_bind: config.official_host.webhook.bind.clone(),
        webhook_base_path: config.official_host.webhook.base_path.clone(),
        webhook_max_body_bytes: config.official_host.webhook.max_body_bytes,
        webhook_request_timeout_ms: config.official_host.webhook.request_timeout_ms,
        webhook_max_in_flight: config.official_host.webhook.max_in_flight,
        webhook_token_configured: !config.official_host.webhook.access_token.trim().is_empty(),
    }
}

pub fn configured_bot_view(bot: &BotConfig) -> BotView {
    BotView {
        id: bot.id.clone(),
        account_id: bot.account_id.clone(),
        protocol: bot.protocol.clone(),
        transport: bot.transport.clone(),
        endpoint: bot.endpoint.clone(),
        bind: bot.bind.clone(),
        path: bot.path.clone(),
        appid: bot.appid.clone(),
        secret_configured: bot
            .secret
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        access_token_configured: bot
            .access_token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty()),
        intents: bot.intents.clone(),
        sandbox: bot.sandbox,
        configured_enabled: bot.enabled,
        desired_enabled: bot.enabled,
        state: if bot.enabled { "starting" } else { "disabled" }.to_string(),
        state_since_epoch_ms: 0,
        last_event_epoch_ms: None,
        events_received: 0,
        reconnect_count: 0,
        last_error: None,
        enabled_modules: bot.enabled_modules.clone(),
        owners: bot.owners.clone(),
        admins: bot.admins.clone(),
        auto_approve_friend_requests: bot.auto_approve_friend_requests,
        auto_approve_group_invites: bot.auto_approve_group_invites,
        auto_reply_poke_enabled: bot.auto_reply_poke_enabled,
        auto_reply_poke_message: bot.auto_reply_poke_message.clone(),
        limiter: RateLimiterView {
            enable: bot.limiter.enable,
            rate: bot.limiter.rate,
            capacity: bot.limiter.capacity,
            timeout_secs: bot.limiter.timeout_secs,
        },
    }
}
