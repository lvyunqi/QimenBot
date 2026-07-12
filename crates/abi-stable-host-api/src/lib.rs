//! ABI-stable types for the dynamic plugin FFI boundary.
//!
//! All structs are `#[repr(C)]` and use [`abi_stable::std_types`]
//! so they can be safely passed across `dlopen`-loaded library boundaries.
//!
//! ## API Version History
//!
//! - **0.1** — Initial version: single command per plugin, plain-text responses.
//! - **0.2** — Multi-command support via `RVec<CommandDescriptorEntry>`,
//!   rich-media responses via JSON segments, event context in requests.
//! - **0.3** — Extended `CommandRequest` with sender nickname, message ID, and
//!   timestamp. Added `ReplyBuilder` for fluent rich-media construction.
//!   Added `PluginInitConfig` / `PluginInitResult` lifecycle hooks.

use abi_stable::std_types::{RString, RVec};
use std::{
    ffi::c_void,
    sync::{Mutex, RwLock},
};

/// Current plugin API version. Dynamic plugins must declare the same version
/// to be loaded by the host.
pub fn expected_api_version() -> RString {
    RString::from("0.4")
}

/// Also accept legacy 0.1-0.3 plugins for backward compatibility.
pub fn is_compatible_api_version(version: &str) -> bool {
    matches!(version, "0.1" | "0.2" | "0.3" | "0.4")
}

// ─── Action constants ───────────────────────────────────────────────────

pub const ACTION_IGNORE: i32 = 0;
pub const ACTION_REPLY: i32 = 1;
pub const ACTION_APPROVE: i32 = 2;
pub const ACTION_REJECT: i32 = 3;

// ─── Shared response ───────────────────────────────────────────────────

/// The action portion of every dynamic plugin response.
#[repr(C)]
#[derive(Clone)]
pub struct DynamicActionResponse {
    /// One of the `ACTION_*` constants.
    pub action_kind: i32,
    /// Plain text message (for backward compatibility or simple replies).
    pub message: RString,
    /// JSON-encoded array of message segments for rich-media responses.
    /// Example: `[{"type":"text","data":{"text":"hello"}},{"type":"face","data":{"id":"1"}}]`
    /// When non-empty, this takes precedence over `message`.
    pub segments_json: RString,
}

impl DynamicActionResponse {
    /// Create a simple text reply response.
    pub fn text_reply(text: &str) -> Self {
        Self {
            action_kind: ACTION_REPLY,
            message: RString::from(text),
            segments_json: RString::new(),
        }
    }

    /// Create a rich-media reply response with JSON segments.
    pub fn rich_reply(segments_json: &str) -> Self {
        Self {
            action_kind: ACTION_REPLY,
            message: RString::new(),
            segments_json: RString::from(segments_json),
        }
    }

    /// Create an ignore response.
    pub fn ignore() -> Self {
        Self {
            action_kind: ACTION_IGNORE,
            message: RString::new(),
            segments_json: RString::new(),
        }
    }

    /// Create an approve response (for friend/group requests).
    pub fn approve(remark: &str) -> Self {
        Self {
            action_kind: ACTION_APPROVE,
            message: RString::from(remark),
            segments_json: RString::new(),
        }
    }

    /// Create a reject response (for friend/group requests).
    pub fn reject(reason: &str) -> Self {
        Self {
            action_kind: ACTION_REJECT,
            message: RString::from(reason),
            segments_json: RString::new(),
        }
    }
}

// ─── Command FFI types ──────────────────────────────────────────────────

/// Request passed to command callback.
#[repr(C)]
pub struct CommandRequest {
    /// Command arguments joined by space (same as v0.1).
    pub args: RString,
    /// The command name that was matched.
    pub command_name: RString,
    /// Sender user ID.
    pub sender_id: RString,
    /// Group ID (empty if private chat).
    pub group_id: RString,
    /// Raw OneBot event JSON (for advanced use).
    pub raw_event_json: RString,

    // ── v0.3 fields ──
    /// Sender display name / nickname.
    pub sender_nickname: RString,
    /// Message ID (if applicable).
    pub message_id: RString,
    /// Unix timestamp of the event (seconds since epoch). 0 if unavailable.
    pub timestamp: i64,
}

/// Response from command callback.
#[repr(C)]
pub struct CommandResponse {
    pub action: DynamicActionResponse,
}

impl CommandResponse {
    /// Create a simple text reply.
    pub fn text(text: &str) -> Self {
        Self {
            action: DynamicActionResponse::text_reply(text),
        }
    }

    /// Create a reply builder for rich-media responses.
    pub fn builder() -> ReplyBuilder {
        ReplyBuilder::new()
    }

    /// Create an ignore response.
    pub fn ignore() -> Self {
        Self {
            action: DynamicActionResponse::ignore(),
        }
    }
}

/// Fluent builder for constructing rich-media command responses.
///
/// # Example
/// ```
/// use abi_stable_host_api::ReplyBuilder;
/// let response = ReplyBuilder::new()
///     .text("Hello, ")
///     .at("12345")
///     .face(1)
///     .text("!")
///     .build();
/// ```
pub struct ReplyBuilder {
    segments: Vec<String>,
}

impl ReplyBuilder {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
        }
    }

    /// Add a text segment.
    pub fn text(mut self, text: &str) -> Self {
        let escaped = text
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        self.segments.push(format!(
            r#"{{"type":"text","data":{{"text":"{}"}}}}"#,
            escaped
        ));
        self
    }

    /// Add an @mention segment.
    pub fn at(mut self, user_id: &str) -> Self {
        self.segments
            .push(format!(r#"{{"type":"at","data":{{"qq":"{}"}}}}"#, user_id));
        self
    }

    /// Add an @all mention.
    pub fn at_all(mut self) -> Self {
        self.segments
            .push(r#"{"type":"at","data":{"qq":"all"}}"#.to_string());
        self
    }

    /// Add a QQ face emoji segment.
    pub fn face(mut self, id: i32) -> Self {
        self.segments
            .push(format!(r#"{{"type":"face","data":{{"id":"{}"}}}}"#, id));
        self
    }

    /// Add an image segment by URL.
    pub fn image_url(mut self, url: &str) -> Self {
        let escaped = url.replace('\\', "\\\\").replace('"', "\\\"");
        self.segments.push(format!(
            r#"{{"type":"image","data":{{"file":"{}"}}}}"#,
            escaped
        ));
        self
    }

    /// Add an image segment by base64 data.
    pub fn image_base64(mut self, base64: &str) -> Self {
        self.segments.push(format!(
            r#"{{"type":"image","data":{{"file":"base64://{}"}}}}"#,
            base64
        ));
        self
    }

    /// Add a record (voice) segment.
    pub fn record(mut self, file: &str) -> Self {
        let escaped = file.replace('\\', "\\\\").replace('"', "\\\"");
        self.segments.push(format!(
            r#"{{"type":"record","data":{{"file":"{}"}}}}"#,
            escaped
        ));
        self
    }

    /// Add a reply (quote) segment referencing a message ID.
    pub fn reply(mut self, message_id: &str) -> Self {
        self.segments.push(format!(
            r#"{{"type":"reply","data":{{"id":"{}"}}}}"#,
            message_id
        ));
        self
    }

    /// Build into a CommandResponse with rich-media content.
    pub fn build(self) -> CommandResponse {
        let json = format!("[{}]", self.segments.join(","));
        CommandResponse {
            action: DynamicActionResponse::rich_reply(&json),
        }
    }

    /// Build into a CommandResponse, but return only text if there's a single text segment.
    /// Falls back to rich_reply for multi-segment or non-text content.
    pub fn build_auto(self) -> CommandResponse {
        self.build()
    }
}

impl Default for ReplyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Interceptor FFI types ───────────────────────────────────────────────

/// Request passed to interceptor callbacks (pre_handle / after_completion).
#[repr(C)]
pub struct InterceptorRequest {
    /// Bot instance ID.
    pub bot_id: RString,
    /// Sender user ID.
    pub sender_id: RString,
    /// Group ID (empty if private chat).
    pub group_id: RString,
    /// Message plain text.
    pub message_text: RString,
    /// Full event JSON.
    pub raw_event_json: RString,
    /// Sender display name / nickname.
    pub sender_nickname: RString,
    /// Message ID (if applicable).
    pub message_id: RString,
    /// Unix timestamp of the event (seconds since epoch). 0 if unavailable.
    pub timestamp: i64,
}

/// Response from a `pre_handle` interceptor callback.
#[repr(C)]
pub struct InterceptorResponse {
    /// 1 = allow (pass through), 0 = block (stop processing).
    pub allow: i32,
}

impl InterceptorResponse {
    /// Allow the event to continue processing.
    pub fn allow() -> Self {
        Self { allow: 1 }
    }

    /// Block the event from further processing.
    pub fn block() -> Self {
        Self { allow: 0 }
    }
}

/// Describes an interceptor registered by a dynamic plugin.
#[repr(C)]
#[derive(Clone)]
pub struct InterceptorDescriptorEntry {
    /// Symbol name for the pre_handle callback. Empty = not registered.
    pub pre_handle_symbol: RString,
    /// Symbol name for the after_completion callback. Empty = not registered.
    pub after_completion_symbol: RString,
}

// ─── Notice FFI types ───────────────────────────────────────────────────

/// Request passed to notice/request/meta callbacks.
#[repr(C)]
pub struct NoticeRequest {
    /// Route name, e.g. "GroupPoke", "Friend", "Heartbeat".
    pub route: RString,
    /// Raw OneBot event JSON.
    pub raw_event_json: RString,
}

/// Response from notice/request/meta callbacks.
#[repr(C)]
pub struct NoticeResponse {
    pub action: DynamicActionResponse,
}

// ─── Command descriptor entry (v0.2) ───────────────────────────────────

/// Describes a single command registered by a dynamic plugin.
#[repr(C)]
#[derive(Clone)]
pub struct CommandDescriptorEntry {
    /// Command name (e.g. "hello").
    pub name: RString,
    /// Human-readable description.
    pub description: RString,
    /// Callback symbol name in the shared library (e.g. "my_plugin_handle_hello").
    pub callback_symbol: RString,
    /// Comma-separated aliases (e.g. "h,hi"). Empty if none.
    pub aliases: RString,
    /// Command category (e.g. "general"). Empty defaults to "dynamic".
    pub category: RString,
    /// Required role: "" or "anyone" = anyone, "admin" = admin, "owner" = owner.
    pub required_role: RString,
    /// Command scope: "" or "all" = all, "group" = group only, "private" = private only.
    pub scope: RString,
}

// ─── Route descriptor entry (v0.2) ─────────────────────────────────────

/// Describes a system event route registered by a dynamic plugin.
#[repr(C)]
#[derive(Clone)]
pub struct RouteDescriptorEntry {
    /// Route type: "notice", "request", or "meta".
    pub kind: RString,
    /// Route name, e.g. "GroupPoke", "Friend", "Heartbeat".
    /// Comma-separated for multiple routes (e.g. "GroupPoke,PrivatePoke").
    pub route: RString,
    /// Callback symbol name.
    pub callback_symbol: RString,
}

// ─── Plugin descriptor ──────────────────────────────────────────────────

/// Metadata returned by the `qimen_plugin_descriptor` FFI symbol.
///
/// v0.2: Supports multiple commands and multiple event routes.
#[repr(C)]
pub struct PluginDescriptor {
    pub plugin_id: RString,
    pub plugin_version: RString,
    pub api_version: RString,

    // ── v0.1 legacy fields (kept for backward compatibility) ──
    /// Single command name (v0.1). Ignored if `commands` is non-empty.
    pub command_name: RString,
    /// Single command description (v0.1). Ignored if `commands` is non-empty.
    pub command_description: RString,
    /// Single notice route (v0.1). Ignored if `routes` is non-empty.
    pub notice_route: RString,
    /// Single request route (v0.1).
    pub request_route: RString,
    /// Single meta route (v0.1).
    pub meta_route: RString,

    // ── v0.2 multi-command / multi-route fields ──
    /// Multiple command descriptors (v0.2). Takes precedence over legacy fields.
    pub commands: RVec<CommandDescriptorEntry>,
    /// Multiple route descriptors (v0.2). Takes precedence over legacy fields.
    pub routes: RVec<RouteDescriptorEntry>,
    /// Interceptor descriptors.
    pub interceptors: RVec<InterceptorDescriptorEntry>,
}

impl PluginDescriptor {
    /// Helper to create a v0.2 descriptor with the builder pattern.
    pub fn new(id: &str, version: &str) -> Self {
        Self {
            plugin_id: RString::from(id),
            plugin_version: RString::from(version),
            api_version: RString::from("0.3"),
            command_name: RString::new(),
            command_description: RString::new(),
            notice_route: RString::new(),
            request_route: RString::new(),
            meta_route: RString::new(),
            commands: RVec::new(),
            routes: RVec::new(),
            interceptors: RVec::new(),
        }
    }

    /// Override the declared dynamic plugin API version.
    pub fn with_api_version(mut self, api_version: &str) -> Self {
        self.api_version = RString::from(api_version);
        self
    }

    /// Add a command to this descriptor.
    pub fn add_command(mut self, name: &str, description: &str, callback_symbol: &str) -> Self {
        self.commands.push(CommandDescriptorEntry {
            name: RString::from(name),
            description: RString::from(description),
            callback_symbol: RString::from(callback_symbol),
            aliases: RString::new(),
            category: RString::new(),
            required_role: RString::new(),
            scope: RString::new(),
        });
        self
    }

    /// Add a command with full options.
    pub fn add_command_full(mut self, entry: CommandDescriptorEntry) -> Self {
        self.commands.push(entry);
        self
    }

    /// Add an interceptor entry.
    pub fn add_interceptor(
        mut self,
        pre_handle_symbol: &str,
        after_completion_symbol: &str,
    ) -> Self {
        self.interceptors.push(InterceptorDescriptorEntry {
            pre_handle_symbol: RString::from(pre_handle_symbol),
            after_completion_symbol: RString::from(after_completion_symbol),
        });
        self
    }

    /// Add a system event route.
    pub fn add_route(mut self, kind: &str, route: &str, callback_symbol: &str) -> Self {
        self.routes.push(RouteDescriptorEntry {
            kind: RString::from(kind),
            route: RString::from(route),
            callback_symbol: RString::from(callback_symbol),
        });
        self
    }
}

// ─── Plugin lifecycle hooks (v0.3) ──────────────────────────────────────

/// Configuration passed to plugin init hook.
#[repr(C)]
pub struct PluginInitConfig {
    /// Plugin ID (same as in descriptor).
    pub plugin_id: RString,
    /// Plugin-specific configuration as JSON string.
    /// Loaded from config/plugins/<plugin_id>.toml and serialized to JSON.
    /// Empty string if no config file exists.
    pub config_json: RString,
    /// The directory where the plugin binary resides.
    pub plugin_dir: RString,
    /// The bot's data directory root.
    pub data_dir: RString,
}

/// Result from plugin init hook.
#[repr(C)]
pub struct PluginInitResult {
    /// 0 = success, non-zero = failure.
    pub code: i32,
    /// Error message if code != 0. Empty on success.
    pub error_message: RString,
}

impl PluginInitResult {
    pub fn ok() -> Self {
        Self {
            code: 0,
            error_message: RString::new(),
        }
    }

    pub fn err(message: &str) -> Self {
        Self {
            code: 1,
            error_message: RString::from(message),
        }
    }
}

// ─── Send queue (BotApi) ─────────────────────────────────────────────────

/// An outbound send action queued by plugin code via `BotApi` / `SendBuilder`.
///
/// The host drains the queue after each FFI callback and executes the sends
/// asynchronously.
#[repr(C)]
#[derive(Clone)]
pub struct SendAction {
    /// `"private"` or `"group"`.
    pub message_type: RString,
    /// Target user_id (for private) or group_id (for group).
    pub target_id: RString,
    /// Plain text message body (used when `segments_json` is empty).
    pub message: RString,
    /// JSON-encoded rich-media segments (takes precedence over `message`).
    pub segments_json: RString,
}

/// Current proactive send request schema version.
pub const PROACTIVE_SEND_SCHEMA_VERSION: u32 = 1;

/// Host API v1 function table version.
pub const HOST_API_V1_ABI_VERSION: u32 = 1;

/// Stable status returned when the host accepts a proactive send request.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendEnqueueStatus {
    Accepted = 0,
    HostUnavailable = 1,
    InvalidRequest = 2,
    BotNotFound = 3,
    BotDisabled = 4,
    QueueFull = 5,
    HostShuttingDown = 6,
}

impl SendEnqueueStatus {
    /// Convert an FFI status code into the public Rust status enum.
    pub fn from_code(code: i32) -> Self {
        match code {
            0 => Self::Accepted,
            2 => Self::InvalidRequest,
            3 => Self::BotNotFound,
            4 => Self::BotDisabled,
            5 => Self::QueueFull,
            6 => Self::HostShuttingDown,
            _ => Self::HostUnavailable,
        }
    }

    /// Return the stable integer representation used across the FFI boundary.
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Return whether the host accepted ownership of the request.
    pub const fn is_accepted(self) -> bool {
        matches!(self, Self::Accepted)
    }
}

/// A proactive send request submitted directly from plugin code.
#[repr(C)]
#[derive(Clone)]
pub struct ProactiveSendRequest {
    pub schema_version: u32,
    pub bot_id: RString,
    pub target_kind: RString,
    pub target_id: RString,
    pub context_json: RString,
    pub message: RString,
    pub segments_json: RString,
    pub options_json: RString,
}

impl ProactiveSendRequest {
    /// Build a v1 request with empty JSON extension objects.
    pub fn new(bot_id: &str, target_kind: &str, target_id: &str) -> Self {
        Self {
            schema_version: PROACTIVE_SEND_SCHEMA_VERSION,
            bot_id: RString::from(bot_id),
            target_kind: RString::from(target_kind),
            target_id: RString::from(target_id),
            context_json: RString::from("{}"),
            message: RString::new(),
            segments_json: RString::new(),
            options_json: RString::from("{}"),
        }
    }
}

/// Host callback used to copy and enqueue a proactive send request.
pub type HostEnqueueSendFn = unsafe extern "C" fn(*mut c_void, *const ProactiveSendRequest) -> i32;

/// Versioned host function table bound to an API 0.4 dynamic plugin.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HostApiV1 {
    pub abi_version: u32,
    pub context: *mut c_void,
    pub enqueue_send: Option<HostEnqueueSendFn>,
}

#[derive(Clone, Copy)]
struct HostApiBinding(HostApiV1);

// SAFETY: The host owns the context and keeps it alive until unbind returns.
// Access is serialized through HOST_API, and unbind waits for active readers.
unsafe impl Send for HostApiBinding {}
unsafe impl Sync for HostApiBinding {}

static HOST_API: RwLock<Option<HostApiBinding>> = RwLock::new(None);

/// Bind the host function table used by real-time proactive sends.
///
/// # Safety
/// The api pointer must be valid for this call. The host must keep the
/// referenced context alive until unbind_host_api_v1 returns.
pub unsafe fn bind_host_api_v1(api: *const HostApiV1) -> i32 {
    if api.is_null() {
        return SendEnqueueStatus::InvalidRequest.code();
    }

    // SAFETY: The caller guarantees that api is valid for this call.
    let api = unsafe { *api };
    if api.abi_version != HOST_API_V1_ABI_VERSION || api.enqueue_send.is_none() {
        return SendEnqueueStatus::InvalidRequest.code();
    }

    match HOST_API.write() {
        Ok(mut binding) => {
            *binding = Some(HostApiBinding(api));
            SendEnqueueStatus::Accepted.code()
        }
        Err(_) => SendEnqueueStatus::HostUnavailable.code(),
    }
}

/// Unbind the current host table after all plugin background workers stop.
pub fn unbind_host_api_v1() -> i32 {
    match HOST_API.write() {
        Ok(mut binding) => {
            *binding = None;
            SendEnqueueStatus::Accepted.code()
        }
        Err(_) => SendEnqueueStatus::HostUnavailable.code(),
    }
}

/// Submit a proactive request through the currently bound host API.
pub fn submit_proactive_send(request: &ProactiveSendRequest) -> SendEnqueueStatus {
    if request.schema_version != PROACTIVE_SEND_SCHEMA_VERSION
        || request.bot_id.is_empty()
        || request.target_id.is_empty()
        || !matches!(
            request.target_kind.as_str(),
            "private" | "group" | "channel" | "channel_private"
        )
        || (request.message.is_empty() && request.segments_json.is_empty())
    {
        return SendEnqueueStatus::InvalidRequest;
    }

    let binding = match HOST_API.read() {
        Ok(binding) => binding,
        Err(_) => return SendEnqueueStatus::HostUnavailable,
    };
    let Some(binding) = *binding else {
        return SendEnqueueStatus::HostUnavailable;
    };
    let Some(enqueue_send) = binding.0.enqueue_send else {
        return SendEnqueueStatus::HostUnavailable;
    };

    // SAFETY: bind_host_api_v1 validated the callback. Holding the read lock
    // prevents unbind from releasing the host context during this call.
    SendEnqueueStatus::from_code(unsafe { enqueue_send(binding.0.context, request) })
}

static SEND_QUEUE: Mutex<Vec<SendAction>> = Mutex::new(Vec::new());

/// Drain all queued send actions. Called by the generated `qimen_plugin_flush_sends` symbol.
pub fn drain_send_queue() -> Vec<SendAction> {
    SEND_QUEUE
        .lock()
        .map(|mut q| q.drain(..).collect())
        .unwrap_or_default()
}

/// Provides static methods for plugins to queue outbound messages to arbitrary
/// users or groups. Messages are buffered in a process-local queue and flushed
/// by the host after the callback returns.
pub struct BotApi;

impl BotApi {
    /// Select a concrete runtime bot for real-time proactive sends.
    pub fn for_bot(bot_id: &str) -> ProactiveBotApi {
        ProactiveBotApi {
            bot_id: bot_id.to_string(),
        }
    }

    /// Send a plain text message to a private chat.
    pub fn send_private_msg(user_id: &str, text: &str) {
        Self::push(SendAction {
            message_type: RString::from("private"),
            target_id: RString::from(user_id),
            message: RString::from(text),
            segments_json: RString::new(),
        });
    }

    /// Send a plain text message to a group chat.
    pub fn send_group_msg(group_id: &str, text: &str) {
        Self::push(SendAction {
            message_type: RString::from("group"),
            target_id: RString::from(group_id),
            message: RString::from(text),
            segments_json: RString::new(),
        });
    }

    /// Send rich-media (JSON segments) to a private chat.
    pub fn send_private_rich(user_id: &str, segments_json: &str) {
        Self::push(SendAction {
            message_type: RString::from("private"),
            target_id: RString::from(user_id),
            message: RString::new(),
            segments_json: RString::from(segments_json),
        });
    }

    /// Send rich-media (JSON segments) to a group chat.
    pub fn send_group_rich(group_id: &str, segments_json: &str) {
        Self::push(SendAction {
            message_type: RString::from("group"),
            target_id: RString::from(group_id),
            message: RString::new(),
            segments_json: RString::from(segments_json),
        });
    }

    fn push(action: SendAction) {
        if let Ok(mut q) = SEND_QUEUE.lock() {
            q.push(action);
        }
    }
}

/// Bot-scoped real-time API used by background plugin workers.
pub struct ProactiveBotApi {
    bot_id: String,
}

impl ProactiveBotApi {
    /// Send a text message to a private target.
    pub fn send_private_msg(&self, user_id: &str, text: &str) -> SendEnqueueStatus {
        self.send_text("private", user_id, "{}", text)
    }

    /// Send a text message to a group target.
    pub fn send_group_msg(&self, group_id: &str, text: &str) -> SendEnqueueStatus {
        self.send_text("group", group_id, "{}", text)
    }

    /// Send a text message to a channel target.
    pub fn send_channel_msg(&self, channel_id: &str, text: &str) -> SendEnqueueStatus {
        self.send_text("channel", channel_id, "{}", text)
    }

    /// Send a OneBot guild channel message with both routing identifiers.
    pub fn send_guild_channel_msg(
        &self,
        guild_id: &str,
        channel_id: &str,
        text: &str,
    ) -> SendEnqueueStatus {
        let context = format!(r#"{{"guild_id":"{}"}}"#, escape_json_string(guild_id));
        self.send_text("channel", channel_id, &context, text)
    }

    /// Send a text message to a channel-private target.
    pub fn send_channel_private_msg(&self, guild_id: &str, text: &str) -> SendEnqueueStatus {
        self.send_text("channel_private", guild_id, "{}", text)
    }

    /// Send pre-encoded rich segments to any supported target kind.
    pub fn send_rich(
        &self,
        target_kind: &str,
        target_id: &str,
        context_json: &str,
        segments_json: &str,
    ) -> SendEnqueueStatus {
        let mut request = ProactiveSendRequest::new(&self.bot_id, target_kind, target_id);
        request.context_json = RString::from(context_json);
        request.segments_json = RString::from(segments_json);
        submit_proactive_send(&request)
    }

    fn send_text(
        &self,
        target_kind: &str,
        target_id: &str,
        context_json: &str,
        text: &str,
    ) -> SendEnqueueStatus {
        let mut request = ProactiveSendRequest::new(&self.bot_id, target_kind, target_id);
        request.context_json = RString::from(context_json);
        request.message = RString::from(text);
        submit_proactive_send(&request)
    }
}

fn escape_json_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Fluent builder for constructing and queuing a rich-media send to an
/// arbitrary target (group or private).
///
/// # Example
/// ```ignore
/// SendBuilder::group("123456")
///     .text("hello ")
///     .at("789")
///     .send();
/// ```
pub struct SendBuilder {
    message_type: String,
    target_id: String,
    bot_id: Option<String>,
    context_json: String,
    options_json: String,
    segments: Vec<String>,
}

impl SendBuilder {
    /// Start building a message destined for a group.
    pub fn group(group_id: &str) -> Self {
        Self {
            message_type: "group".to_string(),
            target_id: group_id.to_string(),
            bot_id: None,
            context_json: "{}".to_string(),
            options_json: "{}".to_string(),
            segments: Vec::new(),
        }
    }

    /// Start building a message destined for a private chat.
    pub fn private(user_id: &str) -> Self {
        Self {
            message_type: "private".to_string(),
            target_id: user_id.to_string(),
            bot_id: None,
            context_json: "{}".to_string(),
            options_json: "{}".to_string(),
            segments: Vec::new(),
        }
    }

    /// Start building a message destined for a channel.
    pub fn channel(channel_id: &str) -> Self {
        Self {
            message_type: "channel".to_string(),
            target_id: channel_id.to_string(),
            bot_id: None,
            context_json: "{}".to_string(),
            options_json: "{}".to_string(),
            segments: Vec::new(),
        }
    }

    /// Start building a message destined for a channel-private conversation.
    pub fn channel_private(guild_id: &str) -> Self {
        Self {
            message_type: "channel_private".to_string(),
            target_id: guild_id.to_string(),
            bot_id: None,
            context_json: "{}".to_string(),
            options_json: "{}".to_string(),
            segments: Vec::new(),
        }
    }

    /// Select the concrete runtime bot used by try_send.
    pub fn bot(mut self, bot_id: &str) -> Self {
        self.bot_id = Some(bot_id.to_string());
        self
    }

    /// Attach the OneBot guild identifier required by channel sends.
    pub fn guild_id(mut self, guild_id: &str) -> Self {
        self.context_json = format!(r#"{{"guild_id":"{}"}}"#, escape_json_string(guild_id));
        self
    }

    /// Replace the protocol-specific routing context JSON object.
    pub fn context_json(mut self, context_json: &str) -> Self {
        self.context_json = context_json.to_string();
        self
    }

    /// Replace the optional send behavior JSON object.
    pub fn options_json(mut self, options_json: &str) -> Self {
        self.options_json = options_json.to_string();
        self
    }

    /// Add a text segment.
    pub fn text(mut self, text: &str) -> Self {
        let escaped = escape_json_string(text);
        self.segments.push(format!(
            r#"{{"type":"text","data":{{"text":"{}"}}}}"#,
            escaped
        ));
        self
    }

    /// Add an @mention segment.
    pub fn at(mut self, user_id: &str) -> Self {
        self.segments
            .push(format!(r#"{{"type":"at","data":{{"qq":"{}"}}}}"#, user_id));
        self
    }

    /// Add an @all mention.
    pub fn at_all(mut self) -> Self {
        self.segments
            .push(r#"{"type":"at","data":{"qq":"all"}}"#.to_string());
        self
    }

    /// Add a QQ face emoji segment.
    pub fn face(mut self, id: i32) -> Self {
        self.segments
            .push(format!(r#"{{"type":"face","data":{{"id":"{}"}}}}"#, id));
        self
    }

    /// Add an image segment by URL.
    pub fn image_url(mut self, url: &str) -> Self {
        let escaped = url.replace('\\', "\\\\").replace('"', "\\\"");
        self.segments.push(format!(
            r#"{{"type":"image","data":{{"file":"{}"}}}}"#,
            escaped
        ));
        self
    }

    /// Add an image segment by base64 data.
    pub fn image_base64(mut self, base64: &str) -> Self {
        self.segments.push(format!(
            r#"{{"type":"image","data":{{"file":"base64://{}"}}}}"#,
            base64
        ));
        self
    }

    /// Queue the built message for sending. The host will flush and send
    /// it after the current FFI callback returns.
    pub fn send(self) {
        let json = format!("[{}]", self.segments.join(","));
        BotApi::push(SendAction {
            message_type: RString::from(self.message_type),
            target_id: RString::from(self.target_id),
            message: RString::new(),
            segments_json: RString::from(json),
        });
    }

    /// Submit the built message immediately through the bound host API.
    pub fn try_send(self) -> SendEnqueueStatus {
        let Some(bot_id) = self.bot_id else {
            return SendEnqueueStatus::InvalidRequest;
        };
        let mut request = ProactiveSendRequest::new(&bot_id, &self.message_type, &self.target_id);
        request.context_json = RString::from(self.context_json);
        request.options_json = RString::from(self.options_json);
        request.segments_json = RString::from(format!("[{}]", self.segments.join(",")));
        submit_proactive_send(&request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn record_send(
        context: *mut c_void,
        request: *const ProactiveSendRequest,
    ) -> i32 {
        if context.is_null() || request.is_null() {
            return SendEnqueueStatus::InvalidRequest.code();
        }
        // SAFETY: The test keeps both allocations alive until unbind completes.
        let output = unsafe { &*(context.cast::<Mutex<Vec<ProactiveSendRequest>>>()) };
        let request = unsafe { &*request };
        match output.lock() {
            Ok(mut output) => {
                output.push(request.clone());
                SendEnqueueStatus::Accepted.code()
            }
            Err(_) => SendEnqueueStatus::HostUnavailable.code(),
        }
    }

    #[test]
    fn api_04_is_current_and_legacy_versions_remain_compatible() {
        assert_eq!(expected_api_version().as_str(), "0.4");
        for version in ["0.1", "0.2", "0.3", "0.4"] {
            assert!(is_compatible_api_version(version));
        }
        assert!(!is_compatible_api_version("0.5"));
    }

    #[test]
    fn bot_scoped_builder_submits_owned_channel_routing() {
        let _guard = TEST_LOCK.lock().expect("test lock");
        let output = Box::new(Mutex::new(Vec::<ProactiveSendRequest>::new()));
        let context = (&*output as *const Mutex<Vec<ProactiveSendRequest>>)
            .cast_mut()
            .cast::<c_void>();
        let api = HostApiV1 {
            abi_version: HOST_API_V1_ABI_VERSION,
            context,
            enqueue_send: Some(record_send),
        };

        // SAFETY: api and output remain alive until unbind returns.
        assert_eq!(
            unsafe { bind_host_api_v1(&api) },
            SendEnqueueStatus::Accepted.code()
        );
        let status = SendBuilder::channel("channel-1")
            .bot("bot-main")
            .guild_id("guild-1")
            .text("hello")
            .try_send();
        assert_eq!(status, SendEnqueueStatus::Accepted);
        assert_eq!(unbind_host_api_v1(), SendEnqueueStatus::Accepted.code());

        let output = output.lock().expect("output");
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].bot_id.as_str(), "bot-main");
        assert_eq!(output[0].target_kind.as_str(), "channel");
        assert_eq!(output[0].target_id.as_str(), "channel-1");
        assert_eq!(output[0].context_json.as_str(), r#"{"guild_id":"guild-1"}"#);
        assert!(output[0].segments_json.as_str().contains("hello"));
    }

    struct BlockingContext {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
    }

    unsafe extern "C" fn blocking_send(
        context: *mut c_void,
        _request: *const ProactiveSendRequest,
    ) -> i32 {
        // SAFETY: The test keeps the context alive until both threads join.
        let context = unsafe { &*context.cast::<BlockingContext>() };
        context.entered.wait();
        context.release.wait();
        SendEnqueueStatus::Accepted.code()
    }

    #[test]
    fn unbind_waits_for_an_in_flight_host_callback() {
        let _guard = TEST_LOCK.lock().expect("test lock");
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let context = Box::new(BlockingContext {
            entered: Arc::clone(&entered),
            release: Arc::clone(&release),
        });
        let api = HostApiV1 {
            abi_version: HOST_API_V1_ABI_VERSION,
            context: (&*context as *const BlockingContext).cast_mut().cast(),
            enqueue_send: Some(blocking_send),
        };
        // SAFETY: context remains alive until send and unbind complete.
        assert_eq!(
            unsafe { bind_host_api_v1(&api) },
            SendEnqueueStatus::Accepted.code()
        );

        let sender =
            std::thread::spawn(|| BotApi::for_bot("bot-main").send_group_msg("group-1", "hello"));
        entered.wait();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let unbinder = std::thread::spawn(move || {
            let status = unbind_host_api_v1();
            done_tx.send(status).expect("send unbind status");
        });

        assert!(
            done_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        release.wait();
        assert_eq!(sender.join().expect("sender"), SendEnqueueStatus::Accepted);
        assert_eq!(
            done_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .expect("unbind status"),
            SendEnqueueStatus::Accepted.code()
        );
        unbinder.join().expect("unbinder");
        assert_eq!(
            BotApi::for_bot("bot-main").send_group_msg("group-1", "after"),
            SendEnqueueStatus::HostUnavailable
        );
    }
}
