use abi_stable_host_api::{ProactiveSendRequest, SendEnqueueStatus};
use async_trait::async_trait;
use qimen_adapter_onebot11::OneBot11Adapter;
use qimen_adapter_qqbot::QqBotAdapter;
use qimen_error::{QimenError, Result};
use qimen_message::Message;
use qimen_protocol_core::{
    ActionMeta, ActionStatus, IncomingPacket, NormalizedActionRequest, NormalizedActionResponse,
    ProtocolAdapter, ProtocolId,
};
use qimen_transport_qqbot::{QqBotOpenApiClient, SendMessagePayload, UploadFilePayload};
use qimen_transport_ws::OneBot11WsActionSender;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, mpsc};

use crate::{BotRuntimeInfo, build_echo, upload_qqbot_media, value_to_optional_string};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProactiveSendSettings {
    pub queue_capacity: usize,
    pub offline_ttl: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedProactiveSendRequest {
    pub schema_version: u32,
    pub bot_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub context: Value,
    pub message: String,
    pub segments: Option<Value>,
    pub options: Value,
    queued_at: Instant,
}

#[derive(Clone)]
pub struct ProactiveSendHub {
    inner: Arc<ProactiveSendHubInner>,
}

struct ProactiveSendHubInner {
    settings: ProactiveSendSettings,
    states: HashMap<String, Arc<BotQueueState>>,
    disabled_bots: HashSet<String>,
    shutting_down: AtomicBool,
    active_workers: AtomicUsize,
    workers_stopped: Notify,
}

struct BotQueueState {
    bot: BotRuntimeInfo,
    sender: mpsc::Sender<OwnedProactiveSendRequest>,
    receiver: std::sync::Mutex<Option<mpsc::Receiver<OwnedProactiveSendRequest>>>,
    executor: Mutex<Option<RegisteredExecutor>>,
    notify: Notify,
    next_registration_id: AtomicU64,
}

#[derive(Clone)]
struct RegisteredExecutor {
    id: u64,
    executor: Arc<dyn ProactiveActionExecutor>,
}

#[async_trait]
trait ProactiveActionExecutor: Send + Sync {
    async fn execute(&self, request: OwnedProactiveSendRequest)
    -> Result<NormalizedActionResponse>;
}

impl ProactiveSendHub {
    pub fn new(bots: &[BotRuntimeInfo], settings: ProactiveSendSettings) -> Self {
        let mut states = HashMap::new();
        let mut disabled_bots = HashSet::new();
        let capacity = settings.queue_capacity.max(1);

        for bot in bots {
            if bot.enabled {
                let (sender, receiver) = mpsc::channel(capacity);
                states.insert(
                    bot.id.clone(),
                    Arc::new(BotQueueState {
                        bot: bot.clone(),
                        sender,
                        receiver: std::sync::Mutex::new(Some(receiver)),
                        executor: Mutex::new(None),
                        notify: Notify::new(),
                        next_registration_id: AtomicU64::new(1),
                    }),
                );
            } else {
                disabled_bots.insert(bot.id.clone());
            }
        }

        Self {
            inner: Arc::new(ProactiveSendHubInner {
                settings,
                states,
                disabled_bots,
                shutting_down: AtomicBool::new(false),
                active_workers: AtomicUsize::new(0),
                workers_stopped: Notify::new(),
            }),
        }
    }

    pub fn start_workers(&self) {
        for state in self.inner.states.values() {
            let receiver = match state.receiver.lock() {
                Ok(mut guard) => guard.take(),
                Err(_) => None,
            };
            if let Some(receiver) = receiver {
                let hub = self.clone();
                let state = Arc::clone(state);
                self.inner.active_workers.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    hub.run_bot_worker(state, receiver).await;
                    if hub.inner.active_workers.fetch_sub(1, Ordering::SeqCst) == 1 {
                        hub.inner.workers_stopped.notify_waiters();
                    }
                });
            }
        }
    }

    pub fn shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
        for state in self.inner.states.values() {
            state.notify.notify_waiters();
        }
    }

    /// 停止接收新请求，并等待所有 Bot 顺序 worker 完成当前发送、丢弃未开始请求后退出。
    pub async fn shutdown_and_wait(&self) {
        self.shutdown();
        loop {
            let stopped = self.inner.workers_stopped.notified();
            tokio::pin!(stopped);
            stopped.as_mut().enable();
            if self.inner.active_workers.load(Ordering::SeqCst) == 0 {
                break;
            }
            stopped.await;
        }
    }

    pub fn enqueue_from_ffi(&self, request: &ProactiveSendRequest) -> SendEnqueueStatus {
        if self.inner.shutting_down.load(Ordering::SeqCst) {
            return SendEnqueueStatus::HostShuttingDown;
        }

        let request = match OwnedProactiveSendRequest::from_ffi(request) {
            Ok(request) => request,
            Err(status) => return status,
        };

        let state = match self.inner.states.get(&request.bot_id) {
            Some(state) => state,
            None => {
                return if self.inner.disabled_bots.contains(&request.bot_id) {
                    SendEnqueueStatus::BotDisabled
                } else {
                    SendEnqueueStatus::BotNotFound
                };
            }
        };

        match state.sender.try_send(request) {
            Ok(()) => SendEnqueueStatus::Accepted,
            Err(mpsc::error::TrySendError::Full(_)) => SendEnqueueStatus::QueueFull,
            Err(mpsc::error::TrySendError::Closed(_)) => SendEnqueueStatus::HostShuttingDown,
        }
    }

    pub async fn register_onebot11_executor(
        &self,
        bot: &BotRuntimeInfo,
        sender: OneBot11WsActionSender,
    ) -> Option<u64> {
        let state = self.inner.states.get(&bot.id)?;
        let registration_id = state.next_registration_id.fetch_add(1, Ordering::SeqCst);
        let executor = Arc::new(OneBot11ProactiveExecutor {
            bot: bot.clone(),
            sender,
        });
        *state.executor.lock().await = Some(RegisteredExecutor {
            id: registration_id,
            executor,
        });
        state.notify.notify_waiters();
        Some(registration_id)
    }

    pub async fn register_qq_official_executor(
        &self,
        bot: &BotRuntimeInfo,
        client: Arc<QqBotOpenApiClient>,
    ) -> Option<u64> {
        let state = self.inner.states.get(&bot.id)?;
        let registration_id = state.next_registration_id.fetch_add(1, Ordering::SeqCst);
        let executor = Arc::new(QqOfficialProactiveExecutor {
            bot: bot.clone(),
            client,
        });
        *state.executor.lock().await = Some(RegisteredExecutor {
            id: registration_id,
            executor,
        });
        state.notify.notify_waiters();
        Some(registration_id)
    }

    pub async fn unregister_executor(&self, bot_id: &str, registration_id: u64) {
        let Some(state) = self.inner.states.get(bot_id) else {
            return;
        };
        let mut executor = state.executor.lock().await;
        if executor
            .as_ref()
            .is_some_and(|registered| registered.id == registration_id)
        {
            *executor = None;
        }
        state.notify.notify_waiters();
    }

    async fn run_bot_worker(
        &self,
        state: Arc<BotQueueState>,
        mut receiver: mpsc::Receiver<OwnedProactiveSendRequest>,
    ) {
        loop {
            if self.inner.shutting_down.load(Ordering::SeqCst) {
                receiver.close();
                while let Ok(request) = receiver.try_recv() {
                    tracing::warn!(
                        bot_id = %state.bot.id,
                        target_kind = %request.target_kind,
                        target_id = %request.target_id,
                        "dropping queued proactive send during runtime shutdown"
                    );
                }
                break;
            }

            // 先启用通知 future，再复查关闭状态，避免 shutdown 的唤醒落在等待窗口之间。
            let notified = state.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if self.inner.shutting_down.load(Ordering::SeqCst) {
                continue;
            }

            let request = tokio::select! {
                request = receiver.recv() => {
                    let Some(request) = request else {
                        break;
                    };
                    request
                }
                _ = &mut notified => {
                    continue;
                }
            };

            if self.inner.shutting_down.load(Ordering::SeqCst) {
                tracing::warn!(
                    bot_id = %state.bot.id,
                    target_kind = %request.target_kind,
                    target_id = %request.target_id,
                    "dropping proactive send during runtime shutdown"
                );
                continue;
            }

            let executor = match self.wait_for_executor(&state, request.queued_at).await {
                Some(executor) => executor,
                None => {
                    tracing::warn!(
                        bot_id = %state.bot.id,
                        target_kind = %request.target_kind,
                        target_id = %request.target_id,
                        offline_ttl_ms = self.inner.settings.offline_ttl.as_millis(),
                        "dropping proactive send because bot has no online executor"
                    );
                    continue;
                }
            };

            let action = request.target_kind.clone();
            let target_id = request.target_id.clone();
            match executor.execute(request).await {
                Ok(response) => {
                    tracing::info!(
                        bot_id = %state.bot.id,
                        target_kind = %action,
                        target_id = %target_id,
                        status = ?response.status,
                        retcode = response.retcode,
                        "executed proactive send"
                    );
                }
                Err(err) => {
                    tracing::warn!(
                        bot_id = %state.bot.id,
                        target_kind = %action,
                        target_id = %target_id,
                        error = %err,
                        "proactive send failed"
                    );
                }
            }
        }
    }

    async fn wait_for_executor(
        &self,
        state: &Arc<BotQueueState>,
        queued_at: Instant,
    ) -> Option<Arc<dyn ProactiveActionExecutor>> {
        loop {
            // Notify 的广播唤醒不保存 permit；先 enable 再检查状态可消除解绑/关闭竞态。
            let notified = state.notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            if self.inner.shutting_down.load(Ordering::SeqCst) {
                return None;
            }

            if let Some(registered) = state.executor.lock().await.as_ref().cloned() {
                return Some(registered.executor);
            }

            let ttl = self.inner.settings.offline_ttl;
            if ttl.is_zero() {
                return None;
            }
            let elapsed = queued_at.elapsed();
            if elapsed >= ttl {
                return None;
            }

            let remaining = ttl.saturating_sub(elapsed);
            if tokio::time::timeout(remaining, &mut notified)
                .await
                .is_err()
            {
                return None;
            }
        }
    }
}

impl OwnedProactiveSendRequest {
    fn from_ffi(request: &ProactiveSendRequest) -> std::result::Result<Self, SendEnqueueStatus> {
        if request.schema_version != abi_stable_host_api::PROACTIVE_SEND_SCHEMA_VERSION {
            return Err(SendEnqueueStatus::InvalidRequest);
        }

        let bot_id = request.bot_id.as_str().trim().to_string();
        let target_kind = request.target_kind.as_str().trim().to_string();
        let target_id = request.target_id.as_str().trim().to_string();
        let message = request.message.as_str().to_string();
        let segments_json = request.segments_json.as_str().trim();
        if bot_id.is_empty()
            || target_id.is_empty()
            || !matches!(
                target_kind.as_str(),
                "private" | "group" | "channel" | "channel_private"
            )
            || (message.is_empty() && segments_json.is_empty())
        {
            return Err(SendEnqueueStatus::InvalidRequest);
        }

        let context = parse_optional_json_object(request.context_json.as_str())?;
        let options = parse_optional_json_object(request.options_json.as_str())?;
        let segments = if segments_json.is_empty() {
            None
        } else {
            Some(
                serde_json::from_str::<Value>(segments_json)
                    .map_err(|_| SendEnqueueStatus::InvalidRequest)?,
            )
        };

        Ok(Self {
            schema_version: request.schema_version,
            bot_id,
            target_kind,
            target_id,
            context,
            message,
            segments,
            options,
            queued_at: Instant::now(),
        })
    }

    fn message_value(&self) -> Value {
        if let Some(segments) = self.segments.as_ref() {
            Message::from_onebot_value(segments).to_onebot_value()
        } else {
            Message::text(self.message.clone()).to_onebot_value()
        }
    }
}

fn parse_optional_json_object(value: &str) -> std::result::Result<Value, SendEnqueueStatus> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(json!({}));
    }
    let parsed =
        serde_json::from_str::<Value>(trimmed).map_err(|_| SendEnqueueStatus::InvalidRequest)?;
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err(SendEnqueueStatus::InvalidRequest)
    }
}

struct OneBot11ProactiveExecutor {
    bot: BotRuntimeInfo,
    sender: OneBot11WsActionSender,
}

#[async_trait]
impl ProactiveActionExecutor for OneBot11ProactiveExecutor {
    async fn execute(
        &self,
        request: OwnedProactiveSendRequest,
    ) -> Result<NormalizedActionResponse> {
        let adapter = OneBot11Adapter;
        let action = build_onebot11_action(&self.bot, request)?;
        let echo = action
            .echo
            .as_ref()
            .and_then(Value::as_str)
            .ok_or_else(|| QimenError::Runtime("proactive action echo missing".to_string()))?
            .to_string();
        let packet = adapter.encode_action(&action).await?;
        let serialized = serde_json::to_string(&packet.payload)?;
        let raw_response = self
            .sender
            .send_text_await_echo(&serialized, &echo, Duration::from_millis(action.timeout_ms))
            .await?;
        let response_packet = IncomingPacket {
            protocol: ProtocolId::OneBot11,
            transport_mode: self.bot.transport.clone(),
            bot_instance: self.bot.id.clone(),
            payload: serde_json::from_str(&raw_response)?,
            raw_bytes: None,
        };
        adapter.decode_action_response(response_packet).await
    }
}

struct QqOfficialProactiveExecutor {
    bot: BotRuntimeInfo,
    client: Arc<QqBotOpenApiClient>,
}

#[async_trait]
impl ProactiveActionExecutor for QqOfficialProactiveExecutor {
    async fn execute(
        &self,
        request: OwnedProactiveSendRequest,
    ) -> Result<NormalizedActionResponse> {
        let adapter = QqBotAdapter;
        let action = build_qq_official_action(&self.bot, request)?;
        let packet = adapter.encode_action(&action).await?;
        let route = packet
            .payload
            .get("route")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                QimenError::Protocol("qqbot action payload missing route".to_string())
            })?;

        if let Some(segments) = packet
            .payload
            .get("unsupported_segments")
            .and_then(Value::as_array)
            && !segments.is_empty()
        {
            tracing::warn!(
                bot_id = %self.bot.id,
                route,
                unsupported_segments = ?segments,
                "QQ official proactive send degraded unsupported message segments"
            );
        }

        let data = execute_qq_official_packet(&self.client, route, &packet.payload).await?;
        Ok(NormalizedActionResponse {
            protocol: ProtocolId::QqOfficial,
            bot_instance: self.bot.id.clone(),
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
        })
    }
}

fn build_onebot11_action(
    bot: &BotRuntimeInfo,
    request: OwnedProactiveSendRequest,
) -> Result<NormalizedActionRequest> {
    let mut params = Map::new();
    let action = match request.target_kind.as_str() {
        "private" => {
            params.insert("user_id".to_string(), id_value(&request.target_id));
            "send_private_msg"
        }
        "group" => {
            params.insert("group_id".to_string(), id_value(&request.target_id));
            "send_group_msg"
        }
        "channel" => {
            let guild_id = request
                .context
                .get("guild_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol(
                        "onebot proactive channel send requires context.guild_id".to_string(),
                    )
                })?;
            params.insert("guild_id".to_string(), id_value(&guild_id));
            params.insert("channel_id".to_string(), id_value(&request.target_id));
            "send_guild_channel_msg"
        }
        "channel_private" => {
            let guild_id = request
                .context
                .get("guild_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol(
                        "onebot proactive channel private send requires context.guild_id"
                            .to_string(),
                    )
                })?;
            params.insert("guild_id".to_string(), id_value(&guild_id));
            params.insert("user_id".to_string(), id_value(&request.target_id));
            "send_guild_private_msg"
        }
        other => {
            return Err(QimenError::Protocol(format!(
                "unsupported proactive target kind '{other}'"
            )));
        }
    };

    params.insert("message".to_string(), request.message_value());
    params.insert("auto_escape".to_string(), Value::Bool(false));
    merge_options(&mut params, &request.options);

    Ok(normalized_action(
        bot,
        ProtocolId::OneBot11,
        action,
        Value::Object(params),
    ))
}

fn build_qq_official_action(
    bot: &BotRuntimeInfo,
    request: OwnedProactiveSendRequest,
) -> Result<NormalizedActionRequest> {
    let mut params = Map::new();
    let action = match request.target_kind.as_str() {
        "private" => {
            params.insert(
                "openid".to_string(),
                Value::String(request.target_id.clone()),
            );
            "send_private_msg"
        }
        "group" => {
            params.insert(
                "group_openid".to_string(),
                Value::String(request.target_id.clone()),
            );
            "send_group_msg"
        }
        "channel" => {
            params.insert(
                "channel_id".to_string(),
                Value::String(request.target_id.clone()),
            );
            if let Some(guild_id) = request.context.get("guild_id").cloned() {
                params.insert("guild_id".to_string(), guild_id);
            }
            "send_channel_msg"
        }
        "channel_private" => {
            params.insert(
                "guild_id".to_string(),
                Value::String(request.target_id.clone()),
            );
            "send_dms"
        }
        other => {
            return Err(QimenError::Protocol(format!(
                "unsupported proactive target kind '{other}'"
            )));
        }
    };

    params.insert("message".to_string(), request.message_value());
    merge_options(&mut params, &request.options);

    Ok(normalized_action(
        bot,
        ProtocolId::QqOfficial,
        action,
        Value::Object(params),
    ))
}

fn normalized_action(
    bot: &BotRuntimeInfo,
    protocol: ProtocolId,
    action: &str,
    params: Value,
) -> NormalizedActionRequest {
    NormalizedActionRequest {
        protocol,
        bot_instance: bot.id.clone(),
        action: action.to_string(),
        params,
        echo: Some(json!(build_echo(bot))),
        timeout_ms: 5000,
        metadata: ActionMeta {
            source: "dynamic-plugin-proactive-send".to_string(),
        },
    }
}

fn merge_options(params: &mut Map<String, Value>, options: &Value) {
    let Some(options) = options.as_object() else {
        return;
    };
    for (key, value) in options {
        params.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

fn id_value(value: &str) -> Value {
    value
        .parse::<i64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(value.to_string()))
}

async fn execute_qq_official_packet(
    client: &QqBotOpenApiClient,
    route: &str,
    payload: &Value,
) -> Result<Value> {
    if matches!(route, "group_file" | "c2c_file") {
        let file_payload = UploadFilePayload {
            file_type: payload
                .get("file_type")
                .and_then(Value::as_i64)
                .unwrap_or(1),
            url: payload
                .get("url")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot upload media missing url".to_string())
                })?,
            srv_send_msg: payload
                .get("srv_send_msg")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        };
        return match route {
            "group_file" => {
                let group_openid = payload
                    .get("group_openid")
                    .and_then(value_to_optional_string)
                    .ok_or_else(|| {
                        QimenError::Protocol("qqbot group_file missing group_openid".to_string())
                    })?;
                client.post_group_file(&group_openid, &file_payload).await
            }
            "c2c_file" => {
                let openid = payload
                    .get("openid")
                    .and_then(value_to_optional_string)
                    .ok_or_else(|| {
                        QimenError::Protocol("qqbot c2c_file missing openid".to_string())
                    })?;
                client.post_c2c_file(&openid, &file_payload).await
            }
            _ => unreachable!(),
        };
    }

    let media_upload = payload.get("media_upload").cloned();
    let mut message_payload = SendMessagePayload {
        msg_type: payload.get("msg_type").and_then(Value::as_i64),
        content: payload.get("content").and_then(value_to_optional_string),
        msg_id: payload.get("msg_id").and_then(value_to_optional_string),
        msg_seq: payload.get("msg_seq").and_then(Value::as_i64),
        event_id: payload.get("event_id").and_then(value_to_optional_string),
        markdown: payload.get("markdown").cloned(),
        keyboard: payload.get("keyboard").cloned(),
        ark: payload.get("ark").cloned(),
        embed: payload.get("embed").cloned(),
        media: payload.get("media").cloned(),
        image: payload.get("image").and_then(value_to_optional_string),
    };

    match route {
        "channel_message" => {
            let channel_id = payload
                .get("channel_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot channel_message missing channel_id".to_string())
                })?;
            client
                .post_channel_message(&channel_id, &message_payload)
                .await
        }
        "group_message" => {
            let group_openid = payload
                .get("group_openid")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot group_message missing group_openid".to_string())
                })?;
            if let Some(upload) = media_upload.as_ref()
                && message_payload.media.is_none()
            {
                message_payload.media =
                    Some(upload_qqbot_media(client, "group_file", &group_openid, upload).await?);
                message_payload.msg_type = Some(7);
                message_payload.image = None;
            }
            client
                .post_group_message(&group_openid, &message_payload)
                .await
        }
        "c2c_message" => {
            let openid = payload
                .get("openid")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot c2c_message missing openid".to_string())
                })?;
            if let Some(upload) = media_upload.as_ref()
                && message_payload.media.is_none()
            {
                message_payload.media =
                    Some(upload_qqbot_media(client, "c2c_file", &openid, upload).await?);
                message_payload.msg_type = Some(7);
                message_payload.image = None;
            }
            client.post_c2c_message(&openid, &message_payload).await
        }
        "dms_message" => {
            let guild_id = payload
                .get("guild_id")
                .and_then(value_to_optional_string)
                .ok_or_else(|| {
                    QimenError::Protocol("qqbot dms_message missing guild_id".to_string())
                })?;
            client.post_dms_message(&guild_id, &message_payload).await
        }
        other => Err(QimenError::Protocol(format!(
            "unsupported qqbot proactive route '{other}'"
        ))),
    }
}

pub(crate) struct ProactiveHostContext {
    hub: ProactiveSendHub,
}

impl ProactiveHostContext {
    pub(crate) fn new(hub: ProactiveSendHub) -> Self {
        Self { hub }
    }

    pub(crate) fn as_context_ptr(&mut self) -> *mut c_void {
        (self as *mut Self).cast::<c_void>()
    }
}

pub(crate) unsafe extern "C" fn host_enqueue_send(
    context: *mut c_void,
    request: *const ProactiveSendRequest,
) -> i32 {
    if context.is_null() || request.is_null() {
        return SendEnqueueStatus::InvalidRequest.code();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // SAFETY: DynamicPluginRuntime owns this context until Host API unbind returns.
        let context = unsafe { &*(context.cast::<ProactiveHostContext>()) };
        // SAFETY: The plugin-side submit call passes a valid pointer for this callback.
        let request = unsafe { &*request };
        context.hub.enqueue_from_ffi(request).code()
    }));

    result.unwrap_or_else(|_| SendEnqueueStatus::HostUnavailable.code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RString;
    use qimen_plugin_api::RateLimiterConfig;
    use qimen_protocol_core::{CapabilitySet, TransportMode};
    use std::ptr;

    struct RecordingExecutor {
        tx: mpsc::UnboundedSender<(String, String)>,
    }

    #[async_trait]
    impl ProactiveActionExecutor for RecordingExecutor {
        async fn execute(
            &self,
            request: OwnedProactiveSendRequest,
        ) -> Result<NormalizedActionResponse> {
            let bot_id = request.bot_id.clone();
            let target_id = request.target_id.clone();
            let _ = self.tx.send((bot_id.clone(), target_id));
            Ok(NormalizedActionResponse {
                protocol: ProtocolId::OneBot11,
                bot_instance: bot_id,
                status: ActionStatus::Ok,
                retcode: 0,
                data: Value::Null,
                echo: None,
                latency_ms: 0,
                raw_json: Value::Null,
            })
        }
    }

    async fn install_recording_executor(
        hub: &ProactiveSendHub,
        bot_id: &str,
    ) -> mpsc::UnboundedReceiver<(String, String)> {
        let state = hub.inner.states.get(bot_id).unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let registration_id = state.next_registration_id.fetch_add(1, Ordering::SeqCst);
        *state.executor.lock().await = Some(RegisteredExecutor {
            id: registration_id,
            executor: Arc::new(RecordingExecutor { tx }),
        });
        state.notify.notify_waiters();
        rx
    }

    fn test_bot(id: &str, enabled: bool, protocol: ProtocolId) -> BotRuntimeInfo {
        BotRuntimeInfo {
            id: id.to_string(),
            protocol,
            transport: TransportMode::WsReverse,
            capabilities: CapabilitySet::default(),
            endpoint: None,
            bind: None,
            path: None,
            access_token: None,
            appid: None,
            secret: None,
            intents: Vec::new(),
            sandbox: false,
            enabled,
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
            limiter_config: RateLimiterConfig::default(),
        }
    }

    fn text_request(bot_id: &str, target_kind: &str, target_id: &str) -> ProactiveSendRequest {
        let mut request = ProactiveSendRequest::new(bot_id, target_kind, target_id);
        request.message = RString::from("hello");
        request
    }

    #[test]
    fn ffi_request_requires_valid_json_objects() {
        let mut request = text_request("bot-a", "group", "42");
        request.context_json = RString::from(r#"{"guild_id":"guild-a"}"#);
        request.options_json = RString::from(r#"{"msg_id":"m1"}"#);

        let owned = OwnedProactiveSendRequest::from_ffi(&request).unwrap();
        assert_eq!(owned.bot_id, "bot-a");
        assert_eq!(owned.context["guild_id"], "guild-a");
        assert_eq!(owned.options["msg_id"], "m1");

        request.context_json = RString::from("[]");
        assert_eq!(
            OwnedProactiveSendRequest::from_ffi(&request),
            Err(SendEnqueueStatus::InvalidRequest)
        );
    }

    #[test]
    fn enqueue_reports_bot_and_capacity_statuses() {
        let enabled = test_bot("bot-a", true, ProtocolId::OneBot11);
        let disabled = test_bot("bot-b", false, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[enabled, disabled],
            ProactiveSendSettings {
                queue_capacity: 1,
                offline_ttl: Duration::ZERO,
            },
        );

        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "1")),
            SendEnqueueStatus::Accepted
        );
        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "2")),
            SendEnqueueStatus::QueueFull
        );
        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-b", "group", "1")),
            SendEnqueueStatus::BotDisabled
        );
        assert_eq!(
            hub.enqueue_from_ffi(&text_request("missing", "group", "1")),
            SendEnqueueStatus::BotNotFound
        );
    }

    #[test]
    fn onebot_channel_actions_require_guild_context() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);
        let request =
            OwnedProactiveSendRequest::from_ffi(&text_request("bot-a", "channel", "100")).unwrap();
        assert!(build_onebot11_action(&bot, request).is_err());

        let mut request = text_request("bot-a", "channel", "100");
        request.context_json = RString::from(r#"{"guild_id":"200"}"#);
        request.options_json = RString::from(r#"{"extra":"ok","message":"ignored"}"#);
        let action =
            build_onebot11_action(&bot, OwnedProactiveSendRequest::from_ffi(&request).unwrap())
                .unwrap();

        assert_eq!(action.action, "send_guild_channel_msg");
        assert_eq!(action.params["guild_id"], 200);
        assert_eq!(action.params["channel_id"], 100);
        assert_eq!(action.params["extra"], "ok");
        assert!(!action.params["message"].is_null());
    }

    #[test]
    fn qq_official_targets_map_to_send_actions() {
        let bot = test_bot("bot-a", true, ProtocolId::QqOfficial);
        let mut request = text_request("bot-a", "channel_private", "guild-a");
        request.options_json = RString::from(r#"{"msg_type":0}"#);

        let action =
            build_qq_official_action(&bot, OwnedProactiveSendRequest::from_ffi(&request).unwrap())
                .unwrap();

        assert_eq!(action.protocol, ProtocolId::QqOfficial);
        assert_eq!(action.action, "send_dms");
        assert_eq!(action.params["guild_id"], "guild-a");
        assert_eq!(action.params["msg_type"], 0);
    }

    #[test]
    fn onebot_private_group_and_channel_private_targets_map_to_actions() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);

        let private = build_onebot11_action(
            &bot,
            OwnedProactiveSendRequest::from_ffi(&text_request("bot-a", "private", "10")).unwrap(),
        )
        .unwrap();
        assert_eq!(private.action, "send_private_msg");
        assert_eq!(private.params["user_id"], 10);

        let group = build_onebot11_action(
            &bot,
            OwnedProactiveSendRequest::from_ffi(&text_request("bot-a", "group", "20")).unwrap(),
        )
        .unwrap();
        assert_eq!(group.action, "send_group_msg");
        assert_eq!(group.params["group_id"], 20);

        let mut channel_private = text_request("bot-a", "channel_private", "30");
        channel_private.context_json = RString::from(r#"{"guild_id":"40"}"#);
        let channel_private = build_onebot11_action(
            &bot,
            OwnedProactiveSendRequest::from_ffi(&channel_private).unwrap(),
        )
        .unwrap();
        assert_eq!(channel_private.action, "send_guild_private_msg");
        assert_eq!(channel_private.params["guild_id"], 40);
        assert_eq!(channel_private.params["user_id"], 30);
    }

    #[test]
    fn qq_official_private_group_and_channel_targets_map_to_actions() {
        let bot = test_bot("bot-a", true, ProtocolId::QqOfficial);

        let private = build_qq_official_action(
            &bot,
            OwnedProactiveSendRequest::from_ffi(&text_request("bot-a", "private", "user-a"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(private.action, "send_private_msg");
        assert_eq!(private.params["openid"], "user-a");

        let group = build_qq_official_action(
            &bot,
            OwnedProactiveSendRequest::from_ffi(&text_request("bot-a", "group", "group-a"))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(group.action, "send_group_msg");
        assert_eq!(group.params["group_openid"], "group-a");

        let mut channel = text_request("bot-a", "channel", "channel-a");
        channel.context_json = RString::from(r#"{"guild_id":"guild-a"}"#);
        let channel =
            build_qq_official_action(&bot, OwnedProactiveSendRequest::from_ffi(&channel).unwrap())
                .unwrap();
        assert_eq!(channel.action, "send_channel_msg");
        assert_eq!(channel.params["channel_id"], "channel-a");
        assert_eq!(channel.params["guild_id"], "guild-a");
    }

    #[test]
    fn host_callback_rejects_null_pointers() {
        let status = unsafe { host_enqueue_send(ptr::null_mut(), ptr::null()) };
        assert_eq!(
            SendEnqueueStatus::from_code(status),
            SendEnqueueStatus::InvalidRequest
        );
    }

    #[tokio::test]
    async fn offline_request_runs_when_executor_connects_within_ttl() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[bot],
            ProactiveSendSettings {
                queue_capacity: 4,
                offline_ttl: Duration::from_secs(1),
            },
        );
        hub.start_workers();

        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "42")),
            SendEnqueueStatus::Accepted
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let mut received = install_recording_executor(&hub, "bot-a").await;

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), received.recv())
                .await
                .unwrap(),
            Some(("bot-a".to_string(), "42".to_string()))
        );
        hub.shutdown_and_wait().await;
    }

    #[tokio::test]
    async fn zero_ttl_drops_request_without_online_executor() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[bot],
            ProactiveSendSettings {
                queue_capacity: 4,
                offline_ttl: Duration::ZERO,
            },
        );
        hub.start_workers();

        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "42")),
            SendEnqueueStatus::Accepted
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
        let mut received = install_recording_executor(&hub, "bot-a").await;

        assert!(
            tokio::time::timeout(Duration::from_millis(30), received.recv())
                .await
                .is_err()
        );
        hub.shutdown_and_wait().await;
    }

    #[tokio::test]
    async fn bot_queues_keep_executors_isolated() {
        let bot_a = test_bot("bot-a", true, ProtocolId::OneBot11);
        let bot_b = test_bot("bot-b", true, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[bot_a, bot_b],
            ProactiveSendSettings {
                queue_capacity: 4,
                offline_ttl: Duration::from_secs(1),
            },
        );
        let mut received_a = install_recording_executor(&hub, "bot-a").await;
        let mut received_b = install_recording_executor(&hub, "bot-b").await;
        hub.start_workers();

        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-b", "group", "same-target")),
            SendEnqueueStatus::Accepted
        );
        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "same-target")),
            SendEnqueueStatus::Accepted
        );

        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), received_a.recv())
                .await
                .unwrap(),
            Some(("bot-a".to_string(), "same-target".to_string()))
        );
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), received_b.recv())
                .await
                .unwrap(),
            Some(("bot-b".to_string(), "same-target".to_string()))
        );
        hub.shutdown_and_wait().await;
    }

    #[tokio::test]
    async fn shutdown_rejects_new_requests_and_wakes_idle_workers() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[bot],
            ProactiveSendSettings {
                queue_capacity: 4,
                offline_ttl: Duration::from_secs(60),
            },
        );
        hub.start_workers();

        tokio::time::timeout(Duration::from_secs(1), hub.shutdown_and_wait())
            .await
            .expect("idle proactive worker should stop immediately");
        assert_eq!(
            hub.enqueue_from_ffi(&text_request("bot-a", "group", "42")),
            SendEnqueueStatus::HostShuttingDown
        );
    }

    #[test]
    fn concurrent_enqueue_keeps_requests_host_owned_and_isolated() {
        let bot = test_bot("bot-a", true, ProtocolId::OneBot11);
        let hub = ProactiveSendHub::new(
            &[bot],
            ProactiveSendSettings {
                queue_capacity: 64,
                offline_ttl: Duration::from_secs(1),
            },
        );
        let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        std::thread::scope(|scope| {
            for index in 0..32 {
                let hub = hub.clone();
                let accepted = Arc::clone(&accepted);
                scope.spawn(move || {
                    let request = text_request("bot-a", "group", &index.to_string());
                    if hub.enqueue_from_ffi(&request) == SendEnqueueStatus::Accepted {
                        accepted.fetch_add(1, Ordering::SeqCst);
                    }
                });
            }
        });

        assert_eq!(accepted.load(Ordering::SeqCst), 32);
    }
}
