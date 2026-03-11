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

use self::command_dispatch::{
    CommandDispatchSignal, CommandDispatcher, render_help_text,
};
use crate::dedup::MessageDedup;
use crate::group_event_filter::GroupEventFilter;
use crate::plugin_acl::PluginAclManager;
use qimen_adapter_onebot11::OneBot11Adapter;
use qimen_config::AppConfig;
use qimen_error::{QimenError, Result};
use qimen_host_types::{
    DynamicCommandDescriptor, DynamicMetaDescriptor, DynamicNoticeDescriptor,
    DynamicRequestDescriptor, HostPluginReport, load_plugin_state,
};
use qimen_message::Message;
use qimen_plugin_api::{
    BuiltinCommandAction, CommandPlugin, OwnedTaskFuture, PluginBundle, RateLimiterConfig,
    RuntimeBotContext, SystemPlugin, TaskHandle,
};
use self::interceptor::InterceptorChain;
use self::rate_limiter::TokenBucketLimiter;
use self::dynamic_runtime::{DynamicPluginRuntime, DynamicResponse};
use self::permission::PermissionResolver;
use self::onebot11_dispatch::{OneBotSystemDispatchSignal, OneBotSystemDispatcher};
use qimen_protocol_core::{
    ActionMeta, CapabilitySet, IncomingPacket, NormalizedActionRequest, ProtocolAdapter,
    ProtocolId, TransportMode,
};
use qimen_transport_ws::{OneBot11ForwardWsClient, ReconnectPolicy};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;

struct OneBotRuntimeContext<'a> {
    runtime: &'a Runtime,
    bot: &'a BotRuntimeInfo,
    adapter: &'a OneBot11Adapter,
    client: &'a OneBot11ForwardWsClient,
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

#[derive(Debug, Clone)]
pub struct BotRuntimeInfo {
    pub id: String,
    pub protocol: ProtocolId,
    pub transport: TransportMode,
    pub capabilities: CapabilitySet,
    pub endpoint: Option<String>,
    pub access_token: Option<String>,
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

pub struct Runtime {
    bots: Vec<BotRuntimeInfo>,
    command_plugins: Vec<std::sync::Arc<dyn CommandPlugin>>,
    system_plugins: Vec<std::sync::Arc<dyn SystemPlugin>>,
    host_plugin_report: std::sync::RwLock<Option<HostPluginReport>>,
    plugin_state_path: Option<String>,
    plugin_bin_dir: Option<String>,
    dynamic_runtime: std::sync::Mutex<DynamicPluginRuntime>,
    interceptor_chain: InterceptorChain,
    rate_limiters: Vec<TokenBucketLimiter>,
    pub dedup: Arc<MessageDedup>,
    pub group_event_filter: Arc<GroupEventFilter>,
    pub plugin_acl: Arc<PluginAclManager>,
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
            dynamic_runtime: std::sync::Mutex::new(DynamicPluginRuntime::new()),
            interceptor_chain: InterceptorChain::new(),
            rate_limiters: Vec::new(),
            dedup: Arc::new(MessageDedup::new(60, 10000)),
            group_event_filter: Arc::new(GroupEventFilter::disabled()),
            plugin_acl: Arc::new(PluginAclManager::new()),
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
                access_token: bot.access_token.clone(),
                enabled: bot.enabled,
                owners: bot.owners.clone(),
                admins: bot.admins.clone(),
                auto_approve_friend_requests: bot.auto_approve_friend_requests,
                auto_approve_group_invites: bot.auto_approve_group_invites,
                auto_approve_friend_request_user_whitelist: bot.auto_approve_friend_request_user_whitelist.clone(),
                auto_approve_friend_request_user_blacklist: bot.auto_approve_friend_request_user_blacklist.clone(),
                auto_approve_friend_request_comment_keywords: bot.auto_approve_friend_request_comment_keywords.clone(),
                auto_reject_friend_request_comment_keywords: bot.auto_reject_friend_request_comment_keywords.clone(),
                auto_approve_friend_request_remark: normalize_optional_string(bot.auto_approve_friend_request_remark.clone()),
                auto_approve_group_invite_user_whitelist: bot.auto_approve_group_invite_user_whitelist.clone(),
                auto_approve_group_invite_user_blacklist: bot.auto_approve_group_invite_user_blacklist.clone(),
                auto_approve_group_invite_group_whitelist: bot.auto_approve_group_invite_group_whitelist.clone(),
                auto_approve_group_invite_group_blacklist: bot.auto_approve_group_invite_group_blacklist.clone(),
                auto_approve_group_invite_comment_keywords: bot.auto_approve_group_invite_comment_keywords.clone(),
                auto_reject_group_invite_comment_keywords: bot.auto_reject_group_invite_comment_keywords.clone(),
                auto_reject_group_invite_reason: normalize_optional_string(bot.auto_reject_group_invite_reason.clone()),
                auto_reply_poke_enabled: bot.auto_reply_poke_enabled,
                auto_reply_poke_message: normalize_optional_string(bot.auto_reply_poke_message.clone()),
                limiter_config: bot.limiter.clone(),
            })
            .collect::<Vec<_>>();

        let rate_limiters = bots
            .iter()
            .map(|b| TokenBucketLimiter::new(&b.limiter_config))
            .collect();

        let mut interceptor_chain = InterceptorChain::new();
        for interceptor in plugins.interceptors {
            interceptor_chain.add(interceptor);
        }

        Self {
            bots,
            command_plugins: plugins.command_plugins,
            system_plugins: plugins.system_plugins,
            host_plugin_report: std::sync::RwLock::new(None),
            plugin_state_path: Some(config.official_host.plugin_state_path.clone()),
            plugin_bin_dir: Some(config.official_host.plugin_bin_dir.clone()),
            dynamic_runtime: std::sync::Mutex::new(DynamicPluginRuntime::new()),
            interceptor_chain,
            rate_limiters,
            dedup: Arc::new(MessageDedup::new(60, 10000)),
            group_event_filter: Arc::new(GroupEventFilter::disabled()),
            plugin_acl: Arc::new(PluginAclManager::new()),
        }
    }

    pub fn with_host_plugin_report(self, report: HostPluginReport) -> Self {
        *self.host_plugin_report.write().unwrap() = Some(report);
        self
    }

    pub fn bots(&self) -> &[BotRuntimeInfo] {
        &self.bots
    }

    pub async fn boot(&self) -> Result<()> {
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

            if matches!(bot.protocol, ProtocolId::OneBot11)
                && matches!(bot.transport, TransportMode::WsForward)
            {
                let limiter = &self.rate_limiters[idx];
                self.run_onebot11_forward_ws(bot, limiter).await?;
            }
        }
        Ok(())
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
                        let command_descriptions: Vec<String> = entry.commands.iter()
                            .map(|cmd| format!("{}: {}", cmd.name, cmd.description))
                            .collect();
                        let commands: Vec<String> = entry.commands.iter()
                            .map(|cmd| cmd.name.clone())
                            .collect();
                        command_dispatch::PluginStatusEntry {
                            id: entry.plugin_id.clone(),
                            name: entry.plugin_id.clone(),
                            version: entry.plugin_version.clone(),
                            api_version: entry.api_version.clone(),
                            command_descriptions,
                            commands,
                            dynamic: true,
                            enabled: Some(true),
                            callback_symbol: entry.commands.first().map(|c| c.callback_symbol.clone()),
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

        tracing::info!(count = count, "dynamic plugin rescan complete");
        Ok(count)
    }

    async fn run_onebot11_forward_ws(&self, bot: &BotRuntimeInfo, limiter: &TokenBucketLimiter) -> Result<()> {
        let endpoint = bot.endpoint.clone().ok_or_else(|| {
            QimenError::Config(format!("bot '{}' missing ws-forward endpoint", bot.id))
        })?;

        tracing::info!(bot_id = %bot.id, endpoint = %endpoint, "connecting to OneBot11 forward WS");

        let adapter = OneBot11Adapter;
        let mut system_dispatcher = self.build_system_dispatcher();
        let mut command_dispatcher = self.build_command_dispatcher()?;
        let mut command_help_text = render_help_text(
            &command_dispatcher.describe_commands(),
        );
        let reconnect_policy = ReconnectPolicy::default();
        let mut reconnect_delay = reconnect_policy.initial_delay;

        loop {
            let session_started = tokio::time::Instant::now();

            let mut client = match OneBot11ForwardWsClient::connect(&endpoint, bot.access_token.as_deref()).await {
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
                    if let Some(action) = reply_action {
                        if let Err(err) = self.execute_action(bot, &adapter, &client, action).await {
                            tracing::warn!(bot_id = %bot.id, error = %err, "failed to send reload reply");
                        }
                    }
                    // Rebuild dispatchers from updated host_plugin_report
                    system_dispatcher = self.build_system_dispatcher();
                    command_dispatcher = self.build_command_dispatcher()?;
                    command_help_text = render_help_text(
                        &command_dispatcher.describe_commands(),
                    );
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

    async fn run_onebot11_session(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        client: &mut OneBot11ForwardWsClient,
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
                                .handle_onebot11_payload(bot, adapter, system_dispatcher, command_dispatcher, &command_help_text, client, &text, limiter)
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

    async fn handle_onebot11_payload(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        system_dispatcher: &OneBotSystemDispatcher,
        command_dispatcher: &CommandDispatcher,
        command_help_text: &str,
        client: &OneBot11ForwardWsClient,
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
            .dispatch(Self::build_system_event_context(bot, &payload, &runtime_ctx))
            .await
        {
            return Ok(match signal {
                OneBotSystemDispatchSignal::Continue(route) => {
                    let report_guard = self.host_plugin_report.read().unwrap();
                    if let Some(report) = report_guard.as_ref() {
                        if let Some(signal) = self.execute_dynamic_system_route(report, &route, &payload)? {
                            drop(report_guard);
                            self.apply_dynamic_system_signal(bot, adapter, client, &payload, signal)
                                .await?;
                        }
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
            transport_mode: TransportMode::WsForward,
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

        // Extract message_id for dedup
        if let Some(message_id) = event.message_id() {
            let dedup_key = format!("{}:{}", event.bot_instance, message_id);
            if !self.dedup.check_and_mark(&dedup_key).await {
                tracing::debug!(bot_id = %bot.id, message_id = %message_id, "duplicate message skipped by dedup");
                return Ok(SessionSignal::EventHandled);
            }
        }

        // Check group event filter
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

        if !self.interceptor_chain.pre_handle(&bot.id, &event).await {
            tracing::debug!(bot_id = %bot.id, "interceptor chain blocked event");
            return Ok(SessionSignal::EventHandled);
        }

        let permission = PermissionResolver::resolve(bot, &event);
        let is_owner = permission.is_owner;
        let is_admin = permission.is_admin;

        let command_result = command_dispatcher
            .dispatch(&bot.id, &event, &runtime_ctx)
            .with_roles(is_admin, is_owner)
            .with_plugin_acl(&self.plugin_acl)
            .execute()
            .await;

        let reply = match command_result {
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsShow)) => {
                let health = self.dynamic_runtime.lock().map_err(|_| {
                    QimenError::Runtime("dynamic runtime lock poisoned".to_string())
                })?.health_entries();
                // refresh dynamic health into the view layer snapshot on-demand
                let mut snapshot = CommandDispatcher::with_default_handlers();
                snapshot.set_dynamic_status_entries(command_dispatcher.plugin_status_entries());
                snapshot.merge_dynamic_health(&health);
                Some(Message::text(render_plugin_status_text(&snapshot)))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryReport)) => {
                Some(Message::text(render_registry_report(command_dispatcher)))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::RegistryConflicts)) => {
                Some(Message::text(render_registry_conflicts_report(command_dispatcher)))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrors)) => {
                let health = self.dynamic_runtime.lock().map_err(|_| {
                    QimenError::Runtime("dynamic runtime lock poisoned".to_string())
                })?.health_entries();
                Some(Message::text(render_dynamic_errors_report(&health)))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::DynamicErrorsClear)) => {
                self.dynamic_runtime.lock().map_err(|_| {
                    QimenError::Runtime("dynamic runtime lock poisoned".to_string())
                })?.clear_errors();
                Some(Message::text("dynamic runtime error state cleared"))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsEnable { plugin_id })) => {
                Some(Message::text(self.update_plugin_state(plugin_id, true, command_dispatcher)?))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsDisable { plugin_id })) => {
                Some(Message::text(self.update_plugin_state(plugin_id, false, command_dispatcher)?))
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::PluginsReload)) => {
                match self.rescan_dynamic_plugins() {
                    Ok(count) => {
                        let reply_msg = Message::text(format!(
                            "dynamic plugins reloaded: {} plugin(s) discovered, dispatchers will be rebuilt",
                            count
                        ));
                        let reply_action = build_send_msg_action(bot, &event.raw_json, reply_msg).ok();
                        return Ok(SessionSignal::EndSession(SessionEnd::PluginReload {
                            reply_action,
                        }));
                    }
                    Err(err) => {
                        Some(Message::text(format!("plugin reload failed: {err}")))
                    }
                }
            }
            Some(CommandDispatchSignal::Builtin(BuiltinCommandAction::Help)) => {
                Some(Message::text(command_help_text.to_string()))
            }
            Some(CommandDispatchSignal::DynamicCommand { descriptor, args }) => {
                // Check plugin ACL before dispatching to dynamic plugin
                let acl_user_id = event.user_id();
                let acl_group_id = event.group_id_i64();
                if !self.plugin_acl.should_process(&descriptor.plugin_id, acl_user_id, acl_group_id).await {
                    tracing::debug!(plugin_id = %descriptor.plugin_id, "event blocked by plugin ACL");
                    None
                } else {
                let mut runtime = self.dynamic_runtime.lock().map_err(|_| {
                    QimenError::Runtime("dynamic runtime lock poisoned".to_string())
                })?;
                let sender_id = event.user_id().map(|id| id.to_string()).unwrap_or_default();
                let group_id = event.group_id_i64().map(|id| id.to_string()).unwrap_or_default();
                let raw_json = event.raw_json.to_string();
                match runtime.execute_command(&descriptor, &args, &sender_id, &group_id, &raw_json)? {
                    DynamicResponse::ReplyMessage(message) => Some(message),
                    DynamicResponse::Reply(message) => Some(Message::text(message)),
                    DynamicResponse::Ignore => None,
                    DynamicResponse::Approve(reason) => Some(Message::text(
                        reason.unwrap_or_else(|| "dynamic command approved".to_string()),
                    )),
                    DynamicResponse::Reject(reason) => Some(Message::text(
                        reason.unwrap_or_else(|| "dynamic command rejected".to_string()),
                    )),
                }
                }
            }
            Some(CommandDispatchSignal::Reply(reply)) => Some(reply),
            None => None,
        };

        let reply = match event.message.as_ref().map(|message| message.plain_text()) {
            Some(text) if text.trim().eq_ignore_ascii_case("help") || text.trim().eq_ignore_ascii_case("/help") => {
                Some(Message::text(command_help_text.to_string()))
            }
            _ => reply,
        };

        if let Some(reply) = reply {
            let action = build_send_msg_action(bot, &event.raw_json, reply)?;
            self.execute_action(bot, adapter, client, action).await?;
        }

        self.interceptor_chain.after_completion(&bot.id, &event).await;

        Ok(SessionSignal::EventHandled)
    }

    async fn execute_auto_approve_friend(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11ForwardWsClient,
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
        client: &OneBot11ForwardWsClient,
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
        client: &OneBot11ForwardWsClient,
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
        client: &OneBot11ForwardWsClient,
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
        client: &OneBot11ForwardWsClient,
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
            transport_mode: TransportMode::WsForward,
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

    async fn apply_dynamic_system_signal(
        &self,
        bot: &BotRuntimeInfo,
        adapter: &OneBot11Adapter,
        client: &OneBot11ForwardWsClient,
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
                        metadata: ActionMeta { source: "dynamic-system-reply".to_string() },
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
                        metadata: ActionMeta { source: "dynamic-system-reply".to_string() },
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

    fn update_plugin_state(
        &self,
        plugin_id: String,
        enabled: bool,
        command_dispatcher: &CommandDispatcher,
    ) -> Result<String> {
        let Some(path) = &self.plugin_state_path else {
            return Err(QimenError::Runtime("plugin state path not configured".to_string()));
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

    fn execute_dynamic_system_route(
        &self,
        report: &HostPluginReport,
        route: &onebot11_dispatch::OneBotSystemRoute,
        payload: &Value,
    ) -> Result<Option<DynamicResponse>> {
        let mut runtime = self.dynamic_runtime.lock().map_err(|_| {
            QimenError::Runtime("dynamic runtime lock poisoned".to_string())
        })?;

        let raw_json = payload.to_string();

        match route {
            onebot11_dispatch::OneBotSystemRoute::Notice(notice) => {
                let route_label = match notice {
                    onebot11_dispatch::NoticeRoute::GroupPoke => "GroupPoke",
                    onebot11_dispatch::NoticeRoute::PrivatePoke => "PrivatePoke",
                    onebot11_dispatch::NoticeRoute::NotifyLuckyKing => "NotifyLuckyKing",
                    onebot11_dispatch::NoticeRoute::NotifyHonor(_) => "NotifyHonor",
                    _ => return Ok(None),
                };
                // Search in v0.2 routes first, then legacy fields
                if let Some((entry, route_entry)) = find_route_entry(report, "notice", route_label) {
                    let descriptor = DynamicNoticeDescriptor {
                        plugin_id: entry.plugin_id.clone(),
                        notice_route: route_label.to_string(),
                        callback_symbol: route_entry.callback_symbol.clone(),
                        library_path: entry.path.clone(),
                    };
                    return Ok(Some(runtime.execute_notice(&descriptor, &raw_json)?));
                }
                // Legacy fallback
                let Some(entry) = report.dynamic_plugins.iter().find(|entry| entry.notice_route == route_label) else {
                    return Ok(None);
                };
                let descriptor = DynamicNoticeDescriptor {
                    plugin_id: entry.plugin_id.clone(),
                    notice_route: entry.notice_route.clone(),
                    callback_symbol: entry.notice_callback_symbol.clone(),
                    library_path: entry.path.clone(),
                };
                Ok(Some(runtime.execute_notice(&descriptor, &raw_json)?))
            }
            onebot11_dispatch::OneBotSystemRoute::Request(request) => {
                let route_label = match request {
                    onebot11_dispatch::RequestRoute::Friend => "Friend",
                    onebot11_dispatch::RequestRoute::GroupAdd => "GroupAdd",
                    onebot11_dispatch::RequestRoute::GroupInvite => "GroupInvite",
                    _ => return Ok(None),
                };
                if let Some((entry, route_entry)) = find_route_entry(report, "request", route_label) {
                    let descriptor = DynamicRequestDescriptor {
                        plugin_id: entry.plugin_id.clone(),
                        request_route: route_label.to_string(),
                        callback_symbol: route_entry.callback_symbol.clone(),
                        library_path: entry.path.clone(),
                    };
                    return Ok(Some(runtime.execute_request(&descriptor, &raw_json)?));
                }
                let Some(entry) = report.dynamic_plugins.iter().find(|entry| entry.request_route == route_label) else {
                    return Ok(None);
                };
                let descriptor = DynamicRequestDescriptor {
                    plugin_id: entry.plugin_id.clone(),
                    request_route: entry.request_route.clone(),
                    callback_symbol: entry.request_callback_symbol.clone(),
                    library_path: entry.path.clone(),
                };
                Ok(Some(runtime.execute_request(&descriptor, &raw_json)?))
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
                    let descriptor = DynamicMetaDescriptor {
                        plugin_id: entry.plugin_id.clone(),
                        meta_route: route_label.to_string(),
                        callback_symbol: route_entry.callback_symbol.clone(),
                        library_path: entry.path.clone(),
                    };
                    return Ok(Some(runtime.execute_meta(&descriptor, &raw_json)?));
                }
                let Some(entry) = report.dynamic_plugins.iter().find(|entry| entry.meta_route == route_label) else {
                    return Ok(None);
                };
                let descriptor = DynamicMetaDescriptor {
                    plugin_id: entry.plugin_id.clone(),
                    meta_route: entry.meta_route.clone(),
                    callback_symbol: entry.meta_callback_symbol.clone(),
                    library_path: entry.path.clone(),
                };
                Ok(Some(runtime.execute_meta(&descriptor, &raw_json)?))
            }
            onebot11_dispatch::OneBotSystemRoute::MessageSent(_) => {
                // message_sent events are not dispatched to dynamic plugins
                Ok(None)
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
            auto_approve_friend_request_user_whitelist: &bot.auto_approve_friend_request_user_whitelist,
            auto_approve_friend_request_user_blacklist: &bot.auto_approve_friend_request_user_blacklist,
            auto_approve_friend_request_comment_keywords: &bot.auto_approve_friend_request_comment_keywords,
            auto_reject_friend_request_comment_keywords: &bot.auto_reject_friend_request_comment_keywords,
            auto_approve_friend_request_remark: bot.auto_approve_friend_request_remark.as_deref(),
            auto_approve_group_invite_user_whitelist: &bot.auto_approve_group_invite_user_whitelist,
            auto_approve_group_invite_user_blacklist: &bot.auto_approve_group_invite_user_blacklist,
            auto_approve_group_invite_group_whitelist: &bot.auto_approve_group_invite_group_whitelist,
            auto_approve_group_invite_group_blacklist: &bot.auto_approve_group_invite_group_blacklist,
            auto_approve_group_invite_comment_keywords: &bot.auto_approve_group_invite_comment_keywords,
            auto_reject_group_invite_comment_keywords: &bot.auto_reject_group_invite_comment_keywords,
            auto_reject_group_invite_reason: bot.auto_reject_group_invite_reason.as_deref(),
        }
    }
}

/// Find a route entry in the v0.2 routes of dynamic plugins.
fn find_route_entry<'a>(
    report: &'a HostPluginReport,
    kind: &str,
    route_label: &str,
) -> Option<(&'a qimen_host_types::DynamicPluginReportEntry, &'a qimen_host_types::DynamicRouteEntry)> {
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

fn build_send_msg_action(
    bot: &BotRuntimeInfo,
    event: &Value,
    reply: Message,
) -> Result<NormalizedActionRequest> {
    let message = reply.to_onebot_value();

    let mut params = serde_json::Map::new();
    match event.get("message_type").and_then(Value::as_str) {
        Some("private") => {
            let user_id = event
                .get("user_id")
                .cloned()
                .ok_or_else(|| QimenError::Protocol("private message missing user_id".to_string()))?;
            params.insert("message_type".to_string(), Value::String("private".to_string()));
            params.insert("user_id".to_string(), user_id);
        }
        Some("group") => {
            let group_id = event
                .get("group_id")
                .cloned()
                .ok_or_else(|| QimenError::Protocol("group message missing group_id".to_string()))?;
            params.insert("message_type".to_string(), Value::String("group".to_string()));
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

fn build_echo(bot: &BotRuntimeInfo) -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or(0);
    format!("reply-{}-{millis}", bot.id)
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
        params.insert("message_type".to_string(), Value::String("group".to_string()));
        params.insert("group_id".to_string(), group_id);
    } else if let Some(user_id) = payload.get("user_id").cloned() {
        params.insert("message_type".to_string(), Value::String("private".to_string()));
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
        other => TransportMode::Custom(other.to_string()),
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
            "  - key={}\n    incoming={}\n    existing={}"
            ,
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
            entry
                .last_error
                .clone()
                .unwrap_or_else(|| "-".to_string())
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
