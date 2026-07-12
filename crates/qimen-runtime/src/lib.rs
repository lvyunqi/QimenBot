//! QimenBot runtime — event dispatch, plugin orchestration, and bot lifecycle management.

pub mod command_dispatch;
pub mod dedup;
pub mod dynamic_runtime;
pub mod group_event_filter;
pub mod interceptor;
pub mod message_filter;
pub mod onebot11_dispatch;
pub mod permission;
pub mod plugin_acl;
pub mod rate_limiter;

use self::command_dispatch::{CommandDispatchSignal, CommandDispatcher, render_help_text};
use self::dynamic_runtime::{DynamicPluginRuntime, DynamicResponse};
use self::interceptor::InterceptorChain;
use self::onebot11_dispatch::{OneBotSystemDispatchSignal, OneBotSystemDispatcher};
use self::permission::PermissionResolver;
use self::rate_limiter::TokenBucketLimiter;
use crate::dedup::MessageDedup;
use crate::group_event_filter::GroupEventFilter;
use crate::plugin_acl::PluginAclManager;
use abi_stable_host_api::SendAction;
use qimen_adapter_onebot11::OneBot11Adapter;
use qimen_adapter_qqbot::QqBotAdapter;
use qimen_config::{AppConfig, qq_official_intents_value};
use qimen_error::{QimenError, Result};
use qimen_host_types::{
    DynamicCommandDescriptor, DynamicInterceptorDescriptor, HostPluginReport, load_plugin_state,
};
use qimen_message::Message;
use qimen_plugin_api::{
    BuiltinCommandAction, CommandPlugin, OwnedTaskFuture, PluginBundle, RateLimiterConfig,
    RuntimeBotContext, SystemPlugin, TaskHandle,
};
use qimen_protocol_core::{
    ActionMeta, ActionStatus, CapabilitySet, EventKind, IncomingPacket, NormalizedActionRequest,
    NormalizedActionResponse, NormalizedEvent, ProtocolAdapter, ProtocolId, TransportMode,
};
use qimen_transport_qqbot::{
    GatewayStep, QqBotGatewayClient, QqBotGatewaySession, QqBotOpenApiClient, QqBotOpenApiConfig,
    SendMessagePayload, UploadFilePayload,
};
use qimen_transport_ws::{
    OneBot11ForwardWsClient, OneBot11ReverseWsConnection, ReconnectPolicy, WsReverseConfig,
    WsReverseServer,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

struct OneBotRuntimeContext<'a> {
    runtime: &'a Runtime,
    bot: &'a BotRuntimeInfo,
    adapter: &'a OneBot11Adapter,
    client: &'a OneBot11WsClient,
}

enum OneBot11WsClient {
    Forward(OneBot11ForwardWsClient),
    Reverse(OneBot11ReverseWsConnection),
}

impl OneBot11WsClient {
    async fn next_event(&mut self) -> Option<String> {
        match self {
            Self::Forward(client) => client.next_event().await,
            Self::Reverse(client) => client.next_event().await,
        }
    }

    async fn send_text_await_echo(
        &self,
        text: &str,
        echo: &str,
        timeout: Duration,
    ) -> Result<String> {
        match self {
            Self::Forward(client) => client.send_text_await_echo(text, echo, timeout).await,
            Self::Reverse(client) => client.send_text_await_echo(text, echo, timeout).await,
        }
    }
}

struct QqOfficialRuntimeContext<'a> {
    runtime: &'a Runtime,
    bot: &'a BotRuntimeInfo,
    adapter: &'a QqBotAdapter,
    client: &'a QqBotOpenApiClient,
}

#[async_trait::async_trait]
trait NormalizedActionExecutor: RuntimeBotContext {
    fn build_reply_action(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionRequest>;

    async fn reply_to_event(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionResponse>;

    async fn process_dynamic_sends(&self, sends: Vec<SendAction>) -> Result<()>;

    fn dedup_key(&self, event: &NormalizedEvent, message_id: &str) -> String;
}

#[async_trait::async_trait]
impl RuntimeBotContext for OneBotRuntimeContext<'_> {
    fn bot_instance(&self) -> &str {
        &self.bot.id
    }

    fn protocol(&self) -> ProtocolId {
        self.bot.protocol.clone()
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.bot.capabilities
    }

    async fn send_action(
        &self,
        req: NormalizedActionRequest,
    ) -> Result<qimen_protocol_core::NormalizedActionResponse> {
        self.runtime
            .execute_action(self.bot, self.adapter, self.client, req)
            .await
    }

    async fn reply(
        &self,
        event: &qimen_protocol_core::NormalizedEvent,
        message: Message,
    ) -> Result<qimen_protocol_core::NormalizedActionResponse> {
        let action = build_send_msg_action(self.bot, &event.raw_json, message)?;
        self.runtime
            .execute_action(self.bot, self.adapter, self.client, action)
            .await
    }

    fn spawn_owned(&self, name: &str, fut: OwnedTaskFuture) -> TaskHandle {
        tokio::spawn(fut);
        TaskHandle {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl NormalizedActionExecutor for OneBotRuntimeContext<'_> {
    fn build_reply_action(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionRequest> {
        build_send_msg_action(self.bot, &event.raw_json, reply)
    }

    async fn reply_to_event(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionResponse> {
        let action = self.build_reply_action(event, reply)?;
        self.runtime
            .execute_action(self.bot, self.adapter, self.client, action)
            .await
    }

    async fn process_dynamic_sends(&self, sends: Vec<SendAction>) -> Result<()> {
        self.runtime
            .process_send_actions(self.bot, self.adapter, self.client, sends)
            .await
    }

    fn dedup_key(&self, event: &NormalizedEvent, message_id: &str) -> String {
        format!("{}:{}", event.bot_instance, message_id)
    }
}

#[derive(Debug, Clone)]
pub struct BotRuntimeInfo {
    pub id: String,
    pub protocol: ProtocolId,
    pub transport: TransportMode,
    pub capabilities: CapabilitySet,
    pub endpoint: Option<String>,
    pub bind: Option<String>,
    pub path: Option<String>,
    pub access_token: Option<String>,
    pub appid: Option<String>,
    pub secret: Option<String>,
    pub intents: Vec<String>,
    pub sandbox: bool,
    pub enabled: bool,
    pub owners: Vec<String>,
    pub admins: Vec<String>,
    pub auto_approve_friend_requests: bool,
    pub auto_approve_group_invites: bool,
    pub auto_approve_friend_request_user_whitelist: Vec<String>,
    pub auto_approve_friend_request_user_blacklist: Vec<String>,
    pub auto_approve_friend_request_comment_keywords: Vec<String>,
    pub auto_reject_friend_request_comment_keywords: Vec<String>,
    pub auto_approve_friend_request_remark: Option<String>,
    pub auto_approve_group_invite_user_whitelist: Vec<String>,
    pub auto_approve_group_invite_user_blacklist: Vec<String>,
    pub auto_approve_group_invite_group_whitelist: Vec<String>,
    pub auto_approve_group_invite_group_blacklist: Vec<String>,
    pub auto_approve_group_invite_comment_keywords: Vec<String>,
    pub auto_reject_group_invite_comment_keywords: Vec<String>,
    pub auto_reject_group_invite_reason: Option<String>,
    pub auto_reply_poke_enabled: bool,
    pub auto_reply_poke_message: Option<String>,
    pub limiter_config: RateLimiterConfig,
}

/// Shared buffer for send actions queued by dynamic interceptors.
/// Drained by the main event loop after each interceptor chain call.
type PendingSendBuffer = Arc<std::sync::Mutex<Vec<SendAction>>>;

/// Adapter that wraps a dynamic plugin interceptor as a [`MessageEventInterceptor`].
struct DynamicInterceptorAdapter {
    descriptor: DynamicInterceptorDescriptor,
    dynamic_runtime: Arc<std::sync::Mutex<DynamicPluginRuntime>>,
    /// Shared buffer — same instance held by [`Runtime`] for draining.
    pending_sends: PendingSendBuffer,
    /// Timeout for FFI calls.
    timeout: Duration,
}

impl DynamicInterceptorAdapter {
    fn build_request(
        bot_id: &str,
        event: &qimen_protocol_core::NormalizedEvent,
    ) -> abi_stable_host_api::InterceptorRequest {
        use abi_stable::std_types::RString;

        let sender_id = event.sender_id().unwrap_or("").to_string();
        let group_id = event.group_id().unwrap_or("").to_string();
        let message_text = event
            .message
            .as_ref()
            .map(|m| m.plain_text())
            .unwrap_or_default();
        let raw_event_json = event.raw_json.to_string();
        let sender_nickname = event.sender_nickname().unwrap_or("").to_string();
        let message_id = event
            .message_id()
            .map(|id| id.to_string())
            .unwrap_or_default();
        let timestamp = event.time.unwrap_or(0);

        abi_stable_host_api::InterceptorRequest {
            bot_id: RString::from(bot_id),
            sender_id: RString::from(sender_id),
            group_id: RString::from(group_id),
            message_text: RString::from(message_text),
            raw_event_json: RString::from(raw_event_json),
            sender_nickname: RString::from(sender_nickname),
            message_id: RString::from(message_id),
            timestamp,
        }
    }
}

#[async_trait::async_trait]
impl qimen_plugin_api::MessageEventInterceptor for DynamicInterceptorAdapter {
    async fn pre_handle(&self, bot_id: &str, event: &qimen_protocol_core::NormalizedEvent) -> bool {
        if self.descriptor.pre_handle_symbol.is_empty() {
            return true;
        }
        let request = Self::build_request(bot_id, event);
        let descriptor = self.descriptor.clone();
        let runtime = Arc::clone(&self.dynamic_runtime);

        // Phase 1: briefly hold outer lock to get per-library handle
        let lib_path = descriptor.library_path.clone();
        let lib_handle = {
            let mut rt = match runtime.lock() {
                Ok(rt) => rt,
                Err(_) => {
                    tracing::warn!(plugin = %self.descriptor.plugin_id, "dynamic runtime lock poisoned, allowing");
                    return true;
                }
            };
            match rt.get_library(&lib_path) {
                Ok(handle) => handle,
                Err(e) => {
                    tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "failed to get library handle, allowing");
                    return true;
                }
            }
        }; // outer lock released

        // Phase 2: spawn_blocking + timeout
        let timeout_dur = self.timeout;
        let runtime_for_timeout = Arc::clone(&self.dynamic_runtime);
        let result = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || {
                DynamicPluginRuntime::execute_pre_handle_on_handle(
                    &lib_handle,
                    &descriptor,
                    request,
                )
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok((allow, sends)))) => {
                if !sends.is_empty()
                    && let Ok(mut pending) = self.pending_sends.lock()
                {
                    pending.extend(sends);
                }
                allow
            }
            Ok(Ok(Err(e))) => {
                tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "dynamic interceptor pre_handle failed, allowing");
                true
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "dynamic interceptor pre_handle task panicked, allowing");
                true
            }
            Err(_) => {
                tracing::warn!(plugin = %self.descriptor.plugin_id, "dynamic interceptor pre_handle timed out, allowing");
                if let Ok(mut rt) = runtime_for_timeout.lock() {
                    rt.record_timeout(&lib_path);
                }
                true
            }
        }
    }

    async fn after_completion(&self, bot_id: &str, event: &qimen_protocol_core::NormalizedEvent) {
        if self.descriptor.after_completion_symbol.is_empty() {
            return;
        }
        let request = Self::build_request(bot_id, event);
        let descriptor = self.descriptor.clone();
        let runtime = Arc::clone(&self.dynamic_runtime);

        // Phase 1: briefly hold outer lock to get per-library handle
        let lib_path = descriptor.library_path.clone();
        let lib_handle = {
            let mut rt = match runtime.lock() {
                Ok(rt) => rt,
                Err(_) => {
                    tracing::warn!(plugin = %self.descriptor.plugin_id, "dynamic runtime lock poisoned");
                    return;
                }
            };
            match rt.get_library(&lib_path) {
                Ok(handle) => handle,
                Err(e) => {
                    tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "failed to get library handle");
                    return;
                }
            }
        }; // outer lock released

        // Phase 2: spawn_blocking + timeout
        let timeout_dur = self.timeout;
        let runtime_for_timeout = Arc::clone(&self.dynamic_runtime);
        let result = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || {
                DynamicPluginRuntime::execute_after_completion_on_handle(
                    &lib_handle,
                    &descriptor,
                    request,
                )
            }),
        )
        .await;

        match result {
            Ok(Ok(Ok(sends))) => {
                if !sends.is_empty()
                    && let Ok(mut pending) = self.pending_sends.lock()
                {
                    pending.extend(sends);
                }
            }
            Ok(Ok(Err(e))) => {
                tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "dynamic interceptor after_completion failed");
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, plugin = %self.descriptor.plugin_id, "dynamic interceptor after_completion task panicked");
            }
            Err(_) => {
                tracing::warn!(plugin = %self.descriptor.plugin_id, "dynamic interceptor after_completion timed out");
                if let Ok(mut rt) = runtime_for_timeout.lock() {
                    rt.record_timeout(&lib_path);
                }
            }
        }
    }
}

pub struct Runtime {
    bots: Vec<BotRuntimeInfo>,
    command_plugins: Vec<std::sync::Arc<dyn CommandPlugin>>,
    system_plugins: Vec<std::sync::Arc<dyn SystemPlugin>>,
    host_plugin_report: std::sync::RwLock<Option<HostPluginReport>>,
    plugin_state_path: Option<String>,
    plugin_bin_dir: Option<String>,
    dynamic_runtime: Arc<std::sync::Mutex<DynamicPluginRuntime>>,
    /// Timeout for dynamic plugin FFI calls.
    dynamic_plugin_timeout: Duration,
    /// Static interceptors from compiled plugin bundles (preserved across rescan).
    static_interceptors: Vec<Arc<dyn qimen_plugin_api::MessageEventInterceptor>>,
    interceptor_chain: std::sync::RwLock<InterceptorChain>,
    rate_limiters: Vec<TokenBucketLimiter>,
    qqbot_send_backoff_until: std::sync::Mutex<HashMap<String, Instant>>,
    pub dedup: Arc<MessageDedup>,
    pub group_event_filter: Arc<GroupEventFilter>,
    pub plugin_acl: Arc<PluginAclManager>,
    /// Shared buffer for send actions queued by dynamic interceptors.
    interceptor_pending_sends: PendingSendBuffer,
}

impl Default for Runtime {
    fn default() -> Self {
        Self {
            bots: Vec::new(),
            command_plugins: Vec::new(),
            system_plugins: Vec::new(),
            host_plugin_report: std::sync::RwLock::new(None),
            plugin_state_path: None,
            plugin_bin_dir: None,
            dynamic_runtime: Arc::new(std::sync::Mutex::new(DynamicPluginRuntime::new())),
            dynamic_plugin_timeout: Duration::from_secs(30),
            static_interceptors: Vec::new(),
            interceptor_chain: std::sync::RwLock::new(InterceptorChain::new()),
            rate_limiters: Vec::new(),
            qqbot_send_backoff_until: std::sync::Mutex::new(HashMap::new()),
            dedup: Arc::new(MessageDedup::new(60, 10000)),
            group_event_filter: Arc::new(GroupEventFilter::disabled()),
            plugin_acl: Arc::new(PluginAclManager::new()),
            interceptor_pending_sends: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

impl Runtime {
    pub fn from_config(config: &AppConfig) -> Self {
        Self::from_config_with_plugins(config, PluginBundle::default())
    }

    pub fn from_config_with_plugins(config: &AppConfig, plugins: PluginBundle) -> Self {
        let bots = config
            .bots
            .iter()
            .map(|bot| BotRuntimeInfo {
                id: bot.id.clone(),
                protocol: parse_protocol(&bot.protocol),
                transport: parse_transport(&bot.transport),
                capabilities: CapabilitySet::default(),
                endpoint: bot.endpoint.clone(),
                bind: bot.bind.clone(),
                path: bot.path.clone(),
                access_token: bot.access_token.clone(),
                appid: bot.appid.clone(),
                secret: bot.secret.clone(),
                intents: bot.intents.clone(),
                sandbox: bot.sandbox,
                enabled: bot.enabled,
                owners: bot.owners.clone(),
                admins: bot.admins.clone(),
                auto_approve_friend_requests: bot.auto_approve_friend_requests,
                auto_approve_group_invites: bot.auto_approve_group_invites,
                auto_approve_friend_request_user_whitelist: bot
                    .auto_approve_friend_request_user_whitelist
                    .clone(),
                auto_approve_friend_request_user_blacklist: bot
                    .auto_approve_friend_request_user_blacklist
                    .clone(),
                auto_approve_friend_request_comment_keywords: bot
                    .auto_approve_friend_request_comment_keywords
                    .clone(),
                auto_reject_friend_request_comment_keywords: bot
                    .auto_reject_friend_request_comment_keywords
                    .clone(),
                auto_approve_friend_request_remark: normalize_optional_string(
                    bot.auto_approve_friend_request_remark.clone(),
                ),
                auto_approve_group_invite_user_whitelist: bot
                    .auto_approve_group_invite_user_whitelist
                    .clone(),
                auto_approve_group_invite_user_blacklist: bot
                    .auto_approve_group_invite_user_blacklist
                    .clone(),
                auto_approve_group_invite_group_whitelist: bot
                    .auto_approve_group_invite_group_whitelist
                    .clone(),
                auto_approve_group_invite_group_blacklist: bot
                    .auto_approve_group_invite_group_blacklist
                    .clone(),
                auto_approve_group_invite_comment_keywords: bot
                    .auto_approve_group_invite_comment_keywords
                    .clone(),
                auto_reject_group_invite_comment_keywords: bot
                    .auto_reject_group_invite_comment_keywords
                    .clone(),
                auto_reject_group_invite_reason: normalize_optional_string(
                    bot.auto_reject_group_invite_reason.clone(),
                ),
                auto_reply_poke_enabled: bot.auto_reply_poke_enabled,
                auto_reply_poke_message: normalize_optional_string(
                    bot.auto_reply_poke_message.clone(),
                ),
                limiter_config: bot.limiter.clone(),
            })
            .collect::<Vec<_>>();

        let rate_limiters = bots
            .iter()
            .map(|b| TokenBucketLimiter::new(&b.limiter_config))
            .collect();

        let static_interceptors = plugins.interceptors;
        let mut interceptor_chain = InterceptorChain::new();
        for interceptor in &static_interceptors {
            interceptor_chain.add(interceptor.clone());
        }

        Self {
            bots,
            command_plugins: plugins.command_plugins,
            system_plugins: plugins.system_plugins,
            host_plugin_report: std::sync::RwLock::new(None),
            plugin_state_path: Some(config.official_host.plugin_state_path.clone()),
            plugin_bin_dir: Some(config.official_host.plugin_bin_dir.clone()),
            dynamic_runtime: Arc::new(std::sync::Mutex::new(DynamicPluginRuntime::new())),
            dynamic_plugin_timeout: Duration::from_secs(
                config.official_host.dynamic_plugin_timeout_secs,
            ),
            static_interceptors,
            interceptor_chain: std::sync::RwLock::new(interceptor_chain),
            rate_limiters,
            qqbot_send_backoff_until: std::sync::Mutex::new(HashMap::new()),
            dedup: Arc::new(MessageDedup::new(60, 10000)),
            group_event_filter: Arc::new(GroupEventFilter::disabled()),
            plugin_acl: Arc::new(PluginAclManager::new()),
            interceptor_pending_sends: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    pub fn with_host_plugin_report(self, report: HostPluginReport) -> Self {
        self.inject_dynamic_interceptors(&report);
        *self.host_plugin_report.write().unwrap() = Some(report);
        self
    }

    /// Rebuild the interceptor chain: static interceptors + dynamic interceptors from the report.
    fn inject_dynamic_interceptors(&self, report: &HostPluginReport) {
        let mut chain = InterceptorChain::new();

        // Re-add static interceptors
        for interceptor in &self.static_interceptors {
            chain.add(interceptor.clone());
        }

        // Add dynamic interceptors
        for entry in &report.dynamic_plugins {
            for interceptor_entry in &entry.interceptors {
                if interceptor_entry.pre_handle_symbol.is_empty()
                    && interceptor_entry.after_completion_symbol.is_empty()
                {
                    continue;
                }
                let descriptor = DynamicInterceptorDescriptor {
                    plugin_id: entry.plugin_id.clone(),
                    library_path: entry.path.clone(),
                    pre_handle_symbol: interceptor_entry.pre_handle_symbol.clone(),
                    after_completion_symbol: interceptor_entry.after_completion_symbol.clone(),
                };
                tracing::info!(
                    plugin = %descriptor.plugin_id,
                    pre_handle = %descriptor.pre_handle_symbol,
                    after_completion = %descriptor.after_completion_symbol,
                    "registering dynamic interceptor"
                );
                let adapter = Arc::new(DynamicInterceptorAdapter {
                    descriptor,
                    dynamic_runtime: Arc::clone(&self.dynamic_runtime),
                    pending_sends: Arc::clone(&self.interceptor_pending_sends),
                    timeout: self.dynamic_plugin_timeout,
                });
                chain.add(adapter);
            }
        }

        *self.interceptor_chain.write().unwrap() = chain;
    }

    pub fn bots(&self) -> &[BotRuntimeInfo] {
        &self.bots
    }

    pub async fn boot(&self) -> Result<()> {
        // Call plugin init for all dynamic plugins
        {
            let report_clone = {
                let report_guard = self.host_plugin_report.read().unwrap();
                report_guard.clone()
            };
            if let Some(report) = report_clone.as_ref() {
                // Use 2x timeout for init since initialization is typically slower
                let init_timeout = self.dynamic_plugin_timeout * 2;

                for entry in &report.dynamic_plugins {
                    // Load plugin config from config/plugins/<plugin_id>.toml
                    let config_path = format!("config/plugins/{}.toml", entry.plugin_id);
                    let config_json = if let Ok(toml_str) = std::fs::read_to_string(&config_path) {
                        // Parse TOML → serde_json::Value → JSON string
                        match toml_str.parse::<toml::Value>() {
                            Ok(toml_val) => {
                                let json_val = toml_to_json(&toml_val);
                                serde_json::to_string(&json_val).unwrap_or_default()
                            }
                            Err(e) => {
                                tracing::warn!(
                                    plugin = %entry.plugin_id,
                                    path = %config_path,
                                    error = %e,
                                    "failed to parse plugin config TOML"
                                );
                                String::new()
                            }
                        }
                    } else {
                        String::new()
                    };

                    let plugin_dir = std::path::Path::new(&entry.path)
                        .parent()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Phase 1: get per-library handle
                    let lib_handle = {
                        let mut drt = self.dynamic_runtime.lock().map_err(|_| {
                            QimenError::Runtime("dynamic runtime lock poisoned".to_string())
                        })?;
                        drt.get_library(&entry.path)?
                    }; // outer lock released

                    // Phase 2: spawn_blocking + timeout for init
                    let path = entry.path.clone();
                    let plugin_id = entry.plugin_id.clone();
                    let plugin_dir_clone = plugin_dir.clone();
                    let config_json_clone = config_json.clone();

                    let init_result = tokio::time::timeout(
                        init_timeout,
                        tokio::task::spawn_blocking(move || {
                            DynamicPluginRuntime::execute_init_on_handle(
                                &lib_handle,
                                &path,
                                &plugin_id,
                                &config_json_clone,
                                &plugin_dir_clone,
                                ".",
                            )
                        }),
                    )
                    .await;

                    match init_result {
                        Ok(Ok(Ok(()))) => {
                            tracing::info!(
                                plugin = %entry.plugin_id,
                                "dynamic plugin init succeeded"
                            );
                        }
                        Ok(Ok(Err(e))) => {
                            tracing::error!(
                                plugin = %entry.plugin_id,
                                error = %e,
                                "dynamic plugin init failed"
                            );
                        }
                        Ok(Err(join_err)) => {
                            tracing::error!(
                                plugin = %entry.plugin_id,
                                error = %join_err,
                                "dynamic plugin init task panicked"
                            );
                        }
                        Err(_) => {
                            tracing::error!(
                                plugin = %entry.plugin_id,
                                timeout_secs = init_timeout.as_secs(),
                                "dynamic plugin init timed out"
                            );
                            if let Ok(mut drt) = self.dynamic_runtime.lock() {
                                drt.record_timeout(&entry.path);
                            }
                        }
                    }
                }
            }
        }

        // Spawn periodic dedup cleanup task
        {
            let dedup = Arc::clone(&self.dedup);
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                loop {
                    interval.tick().await;
                    dedup.cleanup().await;
                }
            });
        }

        let mut bot_futures = Vec::new();
        for (idx, bot) in self.bots.iter().enumerate() {
            tracing::info!(
                bot_id = %bot.id,
                protocol = ?bot.protocol,
                transport = ?bot.transport,
                "registered bot instance"
            );

            if !bot.enabled {
                tracing::info!(bot_id = %bot.id, "bot is disabled, skipping startup");
                continue;
            }

            bot_futures.push(self.run_bot(bot, &self.rate_limiters[idx]));
        }

        if bot_futures.is_empty() {
            tracing::warn!("no enabled bot transport loops were started");
            return Ok(());
        }

        futures_util::future::try_join_all(bot_futures).await?;
        Ok(())
    }

    /// 每个 Bot 使用独立 future 运行，避免长连接阻塞后续实例启动。
    async fn run_bot(&self, bot: &BotRuntimeInfo, limiter: &TokenBucketLimiter) -> Result<()> {
        if matches!(bot.protocol, ProtocolId::OneBot11)
            && matches!(bot.transport, TransportMode::WsForward)
        {
            self.run_onebot11_forward_ws(bot, limiter).await
        } else if matches!(bot.protocol, ProtocolId::OneBot11)
            && matches!(bot.transport, TransportMode::WsReverse)
        {
            self.run_onebot11_reverse_ws(bot, limiter).await
        } else if matches!(bot.protocol, ProtocolId::QqOfficial)
            && matches!(bot.transport, TransportMode::Gateway)
        {
            self.run_qqbot_gateway(bot, limiter).await
        } else {
            tracing::warn!(
                bot_id = %bot.id,
                protocol = ?bot.protocol,
                transport = ?bot.transport,
                "bot transport is not implemented, skipping startup"
            );
            Ok(())
        }
    }

    fn build_system_dispatcher(&self) -> OneBotSystemDispatcher {
        let mut system_dispatcher = OneBotSystemDispatcher::with_default_handlers();
        for plugin in &self.system_plugins {
            system_dispatcher.register_plugin(plugin.clone());
        }
        let report_guard = self.host_plugin_report.read().unwrap();
        if let Some(report) = report_guard.as_ref() {
            for entry in &report.dynamic_plugins {
                // v0.2: Register routes from multi-route entries
                for route_entry in &entry.routes {
                    for route_name in route_entry.route.split(',').map(|s| s.trim()) {
                        if route_name.is_empty() {
                            continue;
                        }
                        match route_entry.kind.as_str() {
                            "notice" => system_dispatcher.register_dynamic_notice_route(
                                entry.plugin_id.clone(),
                                route_name.to_string(),
                                route_entry.callback_symbol.clone(),
                            ),
                            "request" => system_dispatcher.register_dynamic_request_route(
                                entry.plugin_id.clone(),
                                route_name.to_string(),
                                route_entry.callback_symbol.clone(),
                            ),
                            "meta" => system_dispatcher.register_dynamic_meta_route(
                                entry.plugin_id.clone(),
                                route_name.to_string(),
                                route_entry.callback_symbol.clone(),
                            ),
                            _ => {}
                        }
                    }
                }
                // Legacy v0.1 fallback (when routes vec is empty)
                if entry.routes.is_empty() {
                    if !entry.notice_route.is_empty() {
                        system_dispatcher.register_dynamic_notice_route(
                            entry.plugin_id.clone(),
                            entry.notice_route.clone(),
                            entry.notice_callback_symbol.clone(),
                        );
                    }
                    if !entry.request_route.is_empty() {
                        system_dispatcher.register_dynamic_request_route(
                            entry.plugin_id.clone(),
                            entry.request_route.clone(),
                            entry.request_callback_symbol.clone(),
                        );
                    }
                    if !entry.meta_route.is_empty() {
                        system_dispatcher.register_dynamic_meta_route(
                            entry.plugin_id.clone(),
                            entry.meta_route.clone(),
                            entry.meta_callback_symbol.clone(),
                        );
                    }
                }
            }
        }
        system_dispatcher
    }

    fn build_command_dispatcher(&self) -> Result<CommandDispatcher> {
        let mut command_dispatcher = CommandDispatcher::with_default_handlers();
        for plugin in &self.command_plugins {
            command_dispatcher.register_plugin(plugin.clone());
        }
        let report_guard = self.host_plugin_report.read().unwrap();
        if let Some(report) = report_guard.as_ref() {
            command_dispatcher.set_dynamic_status_entries(
                report
                    .dynamic_plugins
                    .iter()
                    .map(|entry| {
                        let command_descriptions: Vec<String> = entry
                            .commands
                            .iter()
                            .map(|cmd| format!("{}: {}", cmd.name, cmd.description))
                            .collect();
                        let commands: Vec<String> =
                            entry.commands.iter().map(|cmd| cmd.name.clone()).collect();
                        command_dispatch::PluginStatusEntry {
                            id: entry.plugin_id.clone(),
                            name: entry.plugin_id.clone(),
                            version: entry.plugin_version.clone(),
                            api_version: entry.api_version.clone(),
                            command_descriptions,
                            commands,
                            dynamic: true,
                            enabled: Some(true),
                            callback_symbol: entry
                                .commands
                                .first()
                                .map(|c| c.callback_symbol.clone()),
                        }
                    })
                    .collect(),
            );
            let health = self
                .dynamic_runtime
                .lock()
                .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?
                .health_entries();
            command_dispatcher.merge_dynamic_health(&health);
            // Build command descriptors from v0.2 multi-command entries
            let mut dynamic_cmd_descriptors = Vec::new();
            for entry in &report.dynamic_plugins {
                for cmd in &entry.commands {
                    dynamic_cmd_descriptors.push(DynamicCommandDescriptor {
                        plugin_id: entry.plugin_id.clone(),
                        command_name: cmd.name.clone(),
                        command_description: cmd.description.clone(),
                        callback_symbol: cmd.callback_symbol.clone(),
                        library_path: entry.path.clone(),
                        aliases: cmd.aliases.clone(),
                        category: cmd.category.clone(),
                        required_role: cmd.required_role.clone(),
                        scope: cmd.scope.clone(),
                    });
                }
            }
            command_dispatcher.set_dynamic_command_descriptors(dynamic_cmd_descriptors);
        }
        Ok(command_dispatcher)
    }

    /// Re-scan the plugin_bin_dir, unload all cached libraries, and update the
    /// host_plugin_report with freshly discovered dynamic plugins.
    /// Returns the number of dynamic plugins found.
    fn rescan_dynamic_plugins(&self) -> Result<usize> {
        let dir = match &self.plugin_bin_dir {
            Some(d) => d.clone(),
            None => return Ok(0),
        };

        tracing::info!(dir = %dir, "rescanning dynamic plugins");

        // Unload all cached libraries so stale handles are dropped
        self.dynamic_runtime
            .lock()
            .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?
            .unload_all();

        let new_entries = dynamic_runtime::scan_dynamic_plugins(&dir)?;
        let count = new_entries.len();

        // Update the report, preserving non-dynamic fields
        let mut report_guard = self.host_plugin_report.write().unwrap();
        if let Some(report) = report_guard.as_mut() {
            report.dynamic_plugins = new_entries;
        } else {
            *report_guard = Some(HostPluginReport {
                builtin_modules: Vec::new(),
                configured_plugins: Vec::new(),
                persisted_states: std::collections::BTreeMap::new(),
                dynamic_plugins: new_entries,
            });
        }

        // Rebuild interceptor chain with updated dynamic plugins
        if let Some(report) = report_guard.as_ref() {
            // We need to drop the write lock before calling inject_dynamic_interceptors
            // since it takes its own write lock on interceptor_chain.
            let report_clone = report.clone();
            drop(report_guard);
            self.inject_dynamic_interceptors(&report_clone);
        } else {
            drop(report_guard);
        }

        tracing::info!(count = count, "dynamic plugin rescan complete");
        Ok(count)
    }

    async fn run_qqbot_gateway(
        &self,
        bot: &BotRuntimeInfo,
        limiter: &TokenBucketLimiter,
    ) -> Result<()> {
        let appid = bot
            .appid
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                QimenError::Config(format!("bot '{}' missing qq-official appid", bot.id))
            })?;
        let secret = bot
            .secret
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                QimenError::Config(format!("bot '{}' missing qq-official secret", bot.id))
            })?;
        let intents = qq_official_intents_value(&bot.intents)?;

        let mut api_config = QqBotOpenApiConfig::new(appid, secret);
        api_config.sandbox = bot.sandbox;
        let api_client = QqBotOpenApiClient::new(api_config)?;
        let adapter = QqBotAdapter;
        let mut system_dispatcher = self.build_system_dispatcher();
        let mut command_dispatcher = self.build_command_dispatcher()?;
        let mut command_help_text = render_help_text(&command_dispatcher.describe_commands());
        let reconnect_policy = ReconnectPolicy::default();
        let mut reconnect_delay = reconnect_policy.initial_delay;
        let mut session = QqBotGatewaySession {
            session_id: None,
            last_sequence: None,
            intents,
            shard_id: 0,
            shard_count: 1,
        };

        loop {
            let session_started = tokio::time::Instant::now();
            let gateway = api_client.get_gateway().await?;
            if let Some(shards) = gateway.shards {
                session.shard_count = shards.max(1);
            }
            let token = api_client.bot_authorization().await?;

            tracing::info!(
                bot_id = %bot.id,
                endpoint = %gateway.url,
                shard_id = session.shard_id,
                shard_count = session.shard_count,
                "connecting to QQ official Gateway"
            );

            let mut client =
                match QqBotGatewayClient::connect(&gateway.url, session.clone(), &token).await {
                    Ok(client) => {
                        tracing::info!(bot_id = %bot.id, "QQ official Gateway connected");
                        client
                    }
                    Err(err) => {
                        tracing::warn!(
                            bot_id = %bot.id,
                            delay_secs = reconnect_delay.as_secs(),
                            error = %err,
                            "QQ official Gateway connect failed, retrying"
                        );
                        tokio::time::sleep(reconnect_delay).await;
                        reconnect_delay = reconnect_policy.next_delay(reconnect_delay);
                        continue;
                    }
                };

            let session_result = self
                .run_qqbot_session(
                    bot,
                    &adapter,
                    &api_client,
                    &system_dispatcher,
                    &command_dispatcher,
                    &command_help_text,
                    &mut client,
                    &reconnect_policy,
                    limiter,
                )
                .await;

            session = client.session().clone();

            match session_result {
                Ok(SessionEnd::Shutdown) => break,
                Ok(SessionEnd::PluginReload { reply_action }) => {
                    tracing::info!(bot_id = %bot.id, "plugin reload triggered, rebuilding dispatchers");
                    if let Some(action) = reply_action
                        && let Err(err) = self
                            .execute_qqbot_action(bot, &adapter, &api_client, action)
                            .await
                    {
                        tracing::warn!(bot_id = %bot.id, error = %err, "failed to send reload reply");
                    }
                    system_dispatcher = self.build_system_dispatcher();
                    command_dispatcher = self.build_command_dispatcher()?;
                    command_help_text = render_help_text(&command_dispatcher.describe_commands());
                    continue;
                }
                Ok(SessionEnd::Reconnect(reason)) => {
                    tracing::warn!(bot_id = %bot.id, reason = %reason, "QQ official session ended, reconnecting");
                }
                Err(err) => {
                    tracing::warn!(bot_id = %bot.id, error = %err, "QQ official session failed, reconnecting");
                }
            }

            if session_started.elapsed() >= reconnect_policy.stable_connection_threshold {
                reconnect_delay = reconnect_policy.initial_delay;
            }

            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = reconnect_policy.next_delay(reconnect_delay);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_qqbot_session(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &QqBotAdapter,
        api_client: &QqBotOpenApiClient,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        client: &mut QqBotGatewayClient,
        reconnect_policy: &ReconnectPolicy,
        limiter: &TokenBucketLimiter,
    ) -> Result<SessionEnd> {
        let idle_timer = tokio::time::sleep(reconnect_policy.idle_timeout);
        let heartbeat_timer = tokio::time::sleep(client.heartbeat_interval());
        tokio::pin!(idle_timer);
        tokio::pin!(heartbeat_timer);

        loop {
            tokio::select! {
                ctrl = tokio::signal::ctrl_c() => {
                    ctrl.map_err(|err| QimenError::Runtime(err.to_string()))?;
                    tracing::info!(bot_id = %bot.id, "received ctrl-c, stopping QQ official bot loop");
                    return Ok(SessionEnd::Shutdown);
                }
                _ = &mut idle_timer => {
                    return Ok(SessionEnd::Reconnect("idle timeout exceeded".to_string()));
                }
                _ = &mut heartbeat_timer => {
                    if client.should_reconnect_for_missing_ack() {
                        return Ok(SessionEnd::Reconnect("heartbeat ack timeout".to_string()));
                    }
                    client.send_heartbeat().await?;
                    heartbeat_timer.as_mut().reset(tokio::time::Instant::now() + client.heartbeat_interval());
                }
                maybe_step = client.next_step() => {
                    match maybe_step? {
                        Some(GatewayStep::Dispatch(event)) => {
                            let signal = self
                                .handle_qqbot_gateway_event(
                                    bot,
                                    adapter,
                                    api_client,
                                    system_dispatcher,
                                    command_dispatcher,
                                    command_help_text,
                                    event,
                                    limiter,
                                )
                                .await?;
                            match signal {
                                SessionSignal::Heartbeat(_) | SessionSignal::EventHandled => {
                                    idle_timer.as_mut().reset(tokio::time::Instant::now() + reconnect_policy.idle_timeout);
                                }
                                SessionSignal::EndSession(end) => {
                                    return Ok(end);
                                }
                            }
                        }
                        Some(GatewayStep::HeartbeatAck | GatewayStep::RemoteHeartbeat | GatewayStep::Ready | GatewayStep::Resumed | GatewayStep::Ignored) => {
                            idle_timer.as_mut().reset(tokio::time::Instant::now() + reconnect_policy.idle_timeout);
                        }
                        Some(GatewayStep::Reconnect) => {
                            return Ok(SessionEnd::Reconnect("gateway requested reconnect".to_string()));
                        }
                        Some(GatewayStep::InvalidSession) => {
                            return Ok(SessionEnd::Reconnect("gateway invalid session".to_string()));
                        }
                        None => {
                            return Ok(SessionEnd::Reconnect("gateway event stream ended".to_string()));
                        }
                    }
                }
            }
        }
    }

    /// 监听 OneBot 反向 WebSocket，并为每次重连复用统一事件处理管线。
    async fn run_onebot11_reverse_ws(
        &self,
        bot: &BotRuntimeInfo,
        limiter: &TokenBucketLimiter,
    ) -> Result<()> {
        let bind = bot.bind.clone().ok_or_else(|| {
            QimenError::Config(format!("bot '{}' missing ws-reverse bind", bot.id))
        })?;
        let path = bot.path.clone().ok_or_else(|| {
            QimenError::Config(format!("bot '{}' missing ws-reverse path", bot.id))
        })?;

        let mut server = WsReverseServer::bind(WsReverseConfig {
            bind: bind.clone(),
            path: path.clone(),
            access_token: bot.access_token.clone(),
        })
        .await?;
        let adapter = OneBot11Adapter;
        let mut system_dispatcher = self.build_system_dispatcher();
        let mut command_dispatcher = self.build_command_dispatcher()?;
        let mut command_help_text = render_help_text(&command_dispatcher.describe_commands());
        let reconnect_policy = ReconnectPolicy::default();

        loop {
            let connection = tokio::select! {
                ctrl = tokio::signal::ctrl_c() => {
                    ctrl.map_err(|err| QimenError::Runtime(err.to_string()))?;
                    tracing::info!(bot_id = %bot.id, "received ctrl-c, stopping ws-reverse listener");
                    return Ok(());
                }
                connection = server.next_connection() => {
                    connection.ok_or_else(|| {
                        QimenError::Transport(format!(
                            "bot '{}' ws-reverse listener stopped unexpectedly",
                            bot.id
                        ))
                    })?
                }
            };

            let peer = connection.peer_addr();
            tracing::info!(bot_id = %bot.id, peer = %peer, "ws-reverse session connected");
            let mut client = OneBot11WsClient::Reverse(connection);

            loop {
                let session_result = self
                    .run_onebot11_session(
                        bot,
                        &adapter,
                        &system_dispatcher,
                        &command_dispatcher,
                        &command_help_text,
                        &mut client,
                        &reconnect_policy,
                        limiter,
                    )
                    .await;

                match session_result {
                    Ok(SessionEnd::Shutdown) => return Ok(()),
                    Ok(SessionEnd::PluginReload { reply_action }) => {
                        tracing::info!(bot_id = %bot.id, "plugin reload triggered, rebuilding dispatchers");
                        if let Some(action) = reply_action
                            && let Err(err) =
                                self.execute_action(bot, &adapter, &client, action).await
                        {
                            tracing::warn!(bot_id = %bot.id, error = %err, "failed to send reload reply");
                        }
                        system_dispatcher = self.build_system_dispatcher();
                        command_dispatcher = self.build_command_dispatcher()?;
                        command_help_text =
                            render_help_text(&command_dispatcher.describe_commands());
                    }
                    Ok(SessionEnd::Reconnect(reason)) => {
                        tracing::warn!(bot_id = %bot.id, peer = %peer, reason = %reason, "ws-reverse session ended, waiting for reconnect");
                        break;
                    }
                    Err(err) => {
                        tracing::warn!(bot_id = %bot.id, peer = %peer, error = %err, "ws-reverse session failed, waiting for reconnect");
                        break;
                    }
                }
            }
        }
    }

    async fn run_onebot11_forward_ws(
        &self,
        bot: &BotRuntimeInfo,
        limiter: &TokenBucketLimiter,
    ) -> Result<()> {
        let endpoint = bot.endpoint.clone().ok_or_else(|| {
            QimenError::Config(format!("bot '{}' missing ws-forward endpoint", bot.id))
        })?;

        tracing::info!(bot_id = %bot.id, endpoint = %endpoint, "connecting to OneBot11 forward WS");

        let adapter = OneBot11Adapter;
        let mut system_dispatcher = self.build_system_dispatcher();
        let mut command_dispatcher = self.build_command_dispatcher()?;
        let mut command_help_text = render_help_text(&command_dispatcher.describe_commands());
        let reconnect_policy = ReconnectPolicy::default();
        let mut reconnect_delay = reconnect_policy.initial_delay;

        loop {
            let session_started = tokio::time::Instant::now();

            let client = match OneBot11ForwardWsClient::connect(
                &endpoint,
                bot.access_token.as_deref(),
            )
            .await
            {
                Ok(client) => {
                    tracing::info!(bot_id = %bot.id, endpoint = %endpoint, "websocket connected");
                    client
                }
                Err(err) => {
                    tracing::warn!(
                        bot_id = %bot.id,
                        endpoint = %endpoint,
                        delay_secs = reconnect_delay.as_secs(),
                        error = %err,
                        "websocket connect failed, retrying"
                    );
                    tokio::time::sleep(reconnect_delay).await;
                    reconnect_delay = reconnect_policy.next_delay(reconnect_delay);
                    continue;
                }
            };
            let mut client = OneBot11WsClient::Forward(client);

            let session_result = self
                .run_onebot11_session(
                    bot,
                    &adapter,
                    &system_dispatcher,
                    &command_dispatcher,
                    &command_help_text,
                    &mut client,
                    &reconnect_policy,
                    limiter,
                )
                .await;

            match session_result {
                Ok(SessionEnd::Shutdown) => break,
                Ok(SessionEnd::PluginReload { reply_action }) => {
                    tracing::info!(bot_id = %bot.id, "plugin reload triggered, rebuilding dispatchers");
                    // Send the reload reply before rebuilding
                    if let Some(action) = reply_action
                        && let Err(err) = self.execute_action(bot, &adapter, &client, action).await
                    {
                        tracing::warn!(bot_id = %bot.id, error = %err, "failed to send reload reply");
                    }
                    // Rebuild dispatchers from updated host_plugin_report
                    system_dispatcher = self.build_system_dispatcher();
                    command_dispatcher = self.build_command_dispatcher()?;
                    command_help_text = render_help_text(&command_dispatcher.describe_commands());
                    // Continue the loop without reconnecting — reuse the existing connection
                    continue;
                }
                Ok(SessionEnd::Reconnect(reason)) => {
                    tracing::warn!(bot_id = %bot.id, reason = %reason, "session ended, reconnecting");
                }
                Err(err) => {
                    tracing::warn!(bot_id = %bot.id, error = %err, "session failed, reconnecting");
                }
            }

            if session_started.elapsed() >= reconnect_policy.stable_connection_threshold {
                reconnect_delay = reconnect_policy.initial_delay;
            }

            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = reconnect_policy.next_delay(reconnect_delay);
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_onebot11_session(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        client: &mut OneBot11WsClient,
        reconnect_policy: &ReconnectPolicy,
        limiter: &TokenBucketLimiter,
    ) -> Result<SessionEnd> {
        let idle_timer = tokio::time::sleep(reconnect_policy.idle_timeout);
        tokio::pin!(idle_timer);

        loop {
            tokio::select! {
                ctrl = tokio::signal::ctrl_c() => {
                    ctrl.map_err(|err| QimenError::Runtime(err.to_string()))?;
                    tracing::info!(bot_id = %bot.id, "received ctrl-c, stopping bot loop");
                    return Ok(SessionEnd::Shutdown);
                }
                _ = &mut idle_timer => {
                    return Ok(SessionEnd::Reconnect("idle timeout exceeded".to_string()));
                }
                maybe_event = client.next_event() => {
                    match maybe_event {
                        Some(text) => {
                            let signal = self
                                .handle_onebot11_payload(bot, adapter, system_dispatcher, command_dispatcher, command_help_text, client, &text, limiter)
                                .await?;

                            let next_deadline = match signal {
                                SessionSignal::Heartbeat(interval_ms) => {
                                    tokio::time::Instant::now() + heartbeat_timeout(interval_ms, reconnect_policy.idle_timeout)
                                }
                                SessionSignal::EventHandled => {
                                    tokio::time::Instant::now() + reconnect_policy.idle_timeout
                                }
                                SessionSignal::EndSession(end) => {
                                    return Ok(end);
                                }
                            };
                            idle_timer.as_mut().reset(next_deadline);
                        }
                        None => {
                            return Ok(SessionEnd::Reconnect("event stream ended".to_string()));
                        }
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::await_holding_lock)]
    async fn handle_onebot11_payload(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        client: &OneBot11WsClient,
        text: &str,
        limiter: &TokenBucketLimiter,
    ) -> Result<SessionSignal> {
        let payload: Value = serde_json::from_str(text)?;
        let runtime_ctx = OneBotRuntimeContext {
            runtime: self,
            bot,
            adapter,
            client,
        };

        if let Some(signal) = system_dispatcher
            .dispatch(Self::build_system_event_context(
                bot,
                &payload,
                &runtime_ctx,
            ))
            .await
        {
            return Ok(match signal {
                OneBotSystemDispatchSignal::Continue(route) => {
                    let report_clone = {
                        let report_guard = self.host_plugin_report.read().unwrap();
                        report_guard.clone()
                    };
                    if let Some(report) = report_clone.as_ref()
                        && let Some((signal, sends)) = self
                            .execute_dynamic_system_route(report, &route, &payload)
                            .await?
                    {
                        if !sends.is_empty() {
                            self.process_send_actions(bot, adapter, client, sends)
                                .await?;
                        }
                        self.apply_dynamic_system_signal(bot, adapter, client, &payload, signal)
                            .await?;
                    }
                    SessionSignal::EventHandled
                }
                OneBotSystemDispatchSignal::Heartbeat(interval_ms) => {
                    SessionSignal::Heartbeat(interval_ms)
                }
                OneBotSystemDispatchSignal::AutoApproveFriend { flag, remark } => {
                    self.execute_auto_approve_friend(bot, adapter, client, flag, remark)
                        .await?;
                    SessionSignal::EventHandled
                }
                OneBotSystemDispatchSignal::AutoRejectFriend { flag, reason } => {
                    self.execute_auto_reject_friend(bot, adapter, client, flag, reason)
                        .await?;
                    SessionSignal::EventHandled
                }
                OneBotSystemDispatchSignal::AutoApproveGroupInvite { flag, sub_type } => {
                    self.execute_auto_approve_group(bot, adapter, client, flag, sub_type)
                        .await?;
                    SessionSignal::EventHandled
                }
                OneBotSystemDispatchSignal::AutoRejectGroupInvite {
                    flag,
                    sub_type,
                    reason,
                } => {
                    self.execute_auto_reject_group(bot, adapter, client, flag, sub_type, reason)
                        .await?;
                    SessionSignal::EventHandled
                }
                OneBotSystemDispatchSignal::NoticeReply { message } => {
                    if let Some(action) = build_notice_reply_action(bot, &payload, message)? {
                        self.execute_action(bot, adapter, client, action).await?;
                    }
                    SessionSignal::EventHandled
                }
            });
        }

        let packet = IncomingPacket {
            protocol: ProtocolId::OneBot11,
            transport_mode: bot.transport.clone(),
            bot_instance: bot.id.clone(),
            payload,
            raw_bytes: None,
        };

        let event = adapter.decode_event(packet).await?;
        tracing::info!(
            bot_id = %bot.id,
            kind = ?event.kind,
            raw = %event.raw_json,
            "received OneBot event"
        );

        self.handle_normalized_event(
            bot,
            event,
            command_dispatcher,
            command_help_text,
            &runtime_ctx,
            limiter,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::await_holding_lock)]
    async fn handle_qqbot_gateway_event(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &QqBotAdapter,
        api_client: &QqBotOpenApiClient,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        gateway_event: qimen_transport_qqbot::GatewayEvent,
        limiter: &TokenBucketLimiter,
    ) -> Result<SessionSignal> {
        let runtime_ctx = QqOfficialRuntimeContext {
            runtime: self,
            bot,
            adapter,
            client: api_client,
        };
        let payload = serde_json::to_value(&gateway_event)?;
        let packet = IncomingPacket {
            protocol: ProtocolId::QqOfficial,
            transport_mode: TransportMode::Gateway,
            bot_instance: bot.id.clone(),
            payload,
            raw_bytes: None,
        };

        let event = adapter.decode_event(packet).await?;
        tracing::info!(
            bot_id = %bot.id,
            kind = ?event.kind,
            event_type = ?event.extensions.get("event_type"),
            "received QQ official event"
        );

        if !matches!(event.kind, EventKind::Message) {
            self.handle_qqbot_system_event(bot, system_dispatcher, &runtime_ctx, event)
                .await?;
            return Ok(SessionSignal::EventHandled);
        }

        self.handle_normalized_event(
            bot,
            event,
            command_dispatcher,
            command_help_text,
            &runtime_ctx,
            limiter,
        )
        .await
    }

    async fn handle_qqbot_system_event(
        &self,
        bot: &BotRuntimeInfo,
        system_dispatcher: &OneBotSystemDispatcher,
        runtime_ctx: &QqOfficialRuntimeContext<'_>,
        event: NormalizedEvent,
    ) -> Result<()> {
        if let Some(signal) = system_dispatcher
            .dispatch(Self::build_system_event_context(
                bot,
                &event.raw_json,
                runtime_ctx,
            ))
            .await
        {
            match signal {
                OneBotSystemDispatchSignal::Continue(route) => {
                    let report_clone = {
                        let report_guard = self.host_plugin_report.read().unwrap();
                        report_guard.clone()
                    };
                    if let Some(report) = report_clone.as_ref()
                        && let Some((signal, sends)) = self
                            .execute_dynamic_system_route(report, &route, &event.raw_json)
                            .await?
                    {
                        if !sends.is_empty() {
                            runtime_ctx.process_dynamic_sends(sends).await?;
                        }
                        self.apply_qqbot_dynamic_system_signal(
                            bot,
                            runtime_ctx.adapter,
                            runtime_ctx.client,
                            &event,
                            signal,
                        )
                        .await?;
                    }
                }
                OneBotSystemDispatchSignal::NoticeReply { message } => {
                    if let Some(action) = build_qqbot_notice_reply_action(
                        bot,
                        &event,
                        Message::text(message),
                        "qqbot-system-notice-reply",
                    ) {
                        self.execute_qqbot_action(
                            bot,
                            runtime_ctx.adapter,
                            runtime_ctx.client,
                            action,
                        )
                        .await?;
                    }
                }
                OneBotSystemDispatchSignal::Heartbeat(_)
                | OneBotSystemDispatchSignal::AutoApproveFriend { .. }
                | OneBotSystemDispatchSignal::AutoRejectFriend { .. }
                | OneBotSystemDispatchSignal::AutoApproveGroupInvite { .. }
                | OneBotSystemDispatchSignal::AutoRejectGroupInvite { .. } => {
                    tracing::debug!(
                        bot_id = %bot.id,
                        kind = ?event.kind,
                        "QQ official system signal has no automatic action mapping"
                    );
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    async fn handle_normalized_event(
        &self,
        bot: &BotRuntimeInfo,
        event: NormalizedEvent,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        runtime_ctx: &dyn NormalizedActionExecutor,
        limiter: &TokenBucketLimiter,
    ) -> Result<SessionSignal> {
        if let Some(message_id) = event.message_id_str() {
            let dedup_key = runtime_ctx.dedup_key(&event, &message_id);
            if !self.dedup.check_and_mark(&dedup_key).await {
                tracing::debug!(bot_id = %bot.id, message_id = %message_id, "duplicate message skipped by dedup");
                return Ok(SessionSignal::EventHandled);
            }
        }

        let group_id = event.group_id_i64();
        if !self.group_event_filter.should_process(group_id).await {
            tracing::debug!(bot_id = %bot.id, group_id = ?group_id, "event filtered by group event filter");
            return Ok(SessionSignal::EventHandled);
        }

        let Some(message) = event.message.as_ref() else {
            return Ok(SessionSignal::EventHandled);
        };

        if message.plain_text().trim().is_empty() {
            return Ok(SessionSignal::EventHandled);
        }

        if !limiter.try_acquire() {
            tracing::debug!(bot_id = %bot.id, "rate limiter dropped message");
            return Ok(SessionSignal::EventHandled);
        }

        if !self
            .interceptor_chain
            .read()
            .unwrap()
            .pre_handle(&bot.id, &event)
            .await
        {
            let sends = self.drain_interceptor_sends();
            if !sends.is_empty() {
                runtime_ctx.process_dynamic_sends(sends).await?;
            }
            tracing::debug!(bot_id = %bot.id, "interceptor chain blocked event");
            return Ok(SessionSignal::EventHandled);
        }

        {
            let sends = self.drain_interceptor_sends();
            if !sends.is_empty() {
                runtime_ctx.process_dynamic_sends(sends).await?;
            }
        }

        let permission = PermissionResolver::resolve(bot, &event);
        let command_result = command_dispatcher
            .dispatch(&bot.id, &event, runtime_ctx)
            .with_roles(permission.is_admin, permission.is_owner)
            .with_plugin_acl(&self.plugin_acl)
            .execute()
            .await;

        let outcome = self
            .handle_command_signal(
                &event,
                command_dispatcher,
                command_help_text,
                command_result,
                runtime_ctx,
            )
            .await?;
        let reply = match outcome {
            NormalizedCommandOutcome::Reply(reply) => Some(reply),
            NormalizedCommandOutcome::Reload { reply_action } => {
                return Ok(SessionSignal::EndSession(SessionEnd::PluginReload {
                    reply_action,
                }));
            }
            NormalizedCommandOutcome::None => None,
        };

        let reply = match event.message.as_ref().map(|message| message.plain_text()) {
            Some(text)
                if text.trim().eq_ignore_ascii_case("help")
                    || text.trim().eq_ignore_ascii_case("/help") =>
            {
                Some(Message::text(command_help_text.to_string()))
            }
            _ => reply,
        };

        if let Some(reply) = reply {
            runtime_ctx.reply_to_event(&event, reply).await?;
        }

        self.interceptor_chain
            .read()
            .unwrap()
            .after_completion(&bot.id, &event)
            .await;

        {
            let sends = self.drain_interceptor_sends();
            if !sends.is_empty() {
                runtime_ctx.process_dynamic_sends(sends).await?;
            }
        }

        Ok(SessionSignal::EventHandled)
    }

    async fn handle_command_signal(
        &self,
        event: &NormalizedEvent,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        command_result: Option<CommandDispatchSignal>,
        runtime_ctx: &dyn NormalizedActionExecutor,
    ) -> Result<NormalizedCommandOutcome> {
        Ok(match command_result {
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsShow)) => {
                let health = self
                    .dynamic_runtime
                    .lock()
                    .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?
                    .health_entries();
                let mut snapshot = CommandDispatcher::with_default_handlers();
                snapshot.set_dynamic_status_entries(command_dispatcher.plugin_status_entries());
                snapshot.merge_dynamic_health(&health);
                NormalizedCommandOutcome::Reply(Message::text(render_plugin_status_text(&snapshot)))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryReport)) => {
                NormalizedCommandOutcome::Reply(Message::text(render_registry_report(
                    command_dispatcher,
                )))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryConflicts)) => {
                NormalizedCommandOutcome::Reply(Message::text(render_registry_conflicts_report(
                    command_dispatcher,
                )))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrors)) => {
                let health = self
                    .dynamic_runtime
                    .lock()
                    .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?
                    .health_entries();
                NormalizedCommandOutcome::Reply(Message::text(render_dynamic_errors_report(
                    &health,
                )))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrorsClear)) => {
                self.dynamic_runtime
                    .lock()
                    .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?
                    .clear_errors();
                NormalizedCommandOutcome::Reply(Message::text(
                    "dynamic runtime error state cleared",
                ))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsEnable {
                plugin_id,
            })) => NormalizedCommandOutcome::Reply(Message::text(self.update_plugin_state(
                plugin_id,
                true,
                command_dispatcher,
            )?)),
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsDisable {
                plugin_id,
            })) => NormalizedCommandOutcome::Reply(Message::text(self.update_plugin_state(
                plugin_id,
                false,
                command_dispatcher,
            )?)),
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsReload)) => {
                match self.rescan_dynamic_plugins() {
                    Ok(count) => {
                        let reply_msg = Message::text(format!(
                            "dynamic plugins reloaded: {} plugin(s) discovered, dispatchers will be rebuilt",
                            count
                        ));
                        let reply_action = runtime_ctx.build_reply_action(event, reply_msg).ok();
                        NormalizedCommandOutcome::Reload { reply_action }
                    }
                    Err(err) => NormalizedCommandOutcome::Reply(Message::text(format!(
                        "plugin reload failed: {err}"
                    ))),
                }
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::Help)) => {
                NormalizedCommandOutcome::Reply(Message::text(command_help_text.to_string()))
            }
            Some(CommandDispatchSignal::DynamicCommand { descriptor, args }) => {
                match self
                    .handle_dynamic_command(event, descriptor, args, runtime_ctx)
                    .await?
                {
                    Some(reply) => NormalizedCommandOutcome::Reply(reply),
                    None => NormalizedCommandOutcome::None,
                }
            }
            Some(CommandDispatchSignal::Reply(reply)) => NormalizedCommandOutcome::Reply(reply),
            None => NormalizedCommandOutcome::None,
        })
    }

    async fn handle_dynamic_command(
        &self,
        event: &NormalizedEvent,
        descriptor: DynamicCommandDescriptor,
        args: Vec<String>,
        runtime_ctx: &dyn NormalizedActionExecutor,
    ) -> Result<Option<Message>> {
        let acl_user_id = event.user_id();
        let acl_group_id = event.group_id_i64();
        if !self
            .plugin_acl
            .should_process(&descriptor.plugin_id, acl_user_id, acl_group_id)
            .await
        {
            tracing::debug!(plugin_id = %descriptor.plugin_id, "event blocked by plugin ACL");
            return Ok(None);
        }

        let lib_path = descriptor.library_path.clone();
        let lib_handle = {
            let mut runtime = self
                .dynamic_runtime
                .lock()
                .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?;
            runtime.get_library(&lib_path)?
        };

        let sender_id = event.sender_id().unwrap_or_default().to_string();
        let group_id_str = event.group_id().unwrap_or_default().to_string();
        let raw_json = event.raw_json.to_string();
        let sender_nickname = event.sender_nickname().unwrap_or_default().to_string();
        let message_id_str = event.message_id_str().unwrap_or_default();
        let timestamp = event.time.unwrap_or(0);
        let timeout_dur = self.dynamic_plugin_timeout;
        let descriptor_clone = descriptor.clone();

        let ffi_result = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || {
                DynamicPluginRuntime::execute_command_on_handle(
                    &lib_handle,
                    &descriptor_clone,
                    &args,
                    &sender_id,
                    &group_id_str,
                    &raw_json,
                    &sender_nickname,
                    &message_id_str,
                    timestamp,
                )
            }),
        )
        .await;

        let (dyn_response, sends) = match ffi_result {
            Ok(Ok(inner)) => inner?,
            Ok(Err(join_err)) => {
                return Err(QimenError::Runtime(format!(
                    "dynamic plugin spawn_blocking panicked: {join_err}"
                )));
            }
            Err(_) => {
                if let Ok(mut runtime) = self.dynamic_runtime.lock() {
                    runtime.record_timeout(&lib_path);
                }
                return Err(QimenError::Runtime(format!(
                    "dynamic plugin command timed out after {}s for '{}'",
                    timeout_dur.as_secs(),
                    descriptor.plugin_id
                )));
            }
        };

        if !sends.is_empty() {
            runtime_ctx.process_dynamic_sends(sends).await?;
        }

        Ok(match dyn_response {
            DynamicResponse::ReplyMessage(message) => Some(message),
            DynamicResponse::Reply(message) => Some(Message::text(message)),
            DynamicResponse::Ignore => None,
            DynamicResponse::Approve(reason) => Some(Message::text(
                reason.unwrap_or_else(|| "dynamic command approved".to_string()),
            )),
            DynamicResponse::Reject(reason) => Some(Message::text(
                reason.unwrap_or_else(|| "dynamic command rejected".to_string()),
            )),
        })
    }

    async fn execute_auto_approve_friend(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        flag: String,
        remark: Option<String>,
    ) -> Result<()> {
        let action = NormalizedActionRequest {
            protocol: ProtocolId::OneBot11,
            bot_instance: bot.id.clone(),
            action: "set_friend_add_request".to_string(),
            params: serde_json::json!({
                "flag": flag,
                "approve": true,
                "remark": remark,
            }),
            echo: Some(json!(build_echo(bot))),
            timeout_ms: 5000,
            metadata: ActionMeta {
                source: "auto-approve-friend-request".to_string(),
            },
        };

        self.execute_action(bot, adapter, client, action).await?;
        Ok(())
    }

    async fn execute_auto_reject_friend(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        flag: String,
        reason: Option<String>,
    ) -> Result<()> {
        let action = NormalizedActionRequest {
            protocol: ProtocolId::OneBot11,
            bot_instance: bot.id.clone(),
            action: "set_friend_add_request".to_string(),
            params: serde_json::json!({
                "flag": flag,
                "approve": false,
                "reason": reason,
            }),
            echo: Some(json!(build_echo(bot))),
            timeout_ms: 5000,
            metadata: ActionMeta {
                source: "auto-reject-friend-request".to_string(),
            },
        };

        self.execute_action(bot, adapter, client, action).await?;
        Ok(())
    }

    async fn execute_auto_approve_group(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        flag: String,
        sub_type: String,
    ) -> Result<()> {
        let action = NormalizedActionRequest {
            protocol: ProtocolId::OneBot11,
            bot_instance: bot.id.clone(),
            action: "set_group_add_request".to_string(),
            params: serde_json::json!({
                "flag": flag,
                "sub_type": sub_type,
                "approve": true,
            }),
            echo: Some(json!(build_echo(bot))),
            timeout_ms: 5000,
            metadata: ActionMeta {
                source: "auto-approve-group-request".to_string(),
            },
        };

        self.execute_action(bot, adapter, client, action).await?;
        Ok(())
    }

    async fn execute_auto_reject_group(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        flag: String,
        sub_type: String,
        reason: Option<String>,
    ) -> Result<()> {
        let action = NormalizedActionRequest {
            protocol: ProtocolId::OneBot11,
            bot_instance: bot.id.clone(),
            action: "set_group_add_request".to_string(),
            params: serde_json::json!({
                "flag": flag,
                "sub_type": sub_type,
                "approve": false,
                "reason": reason,
            }),
            echo: Some(json!(build_echo(bot))),
            timeout_ms: 5000,
            metadata: ActionMeta {
                source: "auto-reject-group-request".to_string(),
            },
        };

        self.execute_action(bot, adapter, client, action).await?;
        Ok(())
    }

    async fn execute_action(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        action: NormalizedActionRequest,
    ) -> Result<qimen_protocol_core::NormalizedActionResponse> {
        let echo = action
            .echo
            .as_ref()
            .and_then(Value::as_str)
            .ok_or_else(|| QimenError::Runtime("action echo missing or not string".to_string()))?
            .to_string();
        let packet = adapter.encode_action(&action).await?;
        let serialized = serde_json::to_string(&packet.payload)?;
        let raw_response = client
            .send_text_await_echo(&serialized, &echo, Duration::from_secs(5))
            .await?;

        let response_packet = IncomingPacket {
            protocol: ProtocolId::OneBot11,
            transport_mode: bot.transport.clone(),
            bot_instance: bot.id.clone(),
            payload: serde_json::from_str(&raw_response)?,
            raw_bytes: None,
        };

        let response = adapter.decode_action_response(response_packet).await?;
        tracing::info!(
            bot_id = %bot.id,
            action = %action.action,
            retcode = response.retcode,
            status = ?response.status,
            echo = ?response.echo,
            "awaited OneBot action response"
        );

        Ok(response)
    }

    async fn execute_qqbot_action(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &QqBotAdapter,
        client: &QqBotOpenApiClient,
        action: NormalizedActionRequest,
    ) -> Result<NormalizedActionResponse> {
        let packet = match adapter.encode_action(&action).await {
            Ok(packet) => packet,
            Err(err) => {
                let error = err.to_string();
                tracing::warn!(
                    bot_id = %bot.id,
                    action = %action.action,
                    error = %error,
                    "failed to encode QQ official action"
                );
                return Ok(qqbot_failed_action_response(
                    bot, &action, None, "Protocol", &error, None,
                ));
            }
        };
        let route = packet
            .payload
            .get("route")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                QimenError::Protocol("qqbot action payload missing route".to_string())
            })?;
        if let Some(remaining) = self.qqbot_send_backoff_remaining(bot, route) {
            let retry_after_ms = remaining.as_millis().min(u128::from(u64::MAX)) as u64;
            tracing::warn!(
                bot_id = %bot.id,
                action = %action.action,
                route,
                retry_after_ms,
                "QQ official action skipped because route is in backoff"
            );
            return Ok(qqbot_failed_action_response(
                bot,
                &action,
                Some(route),
                "RateLimited",
                "QQ official route is temporarily rate limited",
                Some(retry_after_ms),
            ));
        }
        if let Some(segments) = packet
            .payload
            .get("unsupported_segments")
            .and_then(Value::as_array)
            && !segments.is_empty()
        {
            tracing::warn!(
                bot_id = %bot.id,
                action = %action.action,
                route,
                unsupported_segments = ?segments,
                "QQ official action degraded unsupported message segments"
            );
        }
        if route == "channel_recall_message" {
            let channel_id = packet
                .payload
                .get("channel_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot channel recall missing channel_id".to_string())
                })?;
            let message_id = packet
                .payload
                .get("message_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot channel recall missing message_id".to_string())
                })?;
            let hidetip = packet
                .payload
                .get("hidetip")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            return Ok(self
                .complete_qqbot_action(bot, &action, route, async {
                    client
                        .recall_channel_message(&channel_id, &message_id, hidetip)
                        .await
                })
                .await);
        }
        if matches!(route, "group_file" | "c2c_file") {
            return Ok(self
                .complete_qqbot_action(bot, &action, route, async {
                    let payload = UploadFilePayload {
                        file_type: packet
                            .payload
                            .get("file_type")
                            .and_then(Value::as_i64)
                            .unwrap_or(1),
                        url: packet
                            .payload
                            .get("url")
                            .and_then(value_to_optional_string)
                            .ok_or_else(|| {
                                QimenError::Protocol("qqbot upload media missing url".to_string())
                            })?,
                        srv_send_msg: packet
                            .payload
                            .get("srv_send_msg")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    };
                    match route {
                        "group_file" => {
                            let group_openid = packet
                                .payload
                                .get("group_openid")
                                .and_then(value_to_optional_string)
                                .ok_or_else(|| {
                                    QimenError::Protocol(
                                        "qqbot group_file missing group_openid".to_string(),
                                    )
                                })?;
                            client.post_group_file(&group_openid, &payload).await
                        }
                        "c2c_file" => {
                            let openid = packet
                                .payload
                                .get("openid")
                                .and_then(value_to_optional_string)
                                .ok_or_else(|| {
                                    QimenError::Protocol(
                                        "qqbot c2c_file missing openid".to_string(),
                                    )
                                })?;
                            client.post_c2c_file(&openid, &payload).await
                        }
                        _ => unreachable!(),
                    }
                })
                .await);
        }
        Ok(self
            .complete_qqbot_action(bot, &action, route, async {
                let media_upload = packet.payload.get("media_upload").cloned();
                let mut payload = SendMessagePayload {
                    msg_type: packet.payload.get("msg_type").and_then(Value::as_i64),
                    content: packet
                        .payload
                        .get("content")
                        .and_then(value_to_optional_string),
                    msg_id: packet
                        .payload
                        .get("msg_id")
                        .and_then(value_to_optional_string),
                    msg_seq: packet.payload.get("msg_seq").and_then(Value::as_i64),
                    event_id: packet
                        .payload
                        .get("event_id")
                        .and_then(value_to_optional_string),
                    markdown: packet.payload.get("markdown").cloned(),
                    keyboard: packet.payload.get("keyboard").cloned(),
                    ark: packet.payload.get("ark").cloned(),
                    embed: packet.payload.get("embed").cloned(),
                    media: packet.payload.get("media").cloned(),
                    image: packet
                        .payload
                        .get("image")
                        .and_then(value_to_optional_string),
                };

                match route {
                    "channel_message" => {
                        let channel_id = packet
                            .payload
                            .get("channel_id")
                            .and_then(value_to_optional_string)
                            .ok_or_else(|| {
                                QimenError::Protocol(
                                    "qqbot channel_message missing channel_id".to_string(),
                                )
                            })?;
                        client.post_channel_message(&channel_id, &payload).await
                    }
                    "group_message" => {
                        let group_openid = packet
                            .payload
                            .get("group_openid")
                            .and_then(value_to_optional_string)
                            .ok_or_else(|| {
                                QimenError::Protocol(
                                    "qqbot group_message missing group_openid".to_string(),
                                )
                            })?;
                        if let Some(upload) = media_upload.as_ref()
                            && payload.media.is_none()
                        {
                            payload.media = Some(
                                upload_qqbot_media(client, "group_file", &group_openid, upload)
                                    .await?,
                            );
                            payload.msg_type = Some(7);
                            payload.image = None;
                        }
                        client.post_group_message(&group_openid, &payload).await
                    }
                    "c2c_message" => {
                        let openid = packet
                            .payload
                            .get("openid")
                            .and_then(value_to_optional_string)
                            .ok_or_else(|| {
                                QimenError::Protocol("qqbot c2c_message missing openid".to_string())
                            })?;
                        if let Some(upload) = media_upload.as_ref()
                            && payload.media.is_none()
                        {
                            payload.media = Some(
                                upload_qqbot_media(client, "c2c_file", &openid, upload).await?,
                            );
                            payload.msg_type = Some(7);
                            payload.image = None;
                        }
                        client.post_c2c_message(&openid, &payload).await
                    }
                    "dms_message" => {
                        let guild_id = packet
                            .payload
                            .get("guild_id")
                            .and_then(value_to_optional_string)
                            .ok_or_else(|| {
                                QimenError::Protocol(
                                    "qqbot dms_message missing guild_id".to_string(),
                                )
                            })?;
                        client.post_dms_message(&guild_id, &payload).await
                    }
                    other => Err(QimenError::Protocol(format!(
                        "unsupported qqbot action route '{other}'"
                    ))),
                }
            })
            .await)
    }

    async fn complete_qqbot_action<F>(
        &self,
        bot: &BotRuntimeInfo,
        action: &NormalizedActionRequest,
        route: &str,
        api_call: F,
    ) -> NormalizedActionResponse
    where
        F: Future<Output = Result<Value>>,
    {
        match api_call.await {
            Ok(data) => {
                self.clear_qqbot_send_backoff(bot, route);
                tracing::info!(
                    bot_id = %bot.id,
                    action = %action.action,
                    route = %route,
                    "executed QQ official action"
                );
                qqbot_ok_action_response(bot, action, route, data)
            }
            Err(err) => {
                let error = err.to_string();
                let category = qqbot_error_category(&error);
                let retry_after_ms = qqbot_error_retry_after_ms(&error);
                self.record_qqbot_send_failure(bot, route, category, retry_after_ms);
                tracing::warn!(
                    bot_id = %bot.id,
                    action = %action.action,
                    route = %route,
                    category,
                    retry_after_ms = ?retry_after_ms,
                    error = %error,
                    "QQ official action failed"
                );
                qqbot_failed_action_response(
                    bot,
                    action,
                    Some(route),
                    category,
                    &error,
                    retry_after_ms,
                )
            }
        }
    }

    fn qqbot_send_backoff_remaining(&self, bot: &BotRuntimeInfo, route: &str) -> Option<Duration> {
        let key = qqbot_backoff_key(bot, route);
        let now = Instant::now();
        let mut guard = self.qqbot_send_backoff_until.lock().ok()?;
        match guard.get(&key).copied() {
            Some(until) if until > now => Some(until.duration_since(now)),
            Some(_) => {
                guard.remove(&key);
                None
            }
            None => None,
        }
    }

    fn record_qqbot_send_failure(
        &self,
        bot: &BotRuntimeInfo,
        route: &str,
        category: &str,
        retry_after_ms: Option<u64>,
    ) {
        if category != "RateLimited" {
            return;
        }
        let delay_ms = retry_after_ms.unwrap_or(1_000).clamp(1_000, 60_000);
        if let Ok(mut guard) = self.qqbot_send_backoff_until.lock() {
            guard.insert(
                qqbot_backoff_key(bot, route),
                Instant::now() + Duration::from_millis(delay_ms),
            );
        }
    }

    fn clear_qqbot_send_backoff(&self, bot: &BotRuntimeInfo, route: &str) {
        if let Ok(mut guard) = self.qqbot_send_backoff_until.lock() {
            guard.remove(&qqbot_backoff_key(bot, route));
        }
    }

    /// Process outbound send actions queued by a dynamic plugin via `BotApi` / `SendBuilder`.
    async fn process_send_actions(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        sends: Vec<SendAction>,
    ) -> Result<()> {
        for send in sends {
            let msg_type = send.message_type.as_str();
            let target = send.target_id.as_str();
            let segments_json = send.segments_json.as_str().trim();

            let message_value = if !segments_json.is_empty() {
                // Rich-media: parse JSON segments
                match serde_json::from_str::<Value>(segments_json) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "invalid segments_json in SendAction, falling back to text");
                        json!([{"type": "text", "data": {"text": send.message.as_str()}}])
                    }
                }
            } else {
                json!([{"type": "text", "data": {"text": send.message.as_str()}}])
            };

            let (action_name, params) = match msg_type {
                "group" => (
                    "send_group_msg",
                    json!({ "group_id": target.parse::<i64>().unwrap_or(0), "message": message_value }),
                ),
                "private" => (
                    "send_private_msg",
                    json!({ "user_id": target.parse::<i64>().unwrap_or(0), "message": message_value }),
                ),
                other => {
                    tracing::warn!(message_type = %other, "unknown SendAction message_type, skipping");
                    continue;
                }
            };

            let action = NormalizedActionRequest {
                protocol: ProtocolId::OneBot11,
                bot_instance: bot.id.clone(),
                action: action_name.to_string(),
                params,
                echo: Some(json!(build_echo(bot))),
                timeout_ms: 5000,
                metadata: ActionMeta {
                    source: "dynamic-plugin-bot-api".to_string(),
                },
            };

            if let Err(e) = self.execute_action(bot, adapter, client, action).await {
                tracing::warn!(error = %e, target = %target, action = %action_name, "failed to execute BotApi send action");
            }
        }
        Ok(())
    }

    /// Drain pending send actions accumulated by dynamic interceptors.
    fn drain_interceptor_sends(&self) -> Vec<SendAction> {
        self.interceptor_pending_sends
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    async fn process_qqbot_send_actions(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &QqBotAdapter,
        client: &QqBotOpenApiClient,
        sends: Vec<SendAction>,
    ) -> Result<()> {
        for send in sends {
            let msg_type = send.message_type.as_str();
            let target = send.target_id.as_str();
            let segments_json = send.segments_json.as_str().trim();

            let message = if !segments_json.is_empty() {
                match serde_json::from_str::<Value>(segments_json) {
                    Ok(value) => Message::from_onebot_value(&value),
                    Err(err) => {
                        tracing::warn!(error = %err, "invalid segments_json in SendAction, falling back to text");
                        Message::text(send.message.as_str())
                    }
                }
            } else {
                Message::text(send.message.as_str())
            };

            let (action_name, params) = match msg_type {
                "group" => (
                    "send_group_msg",
                    json!({
                        "group_openid": target,
                        "message": message.to_onebot_value(),
                    }),
                ),
                "private" => (
                    "send_private_msg",
                    json!({
                        "openid": target,
                        "message": message.to_onebot_value(),
                    }),
                ),
                "channel" => (
                    "send_channel_msg",
                    json!({
                        "channel_id": target,
                        "message": message.to_onebot_value(),
                    }),
                ),
                "channel_private" | "guild_private" => (
                    "send_dms",
                    json!({
                        "guild_id": target,
                        "message": message.to_onebot_value(),
                    }),
                ),
                other => {
                    tracing::warn!(message_type = %other, "unknown QQ official SendAction message_type, skipping");
                    continue;
                }
            };

            let action = NormalizedActionRequest {
                protocol: ProtocolId::QqOfficial,
                bot_instance: bot.id.clone(),
                action: action_name.to_string(),
                params,
                echo: Some(json!(build_echo(bot))),
                timeout_ms: 5000,
                metadata: ActionMeta {
                    source: "dynamic-plugin-bot-api".to_string(),
                },
            };

            if let Err(err) = self
                .execute_qqbot_action(bot, adapter, client, action)
                .await
            {
                tracing::warn!(
                    error = %err,
                    target = %target,
                    action = %action_name,
                    "failed to execute QQ official BotApi send action"
                );
            }
        }

        Ok(())
    }

    async fn apply_dynamic_system_signal(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11WsClient,
        payload: &Value,
        signal: DynamicResponse,
    ) -> Result<()> {
        match signal {
            DynamicResponse::ReplyMessage(message) => {
                let group_id = payload.get("group_id").and_then(Value::as_i64);
                let user_id = payload.get("user_id").and_then(Value::as_i64);
                if let Some(group_id) = group_id {
                    let action = NormalizedActionRequest {
                        protocol: ProtocolId::OneBot11,
                        bot_instance: bot.id.clone(),
                        action: "send_group_msg".to_string(),
                        params: json!({
                            "group_id": group_id,
                            "message": message.to_onebot_value(),
                        }),
                        echo: Some(json!(build_echo(bot))),
                        timeout_ms: 5000,
                        metadata: ActionMeta {
                            source: "dynamic-system-reply".to_string(),
                        },
                    };
                    self.execute_action(bot, adapter, client, action).await?;
                } else if let Some(user_id) = user_id {
                    let action = NormalizedActionRequest {
                        protocol: ProtocolId::OneBot11,
                        bot_instance: bot.id.clone(),
                        action: "send_private_msg".to_string(),
                        params: json!({
                            "user_id": user_id,
                            "message": message.to_onebot_value(),
                        }),
                        echo: Some(json!(build_echo(bot))),
                        timeout_ms: 5000,
                        metadata: ActionMeta {
                            source: "dynamic-system-reply".to_string(),
                        },
                    };
                    self.execute_action(bot, adapter, client, action).await?;
                }
            }
            DynamicResponse::Reply(message) => {
                if let Some(action) = build_notice_reply_action(bot, payload, message)? {
                    self.execute_action(bot, adapter, client, action).await?;
                }
            }
            DynamicResponse::Ignore => {}
            DynamicResponse::Approve(reason) => {
                if payload.get("request_type").and_then(Value::as_str) == Some("friend") {
                    let flag = payload.get("flag").map(value_to_string).unwrap_or_default();
                    self.execute_auto_approve_friend(bot, adapter, client, flag, reason)
                        .await?;
                } else if payload.get("request_type").and_then(Value::as_str) == Some("group") {
                    let flag = payload.get("flag").map(value_to_string).unwrap_or_default();
                    let sub_type = payload
                        .get("sub_type")
                        .and_then(Value::as_str)
                        .unwrap_or("invite")
                        .to_string();
                    self.execute_auto_approve_group(bot, adapter, client, flag, sub_type)
                        .await?;
                }
            }
            DynamicResponse::Reject(reason) => {
                if payload.get("request_type").and_then(Value::as_str) == Some("friend") {
                    let flag = payload.get("flag").map(value_to_string).unwrap_or_default();
                    self.execute_auto_reject_friend(bot, adapter, client, flag, reason)
                        .await?;
                } else if payload.get("request_type").and_then(Value::as_str) == Some("group") {
                    let flag = payload.get("flag").map(value_to_string).unwrap_or_default();
                    let sub_type = payload
                        .get("sub_type")
                        .and_then(Value::as_str)
                        .unwrap_or("invite")
                        .to_string();
                    self.execute_auto_reject_group(bot, adapter, client, flag, sub_type, reason)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn apply_qqbot_dynamic_system_signal(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &QqBotAdapter,
        client: &QqBotOpenApiClient,
        event: &NormalizedEvent,
        signal: DynamicResponse,
    ) -> Result<()> {
        match signal {
            DynamicResponse::ReplyMessage(message) => {
                if let Some(action) = build_qqbot_notice_reply_action(
                    bot,
                    event,
                    message,
                    "qqbot-dynamic-system-reply",
                ) {
                    self.execute_qqbot_action(bot, adapter, client, action)
                        .await?;
                }
            }
            DynamicResponse::Reply(message) => {
                if let Some(action) = build_qqbot_notice_reply_action(
                    bot,
                    event,
                    Message::text(message),
                    "qqbot-dynamic-system-reply",
                ) {
                    self.execute_qqbot_action(bot, adapter, client, action)
                        .await?;
                }
            }
            DynamicResponse::Ignore => {}
            DynamicResponse::Approve(reason) | DynamicResponse::Reject(reason) => {
                tracing::debug!(
                    bot_id = %bot.id,
                    reason = ?reason,
                    "QQ official dynamic system approval signal has no automatic action mapping"
                );
            }
        }

        Ok(())
    }

    fn update_plugin_state(
        &self,
        plugin_id: String,
        enabled: bool,
        command_dispatcher: &CommandDispatcher,
    ) -> Result<String> {
        let Some(path) = &self.plugin_state_path else {
            return Err(QimenError::Runtime(
                "plugin state path not configured".to_string(),
            ));
        };

        let entries = command_dispatcher.plugin_status_entries();
        let known_static = entries.iter().find(|entry| entry.id == plugin_id);

        let Some(entry) = known_static else {
            return Ok(format!("plugin '{}' not found", plugin_id));
        };

        let mut state = load_plugin_state(path)?;
        let current = state.is_enabled(&plugin_id);
        if current == enabled {
            return Ok(format!(
                "plugin '{}' is already {}",
                plugin_id,
                if enabled { "enabled" } else { "disabled" }
            ));
        }

        state.set_enabled(plugin_id.clone(), enabled);
        state.save_to_path(path)?;

        if entry.dynamic {
            Ok(format!(
                "plugin '{}' {} (takes effect after restart)",
                plugin_id,
                if enabled { "enabled" } else { "disabled" },
            ))
        } else {
            Ok(format!(
                "plugin '{}' {} in {}",
                plugin_id,
                if enabled { "enabled" } else { "disabled" },
                path
            ))
        }
    }

    async fn execute_dynamic_system_route(
        &self,
        report: &HostPluginReport,
        route: &onebot11_dispatch::OneBotSystemRoute,
        payload: &Value,
    ) -> Result<Option<(DynamicResponse, Vec<SendAction>)>> {
        let raw_json = payload.to_string();

        // Determine library_path, callback_symbol, route string, and kind
        let (library_path, callback_symbol, route_str, kind) = match route {
            onebot11_dispatch::OneBotSystemRoute::Notice(notice) => {
                let route_label = match notice {
                    onebot11_dispatch::NoticeRoute::GroupPoke => "GroupPoke",
                    onebot11_dispatch::NoticeRoute::PrivatePoke => "PrivatePoke",
                    onebot11_dispatch::NoticeRoute::NotifyLuckyKing => "NotifyLuckyKing",
                    onebot11_dispatch::NoticeRoute::NotifyHonor(_) => "NotifyHonor",
                    _ => return Ok(None),
                };
                if let Some((entry, route_entry)) = find_route_entry(report, "notice", route_label)
                {
                    (
                        entry.path.clone(),
                        route_entry.callback_symbol.clone(),
                        route_label.to_string(),
                        "notice",
                    )
                } else if let Some(entry) = report
                    .dynamic_plugins
                    .iter()
                    .find(|e| e.notice_route == route_label)
                {
                    (
                        entry.path.clone(),
                        entry.notice_callback_symbol.clone(),
                        entry.notice_route.clone(),
                        "notice",
                    )
                } else {
                    return Ok(None);
                }
            }
            onebot11_dispatch::OneBotSystemRoute::Request(request) => {
                let route_label = match request {
                    onebot11_dispatch::RequestRoute::Friend => "Friend",
                    onebot11_dispatch::RequestRoute::GroupAdd => "GroupAdd",
                    onebot11_dispatch::RequestRoute::GroupInvite => "GroupInvite",
                    _ => return Ok(None),
                };
                if let Some((entry, route_entry)) = find_route_entry(report, "request", route_label)
                {
                    (
                        entry.path.clone(),
                        route_entry.callback_symbol.clone(),
                        route_label.to_string(),
                        "request",
                    )
                } else if let Some(entry) = report
                    .dynamic_plugins
                    .iter()
                    .find(|e| e.request_route == route_label)
                {
                    (
                        entry.path.clone(),
                        entry.request_callback_symbol.clone(),
                        entry.request_route.clone(),
                        "request",
                    )
                } else {
                    return Ok(None);
                }
            }
            onebot11_dispatch::OneBotSystemRoute::Meta(meta) => {
                let route_label = match meta {
                    onebot11_dispatch::MetaRoute::Heartbeat => "Heartbeat",
                    onebot11_dispatch::MetaRoute::LifecycleConnect => "LifecycleConnect",
                    onebot11_dispatch::MetaRoute::LifecycleEnable => "LifecycleEnable",
                    onebot11_dispatch::MetaRoute::LifecycleDisable => "LifecycleDisable",
                    _ => return Ok(None),
                };
                if let Some((entry, route_entry)) = find_route_entry(report, "meta", route_label) {
                    (
                        entry.path.clone(),
                        route_entry.callback_symbol.clone(),
                        route_label.to_string(),
                        "meta",
                    )
                } else if let Some(entry) = report
                    .dynamic_plugins
                    .iter()
                    .find(|e| e.meta_route == route_label)
                {
                    (
                        entry.path.clone(),
                        entry.meta_callback_symbol.clone(),
                        entry.meta_route.clone(),
                        "meta",
                    )
                } else {
                    return Ok(None);
                }
            }
            onebot11_dispatch::OneBotSystemRoute::MessageSent(_) => {
                return Ok(None);
            }
        };

        // Phase 1: briefly hold outer lock to get per-library handle
        let lib_path = library_path.clone();
        let lib_handle = {
            let mut runtime = self
                .dynamic_runtime
                .lock()
                .map_err(|_| QimenError::Runtime("dynamic runtime lock poisoned".to_string()))?;
            runtime.get_library(&lib_path)?
        }; // outer lock released

        // Phase 2: spawn_blocking + timeout
        let timeout_dur = self.dynamic_plugin_timeout;
        let ffi_result = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || {
                DynamicPluginRuntime::execute_route_on_handle(
                    &lib_handle,
                    &library_path,
                    &callback_symbol,
                    route_str,
                    kind,
                    &raw_json,
                )
            }),
        )
        .await;

        match ffi_result {
            Ok(Ok(inner)) => Ok(Some(inner?)),
            Ok(Err(join_err)) => Err(QimenError::Runtime(format!(
                "dynamic plugin system route spawn_blocking panicked: {join_err}"
            ))),
            Err(_) => {
                if let Ok(mut runtime) = self.dynamic_runtime.lock() {
                    runtime.record_timeout(&lib_path);
                }
                Err(QimenError::Runtime(format!(
                    "dynamic plugin system route timed out after {}s",
                    timeout_dur.as_secs()
                )))
            }
        }
    }

    fn build_system_event_context<'a>(
        bot: &'a BotRuntimeInfo,
        payload: &'a Value,
        runtime: &'a dyn RuntimeBotContext,
    ) -> onebot11_dispatch::SystemEventContext<'a> {
        onebot11_dispatch::SystemEventContext {
            bot_id: &bot.id,
            payload,
            runtime,
            auto_approve_friend_requests: bot.auto_approve_friend_requests,
            auto_approve_group_invites: bot.auto_approve_group_invites,
            auto_reply_poke_enabled: bot.auto_reply_poke_enabled,
            auto_reply_poke_message: bot.auto_reply_poke_message.as_deref(),
            auto_approve_friend_request_user_whitelist: &bot
                .auto_approve_friend_request_user_whitelist,
            auto_approve_friend_request_user_blacklist: &bot
                .auto_approve_friend_request_user_blacklist,
            auto_approve_friend_request_comment_keywords: &bot
                .auto_approve_friend_request_comment_keywords,
            auto_reject_friend_request_comment_keywords: &bot
                .auto_reject_friend_request_comment_keywords,
            auto_approve_friend_request_remark: bot.auto_approve_friend_request_remark.as_deref(),
            auto_approve_group_invite_user_whitelist: &bot.auto_approve_group_invite_user_whitelist,
            auto_approve_group_invite_user_blacklist: &bot.auto_approve_group_invite_user_blacklist,
            auto_approve_group_invite_group_whitelist: &bot
                .auto_approve_group_invite_group_whitelist,
            auto_approve_group_invite_group_blacklist: &bot
                .auto_approve_group_invite_group_blacklist,
            auto_approve_group_invite_comment_keywords: &bot
                .auto_approve_group_invite_comment_keywords,
            auto_reject_group_invite_comment_keywords: &bot
                .auto_reject_group_invite_comment_keywords,
            auto_reject_group_invite_reason: bot.auto_reject_group_invite_reason.as_deref(),
        }
    }
}

/// Find a route entry in the v0.2 routes of dynamic plugins.
fn find_route_entry<'a>(
    report: &'a HostPluginReport,
    kind: &str,
    route_label: &str,
) -> Option<(
    &'a qimen_host_types::DynamicPluginReportEntry,
    &'a qimen_host_types::DynamicRouteEntry,
)> {
    for entry in &report.dynamic_plugins {
        for route_entry in &entry.routes {
            if route_entry.kind == kind {
                for name in route_entry.route.split(',').map(|s| s.trim()) {
                    if name == route_label {
                        return Some((entry, route_entry));
                    }
                }
            }
        }
    }
    None
}

enum SessionEnd {
    Shutdown,
    Reconnect(String),
    PluginReload {
        reply_action: Option<NormalizedActionRequest>,
    },
}

enum SessionSignal {
    EventHandled,
    Heartbeat(u64),
    EndSession(SessionEnd),
}

enum NormalizedCommandOutcome {
    Reply(Message),
    Reload {
        reply_action: Option<NormalizedActionRequest>,
    },
    None,
}

fn build_send_msg_action(
    bot: &BotRuntimeInfo,
    event: &Value,
    reply: Message,
) -> Result<NormalizedActionRequest> {
    let message = reply.to_onebot_value();

    let mut params = serde_json::Map::new();
    match event.get("message_type").and_then(Value::as_str) {
        Some("private") => {
            let user_id = event.get("user_id").cloned().ok_or_else(|| {
                QimenError::Protocol("private message missing user_id".to_string())
            })?;
            params.insert(
                "message_type".to_string(),
                Value::String("private".to_string()),
            );
            params.insert("user_id".to_string(), user_id);
        }
        Some("group") => {
            let group_id = event.get("group_id").cloned().ok_or_else(|| {
                QimenError::Protocol("group message missing group_id".to_string())
            })?;
            params.insert(
                "message_type".to_string(),
                Value::String("group".to_string()),
            );
            params.insert("group_id".to_string(), group_id);
        }
        Some(other) => {
            return Err(QimenError::Protocol(format!(
                "unsupported message_type for reply: {other}"
            )));
        }
        None => {
            return Err(QimenError::Protocol(
                "message event missing message_type".to_string(),
            ));
        }
    }

    params.insert("message".to_string(), message);
    params.insert("auto_escape".to_string(), Value::Bool(false));

    Ok(NormalizedActionRequest {
        protocol: ProtocolId::OneBot11,
        bot_instance: bot.id.clone(),
        action: "send_msg".to_string(),
        params: Value::Object(params),
        echo: Some(json!(build_echo(bot))),
        timeout_ms: 5000,
        metadata: ActionMeta {
            source: "mvp-auto-reply".to_string(),
        },
    })
}

fn build_qqbot_send_msg_action(
    bot: &BotRuntimeInfo,
    event: &qimen_protocol_core::NormalizedEvent,
    reply: Message,
) -> Result<NormalizedActionRequest> {
    let chat = event
        .chat
        .as_ref()
        .ok_or_else(|| QimenError::Protocol("qqbot reply event missing chat".to_string()))?;
    let mut params = serde_json::Map::new();
    params.insert("message".to_string(), reply.to_onebot_value());
    if let Some(message_id) = event.message_id_str() {
        params.insert("msg_id".to_string(), Value::String(message_id));
    }
    if let Some(event_id) = event.extensions.get("event_id").cloned() {
        params.insert("event_id".to_string(), event_id);
    }
    if let Some(msg_seq) = event.extensions.get("msg_seq").cloned() {
        params.insert("msg_seq".to_string(), msg_seq);
    }

    let action = match chat.kind.as_str() {
        "group" => {
            params.insert("group_openid".to_string(), Value::String(chat.id.clone()));
            "send_group_msg"
        }
        "private" => {
            params.insert("openid".to_string(), Value::String(chat.id.clone()));
            "send_private_msg"
        }
        "channel" => {
            params.insert("channel_id".to_string(), Value::String(chat.id.clone()));
            if let Some(guild_id) = event.extensions.get("guild_id").cloned() {
                params.insert("guild_id".to_string(), guild_id);
            }
            "send_channel_msg"
        }
        "channel_private" => {
            params.insert("guild_id".to_string(), Value::String(chat.id.clone()));
            "send_dms"
        }
        other => {
            return Err(QimenError::Protocol(format!(
                "unsupported qqbot chat kind for reply: {other}"
            )));
        }
    };

    Ok(NormalizedActionRequest {
        protocol: ProtocolId::QqOfficial,
        bot_instance: bot.id.clone(),
        action: action.to_string(),
        params: Value::Object(params),
        echo: Some(json!(build_echo(bot))),
        timeout_ms: 5000,
        metadata: ActionMeta {
            source: "qqbot-reply".to_string(),
        },
    })
}

fn build_qqbot_notice_reply_action(
    bot: &BotRuntimeInfo,
    event: &NormalizedEvent,
    reply: Message,
    source: &str,
) -> Option<NormalizedActionRequest> {
    let mut params = serde_json::Map::new();
    params.insert("message".to_string(), reply.to_onebot_value());

    let action = if let Some(group_openid) = event
        .extensions
        .get("group_openid")
        .and_then(value_to_optional_string)
    {
        params.insert("group_openid".to_string(), Value::String(group_openid));
        "send_group_msg"
    } else if let Some(openid) = event
        .extensions
        .get("openid")
        .or_else(|| event.extensions.get("user_openid"))
        .and_then(value_to_optional_string)
    {
        params.insert("openid".to_string(), Value::String(openid));
        "send_private_msg"
    } else if let Some(channel_id) = event
        .extensions
        .get("channel_id")
        .and_then(value_to_optional_string)
    {
        params.insert("channel_id".to_string(), Value::String(channel_id));
        "send_channel_msg"
    } else {
        let guild_id = event
            .extensions
            .get("guild_id")
            .and_then(value_to_optional_string)?;
        params.insert("guild_id".to_string(), Value::String(guild_id));
        "send_dms"
    };

    if let Some(message_id) = event.message_id_str() {
        params.insert("msg_id".to_string(), Value::String(message_id));
    }
    if let Some(event_id) = event.extensions.get("event_id").cloned() {
        params.insert("event_id".to_string(), event_id);
    }
    if let Some(msg_seq) = event.extensions.get("msg_seq").cloned() {
        params.insert("msg_seq".to_string(), msg_seq);
    }

    Some(NormalizedActionRequest {
        protocol: ProtocolId::QqOfficial,
        bot_instance: bot.id.clone(),
        action: action.to_string(),
        params: Value::Object(params),
        echo: Some(json!(build_echo(bot))),
        timeout_ms: 5000,
        metadata: ActionMeta {
            source: source.to_string(),
        },
    })
}

fn build_echo(bot: &BotRuntimeInfo) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    format!("reply-{}-{millis}", bot.id)
}

fn build_event_dedup_key(event: &qimen_protocol_core::NormalizedEvent, message_id: &str) -> String {
    let chat = event
        .chat
        .as_ref()
        .map(|chat| format!("{}:{}", chat.kind, chat.id))
        .unwrap_or_else(|| "unknown".to_string());
    format!(
        "{}:{:?}:{}:{}",
        event.bot_instance, event.protocol, chat, message_id
    )
}

fn value_to_optional_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) if text.is_empty() => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        other => Some(other.to_string()),
    }
}

async fn upload_qqbot_media(
    client: &QqBotOpenApiClient,
    route: &str,
    target_id: &str,
    upload: &Value,
) -> Result<Value> {
    let payload = UploadFilePayload {
        file_type: upload.get("file_type").and_then(Value::as_i64).unwrap_or(1),
        url: upload
            .get("url")
            .and_then(value_to_optional_string)
            .ok_or_else(|| QimenError::Protocol("qqbot media upload missing url".to_string()))?,
        srv_send_msg: upload
            .get("srv_send_msg")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };

    match route {
        "group_file" => client.post_group_file(target_id, &payload).await,
        "c2c_file" => client.post_c2c_file(target_id, &payload).await,
        other => Err(QimenError::Protocol(format!(
            "unsupported qqbot media upload route '{other}'"
        ))),
    }
}

fn qqbot_ok_action_response(
    bot: &BotRuntimeInfo,
    action: &NormalizedActionRequest,
    route: &str,
    data: Value,
) -> NormalizedActionResponse {
    NormalizedActionResponse {
        protocol: ProtocolId::QqOfficial,
        bot_instance: bot.id.clone(),
        status: ActionStatus::Ok,
        retcode: 0,
        data: data.clone(),
        echo: action.echo.clone(),
        latency_ms: 0,
        raw_json: json!({
            "code": 0,
            "data": data,
            "route": route,
            "echo": action.echo,
        }),
    }
}

fn qqbot_failed_action_response(
    bot: &BotRuntimeInfo,
    action: &NormalizedActionRequest,
    route: Option<&str>,
    category: &str,
    error: &str,
    retry_after_ms: Option<u64>,
) -> NormalizedActionResponse {
    let retcode = qqbot_error_retcode(category);
    NormalizedActionResponse {
        protocol: ProtocolId::QqOfficial,
        bot_instance: bot.id.clone(),
        status: ActionStatus::Failed,
        retcode,
        data: json!({
            "category": category,
            "error": error,
            "retry_after_ms": retry_after_ms,
        }),
        echo: action.echo.clone(),
        latency_ms: 0,
        raw_json: json!({
            "code": retcode,
            "status": "failed",
            "category": category,
            "message": error,
            "retry_after_ms": retry_after_ms,
            "route": route,
            "echo": action.echo,
        }),
    }
}

fn qqbot_error_category(error: &str) -> &'static str {
    if error.contains("RateLimited") || error.contains("HTTP 429") {
        "RateLimited"
    } else if error.contains("Authentication") || error.contains("HTTP 401") {
        "Authentication"
    } else if error.contains("Permission") || error.contains("HTTP 403") {
        "Permission"
    } else if error.contains("NotFound") || error.contains("HTTP 404") {
        "NotFound"
    } else if error.contains("BadRequest") || error.contains("HTTP 400") {
        "BadRequest"
    } else if error.contains("Server") || error.contains("HTTP 5") {
        "Server"
    } else if error.contains("protocol error") {
        "Protocol"
    } else {
        "Unknown"
    }
}

fn qqbot_error_retcode(category: &str) -> i64 {
    match category {
        "Authentication" => 401,
        "Permission" => 403,
        "RateLimited" => 429,
        "NotFound" => 404,
        "BadRequest" => 400,
        "Server" => 500,
        "Protocol" => 422,
        _ => -1,
    }
}

fn qqbot_error_retry_after_ms(error: &str) -> Option<u64> {
    let rest = error.split("retry_after_ms=").nth(1)?;
    let digits = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn qqbot_backoff_key(bot: &BotRuntimeInfo, route: &str) -> String {
    format!("{}:{route}", bot.id)
}

fn build_notice_reply_action(
    bot: &BotRuntimeInfo,
    payload: &Value,
    message: String,
) -> Result<Option<NormalizedActionRequest>> {
    let message_type = payload.get("notice_type").and_then(Value::as_str);
    if message_type != Some("notify") {
        return Ok(None);
    }

    let sub_type = payload.get("sub_type").and_then(Value::as_str);
    if sub_type != Some("poke") {
        return Ok(None);
    }

    let mut params = serde_json::Map::new();
    if let Some(group_id) = payload.get("group_id").cloned() {
        params.insert(
            "message_type".to_string(),
            Value::String("group".to_string()),
        );
        params.insert("group_id".to_string(), group_id);
    } else if let Some(user_id) = payload.get("user_id").cloned() {
        params.insert(
            "message_type".to_string(),
            Value::String("private".to_string()),
        );
        params.insert("user_id".to_string(), user_id);
    } else {
        return Ok(None);
    }

    params.insert(
        "message".to_string(),
        Message::builder().text(message).build().to_onebot_value(),
    );
    params.insert("auto_escape".to_string(), Value::Bool(false));

    Ok(Some(NormalizedActionRequest {
        protocol: ProtocolId::OneBot11,
        bot_instance: bot.id.clone(),
        action: "send_msg".to_string(),
        params: Value::Object(params),
        echo: Some(json!(build_echo(bot))),
        timeout_ms: 5000,
        metadata: ActionMeta {
            source: "auto-reply-poke-notice".to_string(),
        },
    }))
}

fn heartbeat_timeout(interval_ms: u64, fallback: Duration) -> Duration {
    if interval_ms == 0 {
        return fallback;
    }

    let interval = Duration::from_millis(interval_ms);
    let extended = interval.saturating_mul(3);
    if extended > fallback {
        extended
    } else {
        fallback
    }
}

fn parse_protocol(value: &str) -> ProtocolId {
    match value {
        "onebot11" => ProtocolId::OneBot11,
        "onebot12" => ProtocolId::OneBot12,
        "qq-official" | "qqbot" => ProtocolId::QqOfficial,
        "satori" => ProtocolId::Satori,
        other => ProtocolId::Custom(other.to_string()),
    }
}

fn parse_transport(value: &str) -> TransportMode {
    match value {
        "ws-forward" => TransportMode::WsForward,
        "ws-reverse" => TransportMode::WsReverse,
        "http-api" => TransportMode::HttpApi,
        "http-post" => TransportMode::HttpPost,
        "gateway" => TransportMode::Gateway,
        other => TransportMode::Custom(other.to_string()),
    }
}

#[async_trait::async_trait]
impl RuntimeBotContext for QqOfficialRuntimeContext<'_> {
    fn bot_instance(&self) -> &str {
        &self.bot.id
    }

    fn protocol(&self) -> ProtocolId {
        self.bot.protocol.clone()
    }

    fn capabilities(&self) -> &CapabilitySet {
        &self.bot.capabilities
    }

    async fn send_action(&self, req: NormalizedActionRequest) -> Result<NormalizedActionResponse> {
        self.runtime
            .execute_qqbot_action(self.bot, self.adapter, self.client, req)
            .await
    }

    async fn reply(
        &self,
        event: &qimen_protocol_core::NormalizedEvent,
        message: Message,
    ) -> Result<NormalizedActionResponse> {
        let action = build_qqbot_send_msg_action(self.bot, event, message)?;
        self.runtime
            .execute_qqbot_action(self.bot, self.adapter, self.client, action)
            .await
    }

    fn spawn_owned(&self, name: &str, fut: OwnedTaskFuture) -> TaskHandle {
        tokio::spawn(fut);
        TaskHandle {
            name: name.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl NormalizedActionExecutor for QqOfficialRuntimeContext<'_> {
    fn build_reply_action(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionRequest> {
        build_qqbot_send_msg_action(self.bot, event, reply)
    }

    async fn reply_to_event(
        &self,
        event: &NormalizedEvent,
        reply: Message,
    ) -> Result<NormalizedActionResponse> {
        let action = self.build_reply_action(event, reply)?;
        self.runtime
            .execute_qqbot_action(self.bot, self.adapter, self.client, action)
            .await
    }

    async fn process_dynamic_sends(&self, sends: Vec<SendAction>) -> Result<()> {
        self.runtime
            .process_qqbot_send_actions(self.bot, self.adapter, self.client, sends)
            .await
    }

    fn dedup_key(&self, event: &NormalizedEvent, message_id: &str) -> String {
        build_event_dedup_key(event, message_id)
    }
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn render_plugin_status_text(command_dispatcher: &CommandDispatcher) -> String {
    let entries = command_dispatcher.plugin_status_entries();
    if entries.is_empty() {
        return "no registered plugins".to_string();
    }

    let mut static_plugins = Vec::new();
    let mut dynamic_descriptors = Vec::new();
    let mut dynamic_health = Vec::new();

    for entry in &entries {
        if entry.dynamic {
            dynamic_descriptors.push(entry);
            if entry
                .command_descriptions
                .iter()
                .any(|item| item.starts_with("health:"))
            {
                dynamic_health.push(entry);
            }
        } else {
            static_plugins.push(entry);
        }
    }

    let mut lines = vec!["[plugins]".to_string()];
    lines.push("[static plugins]".to_string());
    lines.extend(render_plugin_section(&static_plugins));
    lines.push("[dynamic descriptors]".to_string());
    lines.extend(render_plugin_section(&dynamic_descriptors));
    lines.push("[dynamic runtime health]".to_string());
    lines.extend(render_plugin_section(&dynamic_health));
    lines.join("\n")
}

fn render_registry_report(command_dispatcher: &CommandDispatcher) -> String {
    let diagnostics = command_dispatcher.registry().diagnostics();
    let precedence = command_dispatcher.registry().precedence_report();
    let mut lines = vec!["[registry report]".to_string()];

    lines.push("[diagnostics]".to_string());
    if diagnostics.is_empty() {
        lines.push("  - none".to_string());
    } else {
        lines.extend(diagnostics.iter().map(|item| {
            format!(
                "  - key={} incoming={} existing={}",
                item.key,
                item.incoming_source,
                item.existing_sources.join(",")
            )
        }));
    }

    lines.push("[effective commands]".to_string());
    for (definition, source) in command_dispatcher.describe_commands() {
        lines.push(format!(
            "  - {} (source={}, category={}, role={})",
            definition.name,
            source,
            definition.category,
            match definition.required_role {
                qimen_plugin_api::CommandRole::Anyone => "anyone",
                qimen_plugin_api::CommandRole::Admin => "admin",
                qimen_plugin_api::CommandRole::Owner => "owner",
            }
        ));
    }

    lines.push("[precedence]".to_string());
    for (key, entries) in precedence {
        let summary = entries
            .iter()
            .map(|(source, priority)| format!("{}(p={})", source, priority))
            .collect::<Vec<_>>()
            .join(" > ");
        lines.push(format!("  - {} => {}", key, summary));
    }

    lines.join("\n")
}

fn render_registry_conflicts_report(command_dispatcher: &CommandDispatcher) -> String {
    let diagnostics = command_dispatcher.registry().diagnostics();
    let mut lines = vec!["[registry conflicts]".to_string()];
    if diagnostics.is_empty() {
        lines.push("  - none".to_string());
        return lines.join("\n");
    }

    for item in diagnostics {
        lines.push(format!(
            "  - key={}\n    incoming={}\n    existing={}",
            item.key,
            item.incoming_source,
            item.existing_sources.join(",")
        ));
    }

    lines.join("\n")
}

fn render_dynamic_errors_report(health: &[qimen_host_types::DynamicRuntimeHealthEntry]) -> String {
    let mut lines = vec!["[dynamic errors]".to_string()];
    if health.is_empty() {
        lines.push("  - none".to_string());
        return lines.join("\n");
    }

    for entry in health {
        lines.push(format!(
            "  - path={}\n    failures={}\n    isolated_until={}\n    last_error={}",
            entry.path,
            entry.failures,
            entry
                .isolated_until_epoch_ms
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry.last_error.clone().unwrap_or_else(|| "-".to_string())
        ));
        if !entry.recent_errors.is_empty() {
            lines.push(format!(
                "    recent_errors={}",
                entry.recent_errors.join(" | ")
            ));
        }
    }

    lines.join("\n")
}

fn render_plugin_section(entries: &[&command_dispatch::PluginStatusEntry]) -> Vec<String> {
    if entries.is_empty() {
        return vec!["  - none".to_string()];
    }

    entries
        .iter()
        .map(|entry| {
            format!(
                "  - plugin: {}\n    name: {}\n    version: {}\n    api: {}\n    state: {}\n    dynamic: {}\n    command_descriptions: {}\n    callback: {}\n    commands: {}",
                entry.id,
                entry.name,
                entry.version,
                entry.api_version,
                match entry.enabled {
                    Some(true) => "enabled",
                    Some(false) => "disabled",
                    None => "unknown",
                },
                if entry.dynamic { "yes" } else { "no" },
                if entry.command_descriptions.is_empty() {
                    "-".to_string()
                } else {
                    entry.command_descriptions.join(" | ")
                },
                entry.callback_symbol.as_deref().unwrap_or("-"),
                if entry.commands.is_empty() {
                    "-".to_string()
                } else {
                    entry.commands.join(",")
                }
            )
        })
        .collect()
}

/// Convert a TOML value to a serde_json::Value.
fn toml_to_json(val: &toml::Value) -> serde_json::Value {
    match val {
        toml::Value::String(s) => serde_json::Value::String(s.clone()),
        toml::Value::Integer(i) => serde_json::Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        toml::Value::Boolean(b) => serde_json::Value::Bool(*b),
        toml::Value::Datetime(dt) => serde_json::Value::String(dt.to_string()),
        toml::Value::Array(arr) => serde_json::Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map = tbl
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use qimen_plugin_api::MessageEventInterceptor;
    use qimen_protocol_core::{ActorRef, ChatRef};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn ws_reverse_boot_stays_alive_while_waiting_for_connection() {
        let config: AppConfig = toml::from_str(
            r#"
[runtime]
env = "test"
shutdown_timeout_secs = 1
task_grace_secs = 1

[observability]
level = "info"
json_logs = false
metrics_bind = "127.0.0.1:0"

[official_host]
builtin_modules = []
plugin_modules = []

[[bots]]
id = "reverse-bot"
protocol = "onebot11"
transport = "ws-reverse"
bind = "127.0.0.1:0"
path = "/onebot/reverse"
"#,
        )
        .unwrap();
        let runtime = Runtime::from_config(&config);

        assert!(
            tokio::time::timeout(Duration::from_millis(100), runtime.boot())
                .await
                .is_err(),
            "ws-reverse boot must keep running while it waits for clients"
        );
    }

    #[test]
    fn qqbot_error_helpers_detect_rate_limit_backoff() {
        let error = "transport error: qqbot request /v2/users/u/messages failed with HTTP 429, code 11241, category RateLimited: rate limit exceeded, retry_after_ms=1500";

        assert_eq!(qqbot_error_category(error), "RateLimited");
        assert_eq!(qqbot_error_retry_after_ms(error), Some(1500));
    }

    #[test]
    fn qqbot_error_helpers_classify_permission() {
        let error = "transport error: qqbot request /channels/c/messages failed with HTTP 403, code 304003, category Permission: permission denied";

        assert_eq!(qqbot_error_category(error), "Permission");
        assert_eq!(qqbot_error_retry_after_ms(error), None);
    }

    struct RecordingQqOfficialExecutor {
        replies: Arc<std::sync::Mutex<Vec<String>>>,
        dynamic_send_count: Arc<AtomicUsize>,
    }

    impl RecordingQqOfficialExecutor {
        fn new() -> Self {
            Self {
                replies: Arc::new(std::sync::Mutex::new(Vec::new())),
                dynamic_send_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl RuntimeBotContext for RecordingQqOfficialExecutor {
        fn bot_instance(&self) -> &str {
            "qq-official"
        }

        fn protocol(&self) -> ProtocolId {
            ProtocolId::QqOfficial
        }

        fn capabilities(&self) -> &CapabilitySet {
            static CAPABILITIES: std::sync::OnceLock<CapabilitySet> = std::sync::OnceLock::new();
            CAPABILITIES.get_or_init(CapabilitySet::default)
        }

        async fn send_action(
            &self,
            _req: NormalizedActionRequest,
        ) -> Result<NormalizedActionResponse> {
            Ok(ok_test_action_response())
        }

        async fn reply(
            &self,
            _event: &NormalizedEvent,
            message: Message,
        ) -> Result<NormalizedActionResponse> {
            self.replies.lock().unwrap().push(message.plain_text());
            Ok(ok_test_action_response())
        }

        fn spawn_owned(&self, name: &str, _fut: OwnedTaskFuture) -> TaskHandle {
            TaskHandle {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl NormalizedActionExecutor for RecordingQqOfficialExecutor {
        fn build_reply_action(
            &self,
            event: &NormalizedEvent,
            reply: Message,
        ) -> Result<NormalizedActionRequest> {
            Ok(NormalizedActionRequest {
                protocol: ProtocolId::QqOfficial,
                bot_instance: event.bot_instance.clone(),
                action: "send_msg".to_string(),
                params: json!({
                    "message": reply.to_onebot_value(),
                    "chat": event.chat.as_ref().map(|chat| {
                        json!({
                            "kind": chat.kind,
                            "id": chat.id,
                        })
                    }),
                }),
                echo: None,
                timeout_ms: 5000,
                metadata: ActionMeta {
                    source: "test".to_string(),
                },
            })
        }

        async fn reply_to_event(
            &self,
            _event: &NormalizedEvent,
            reply: Message,
        ) -> Result<NormalizedActionResponse> {
            self.replies.lock().unwrap().push(reply.plain_text());
            Ok(ok_test_action_response())
        }

        async fn process_dynamic_sends(&self, sends: Vec<SendAction>) -> Result<()> {
            self.dynamic_send_count
                .fetch_add(sends.len(), Ordering::SeqCst);
            Ok(())
        }

        fn dedup_key(&self, event: &NormalizedEvent, message_id: &str) -> String {
            format!(
                "{}:{:?}:{}:{}",
                event.bot_instance,
                event.protocol,
                event
                    .chat
                    .as_ref()
                    .map(|chat| chat.kind.as_str())
                    .unwrap_or("-"),
                message_id
            )
        }
    }

    struct CountingInterceptor {
        pre_count: Arc<AtomicUsize>,
        after_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl MessageEventInterceptor for CountingInterceptor {
        async fn pre_handle(&self, _bot_id: &str, _event: &NormalizedEvent) -> bool {
            self.pre_count.fetch_add(1, Ordering::SeqCst);
            true
        }

        async fn after_completion(&self, _bot_id: &str, _event: &NormalizedEvent) {
            self.after_count.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn qq_official_bot() -> BotRuntimeInfo {
        BotRuntimeInfo {
            id: "qq-official".to_string(),
            protocol: ProtocolId::QqOfficial,
            transport: TransportMode::Gateway,
            capabilities: CapabilitySet::default(),
            endpoint: None,
            bind: None,
            path: None,
            access_token: None,
            appid: Some("appid".to_string()),
            secret: Some("secret".to_string()),
            intents: vec!["public_messages".to_string()],
            sandbox: false,
            enabled: true,
            owners: Vec::new(),
            admins: Vec::new(),
            auto_approve_friend_requests: false,
            auto_approve_group_invites: false,
            auto_approve_friend_request_user_whitelist: Vec::new(),
            auto_approve_friend_request_user_blacklist: Vec::new(),
            auto_approve_friend_request_comment_keywords: Vec::new(),
            auto_reject_friend_request_comment_keywords: Vec::new(),
            auto_approve_friend_request_remark: None,
            auto_approve_group_invite_user_whitelist: Vec::new(),
            auto_approve_group_invite_user_blacklist: Vec::new(),
            auto_approve_group_invite_group_whitelist: Vec::new(),
            auto_approve_group_invite_group_blacklist: Vec::new(),
            auto_approve_group_invite_comment_keywords: Vec::new(),
            auto_reject_group_invite_comment_keywords: Vec::new(),
            auto_reject_group_invite_reason: None,
            auto_reply_poke_enabled: false,
            auto_reply_poke_message: None,
            limiter_config: RateLimiterConfig {
                enable: false,
                rate: 1.0,
                capacity: 1,
                timeout_secs: 0,
            },
        }
    }

    fn qq_official_message_event(chat_kind: &str, text: &str, message_id: &str) -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::QqOfficial,
            bot_instance: "qq-official".to_string(),
            transport_mode: TransportMode::Gateway,
            time: Some(1),
            kind: EventKind::Message,
            message: Some(Message::text(text)),
            actor: Some(ActorRef {
                id: "user-openid".to_string(),
                display_name: Some("tester".to_string()),
            }),
            chat: Some(ChatRef {
                id: format!("{chat_kind}-chat"),
                kind: chat_kind.to_string(),
            }),
            raw_json: json!({
                "message_id": message_id,
                "user_id": "user-openid",
                "message": text,
            }),
            raw_bytes: None,
            extensions: serde_json::Map::new(),
        }
    }

    fn ok_test_action_response() -> NormalizedActionResponse {
        NormalizedActionResponse {
            protocol: ProtocolId::QqOfficial,
            bot_instance: "qq-official".to_string(),
            status: ActionStatus::Ok,
            retcode: 0,
            data: Value::Null,
            echo: None,
            latency_ms: 0,
            raw_json: json!({
                "status": "ok",
                "retcode": 0,
            }),
        }
    }

    #[tokio::test]
    async fn qq_official_message_pipeline_replies_for_supported_chats() {
        let runtime = Runtime::default();
        let bot = qq_official_bot();
        let dispatcher = CommandDispatcher::with_default_handlers();
        let help_text = render_help_text(&dispatcher.describe_commands());
        let executor = RecordingQqOfficialExecutor::new();
        let limiter = TokenBucketLimiter::new(&bot.limiter_config);

        for (index, chat_kind) in ["group", "private", "channel", "guild_private"]
            .into_iter()
            .enumerate()
        {
            let event = qq_official_message_event(chat_kind, "/ping", &format!("msg-{index}"));
            let signal = runtime
                .handle_normalized_event(&bot, event, &dispatcher, &help_text, &executor, &limiter)
                .await
                .expect("message should be handled");

            assert!(matches!(signal, SessionSignal::EventHandled));
        }

        let replies = executor.replies.lock().unwrap();
        assert_eq!(&*replies, &["pong", "pong", "pong", "pong"]);
    }

    #[tokio::test]
    async fn qq_official_help_runs_interceptors_and_replies() {
        let runtime = Runtime::default();
        let pre_count = Arc::new(AtomicUsize::new(0));
        let after_count = Arc::new(AtomicUsize::new(0));
        runtime
            .interceptor_chain
            .write()
            .unwrap()
            .add(Arc::new(CountingInterceptor {
                pre_count: Arc::clone(&pre_count),
                after_count: Arc::clone(&after_count),
            }));

        let bot = qq_official_bot();
        let dispatcher = CommandDispatcher::with_default_handlers();
        let help_text = render_help_text(&dispatcher.describe_commands());
        let executor = RecordingQqOfficialExecutor::new();
        let limiter = TokenBucketLimiter::new(&bot.limiter_config);
        let event = qq_official_message_event("private", "/help", "help-msg");

        let signal = runtime
            .handle_normalized_event(&bot, event, &dispatcher, &help_text, &executor, &limiter)
            .await
            .expect("help should be handled");

        assert!(matches!(signal, SessionSignal::EventHandled));
        assert_eq!(pre_count.load(Ordering::SeqCst), 1);
        assert_eq!(after_count.load(Ordering::SeqCst), 1);

        let replies = executor.replies.lock().unwrap();
        assert_eq!(replies.len(), 1);
        assert!(replies[0].contains("[help]"));
        assert!(replies[0].contains("/ping"));
    }
}
