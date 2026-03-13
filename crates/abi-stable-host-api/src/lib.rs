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

/// Current plugin API version. Dynamic plugins must declare the same version
/// to be loaded by the host.
pub fn expected_api_version() -> RString {
    RString::from("0.3")
}

/// Also accept legacy 0.1 / 0.2 plugins for backward compatibility.
pub fn is_compatible_api_version(version: &str) -> bool {
    version == "0.1" || version == "0.2" || version == "0.3"
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
        self.segments.push(format!(
            r#"{{"type":"at","data":{{"qq":"{}"}}}}"#,
            user_id
        ));
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
        self.segments.push(format!(
            r#"{{"type":"face","data":{{"id":"{}"}}}}"#,
            id
        ));
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

    /// Add a command to this descriptor.
    pub fn add_command(
        mut self,
        name: &str,
        description: &str,
        callback_symbol: &str,
    ) -> Self {
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
    pub fn add_route(
        mut self,
        kind: &str,
        route: &str,
        callback_symbol: &str,
    ) -> Self {
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
