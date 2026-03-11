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

use abi_stable::std_types::{RString, RVec};

/// Current plugin API version. Dynamic plugins must declare the same version
/// to be loaded by the host.
pub fn expected_api_version() -> RString {
    RString::from("0.2")
}

/// Also accept legacy 0.1 plugins for backward compatibility.
pub fn is_compatible_api_version(version: &str) -> bool {
    version == "0.1" || version == "0.2"
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
}

/// Response from command callback.
#[repr(C)]
pub struct CommandResponse {
    pub action: DynamicActionResponse,
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
}

impl PluginDescriptor {
    /// Helper to create a v0.2 descriptor with the builder pattern.
    pub fn new(id: &str, version: &str) -> Self {
        Self {
            plugin_id: RString::from(id),
            plugin_version: RString::from(version),
            api_version: RString::from("0.2"),
            command_name: RString::new(),
            command_description: RString::new(),
            notice_route: RString::new(),
            request_route: RString::new(),
            meta_route: RString::new(),
            commands: RVec::new(),
            routes: RVec::new(),
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
        });
        self
    }

    /// Add a command with full options.
    pub fn add_command_full(mut self, entry: CommandDescriptorEntry) -> Self {
        self.commands.push(entry);
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
