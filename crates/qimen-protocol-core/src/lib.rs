//! Protocol-agnostic core types for QimenBot event processing and action dispatch.
//!
//! Defines the normalized event/action model that all protocol adapters
//! translate to and from, enabling the runtime to work with any chat protocol.

pub mod event_dto;

use async_trait::async_trait;
use bytes::Bytes;
use qimen_error::QimenError;
use qimen_message::Message;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type ProtocolResult<T> = std::result::Result<T, QimenError>;

/// Identifies the chat protocol a bot connection uses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolId {
    OneBot11,
    OneBot12,
    Satori,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportMode {
    WsForward,
    WsReverse,
    HttpApi,
    HttpPost,
    Webhook,
    Custom(String),
}

/// High-level classification of a normalized event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Message,
    MessageSent,
    Notice,
    Request,
    Meta,
    Internal(String),
}

/// Set of features/capabilities that a protocol adapter supports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilitySet {
    #[serde(default)]
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActorRef {
    pub id: String,
    pub display_name: Option<String>,
}

/// Reference to a chat context (group, channel, or private conversation).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatRef {
    pub id: String,
    /// Chat type, e.g. `"group"`, `"private"`, `"channel"`.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingPacket {
    pub protocol: ProtocolId,
    pub transport_mode: TransportMode,
    pub bot_instance: String,
    pub payload: Value,
    pub raw_bytes: Option<Bytes>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutgoingPacket {
    pub payload: Value,
}

/// A protocol-agnostic event produced by a [`ProtocolAdapter`] from raw incoming data.
///
/// Contains the parsed message (if applicable), the actor who triggered the event,
/// the chat context, and the full raw JSON for adapter-specific fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub protocol: ProtocolId,
    pub bot_instance: String,
    pub transport_mode: TransportMode,
    pub time: Option<i64>,
    pub kind: EventKind,
    pub message: Option<Message>,
    pub actor: Option<ActorRef>,
    pub chat: Option<ChatRef>,
    pub raw_json: Value,
    pub raw_bytes: Option<Bytes>,
    pub extensions: Map<String, Value>,
}

impl NormalizedEvent {
    // ── Identity ──

    /// Sender user ID from `actor.id`.
    pub fn sender_id(&self) -> Option<&str> {
        self.actor.as_ref().map(|a| a.id.as_str())
    }

    /// Sender user ID parsed as `i64` (from `actor.id`). Useful for APIs that accept numeric IDs.
    pub fn sender_id_i64(&self) -> Option<i64> {
        self.actor.as_ref().and_then(|a| a.id.parse().ok())
    }

    /// Sender display name from `actor.display_name`.
    pub fn sender_nickname(&self) -> Option<&str> {
        self.actor
            .as_ref()
            .and_then(|a| a.display_name.as_deref())
    }

    // ── Chat context ──

    /// Chat ID from `chat.id`.
    pub fn chat_id(&self) -> Option<&str> {
        self.chat.as_ref().map(|c| c.id.as_str())
    }

    /// Returns `chat.id` only when the chat is a group.
    pub fn group_id(&self) -> Option<&str> {
        self.chat
            .as_ref()
            .filter(|c| c.kind == "group")
            .map(|c| c.id.as_str())
    }

    /// Whether the event originated in a group chat.
    pub fn is_group(&self) -> bool {
        self.chat.as_ref().is_some_and(|c| c.kind == "group")
    }

    /// Whether the event originated in a private chat.
    pub fn is_private(&self) -> bool {
        self.chat.as_ref().is_some_and(|c| c.kind == "private")
    }

    // ── Raw JSON top-level fields ──

    /// `raw_json["user_id"]` as `i64`. Useful for OneBot API calls that accept `i64`.
    pub fn user_id(&self) -> Option<i64> {
        self.raw_json.get("user_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["group_id"]` as `i64`. Useful for OneBot API calls that accept `i64`.
    pub fn group_id_i64(&self) -> Option<i64> {
        self.raw_json.get("group_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["message_id"]` as `i64`.
    pub fn message_id(&self) -> Option<i64> {
        self.raw_json.get("message_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["self_id"]` as `i64`.
    pub fn self_id(&self) -> Option<i64> {
        self.raw_json.get("self_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["self_id"]` as a lossless `String` (handles both string and number JSON values).
    pub fn self_id_str(&self) -> Option<String> {
        self.raw_json.get("self_id").map(value_to_lossless_id)
    }

    /// `raw_json["sub_type"]` as `&str`.
    pub fn sub_type(&self) -> Option<&str> {
        self.raw_json.get("sub_type").and_then(|v| v.as_str())
    }

    /// `raw_json["message_type"]` as `&str` (e.g. `"private"`, `"group"`).
    pub fn message_type(&self) -> Option<&str> {
        self.raw_json
            .get("message_type")
            .and_then(|v| v.as_str())
    }

    /// `raw_json["post_type"]` as `&str` (e.g. `"message"`, `"notice"`, `"request"`, `"meta_event"`).
    pub fn post_type(&self) -> Option<&str> {
        self.raw_json.get("post_type").and_then(|v| v.as_str())
    }

    /// `raw_json["notice_type"]` as `&str` (e.g. `"group_increase"`, `"friend_add"`).
    pub fn notice_type(&self) -> Option<&str> {
        self.raw_json.get("notice_type").and_then(|v| v.as_str())
    }

    /// `raw_json["request_type"]` as `&str` (e.g. `"friend"`, `"group"`).
    pub fn request_type(&self) -> Option<&str> {
        self.raw_json
            .get("request_type")
            .and_then(|v| v.as_str())
    }

    // ── Sender detail fields (from raw_json.sender) ──

    /// Generic accessor for `raw_json["sender"][field]`.
    pub fn sender_field(&self, field: &str) -> Option<&Value> {
        self.raw_json
            .get("sender")
            .and_then(|s| s.get(field))
    }

    /// Sender role in group: `"owner"` / `"admin"` / `"member"`.
    pub fn sender_role(&self) -> Option<&str> {
        self.sender_field("role").and_then(|v| v.as_str())
    }

    /// Sender group card (nickname override).
    pub fn sender_card(&self) -> Option<&str> {
        self.sender_field("card").and_then(|v| v.as_str())
    }

    /// Sender sex.
    pub fn sender_sex(&self) -> Option<&str> {
        self.sender_field("sex").and_then(|v| v.as_str())
    }

    /// Sender age.
    pub fn sender_age(&self) -> Option<i64> {
        self.sender_field("age").and_then(|v| v.as_i64())
    }

    /// Sender level string.
    pub fn sender_level(&self) -> Option<&str> {
        self.sender_field("level").and_then(|v| v.as_str())
    }

    /// Sender exclusive title in group.
    pub fn sender_title(&self) -> Option<&str> {
        self.sender_field("title").and_then(|v| v.as_str())
    }

    // ── Notice / request fields ──

    /// `raw_json["operator_id"]` as `i64`.
    pub fn operator_id(&self) -> Option<i64> {
        self.raw_json.get("operator_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["target_id"]` as `i64`.
    pub fn target_id(&self) -> Option<i64> {
        self.raw_json.get("target_id").and_then(|v| v.as_i64())
    }

    /// `raw_json["comment"]` as `&str`.
    pub fn comment(&self) -> Option<&str> {
        self.raw_json.get("comment").and_then(|v| v.as_str())
    }

    /// `raw_json["flag"]` as `&str`.
    pub fn flag(&self) -> Option<&str> {
        self.raw_json.get("flag").and_then(|v| v.as_str())
    }

    /// `raw_json["duration"]` as `i64`.
    pub fn duration(&self) -> Option<i64> {
        self.raw_json.get("duration").and_then(|v| v.as_i64())
    }

    // ── Convenience ──

    /// Shortcut to `message.plain_text()`. Returns empty string if no message.
    pub fn plain_text(&self) -> String {
        self.message
            .as_ref()
            .map(|m| m.plain_text())
            .unwrap_or_default()
    }

    /// Whether the message contains an @-mention targeting this bot's `self_id`.
    pub fn is_at_self(&self) -> bool {
        let Some(sid) = self.self_id() else {
            return false;
        };
        let sid_str = sid.to_string();
        self.message
            .as_ref()
            .is_some_and(|m| m.has_at(&sid_str))
    }

    /// Whether the sender is a group admin or owner (based on `raw_json["sender"]["role"]`).
    pub fn is_group_admin_or_owner(&self) -> bool {
        self.sender_role()
            .is_some_and(|role| role == "admin" || role == "owner")
    }

    /// Whether this is a poke event targeting the bot itself (`target_id == self_id`).
    pub fn is_poke_self(&self) -> bool {
        match (self.self_id(), self.target_id()) {
            (Some(sid), Some(tid)) => sid == tid,
            _ => false,
        }
    }
}

/// Convert a JSON value to a lossless string ID (handles both string and number values).
pub fn value_to_lossless_id(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        other => other.to_string(),
    }
}

// ── QQ avatar helpers ──

/// Build a QQ user avatar URL. Common sizes: 640 (large), 100 (medium), 40 (small).
pub fn qq_avatar_url(user_id: &str, size: u32) -> String {
    format!("https://q1.qlogo.cn/g?b=qq&nk={user_id}&s={size}")
}

/// Build a QQ group avatar URL. Common sizes: 640 (large), 100 (medium), 40 (small).
pub fn qq_group_avatar_url(group_id: &str, size: u32) -> String {
    format!("https://p.qlogo.cn/gh/{group_id}/{group_id}/{size}")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMeta {
    pub source: String,
}

/// A protocol-agnostic action request to be sent through the bot connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedActionRequest {
    pub protocol: ProtocolId,
    pub bot_instance: String,
    pub action: String,
    pub params: Value,
    pub echo: Option<Value>,
    pub timeout_ms: u64,
    pub metadata: ActionMeta,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionStatus {
    Ok,
    Async,
    Failed,
}

/// Response received after executing an action through the bot connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedActionResponse {
    pub protocol: ProtocolId,
    pub bot_instance: String,
    pub status: ActionStatus,
    pub retcode: i64,
    pub data: Value,
    pub echo: Option<Value>,
    pub latency_ms: u128,
    pub raw_json: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuickOpPatch {
    pub reply_text: Option<String>,
    pub approve: Option<bool>,
    pub reason: Option<String>,
}

#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    fn protocol_id(&self) -> ProtocolId;
    fn supported_transports(&self) -> &'static [TransportMode];
    fn capabilities(&self) -> CapabilitySet;

    async fn decode_event(&self, packet: IncomingPacket) -> ProtocolResult<NormalizedEvent>;

    async fn decode_action_response(
        &self,
        packet: IncomingPacket,
    ) -> ProtocolResult<NormalizedActionResponse>;

    async fn encode_action(&self, req: &NormalizedActionRequest) -> ProtocolResult<OutgoingPacket>;

    fn quick_op_from_event_and_patch(
        &self,
        event: &NormalizedEvent,
        patch: &QuickOpPatch,
    ) -> ProtocolResult<Option<OutgoingPacket>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_event(raw: Value) -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: "test".into(),
            transport_mode: TransportMode::WsReverse,
            time: Some(0),
            kind: EventKind::Message,
            message: None,
            actor: Some(ActorRef {
                id: "12345".into(),
                display_name: Some("Alice".into()),
            }),
            chat: Some(ChatRef {
                id: "67890".into(),
                kind: "group".into(),
            }),
            raw_json: raw,
            raw_bytes: None,
            extensions: Map::new(),
        }
    }

    #[test]
    fn test_identity_methods() {
        let evt = make_event(json!({}));
        assert_eq!(evt.sender_id(), Some("12345"));
        assert_eq!(evt.sender_id_i64(), Some(12345));
        assert_eq!(evt.sender_nickname(), Some("Alice"));
    }

    #[test]
    fn test_sender_id_i64_non_numeric() {
        let mut evt = make_event(json!({}));
        evt.actor = Some(ActorRef {
            id: "not_a_number".into(),
            display_name: None,
        });
        assert_eq!(evt.sender_id_i64(), None);
        assert_eq!(evt.sender_id(), Some("not_a_number"));
    }

    #[test]
    fn test_chat_methods() {
        let evt = make_event(json!({}));
        assert_eq!(evt.chat_id(), Some("67890"));
        assert_eq!(evt.group_id(), Some("67890"));
        assert!(evt.is_group());
        assert!(!evt.is_private());
    }

    #[test]
    fn test_private_chat() {
        let mut evt = make_event(json!({}));
        evt.chat = Some(ChatRef {
            id: "111".into(),
            kind: "private".into(),
        });
        assert!(evt.is_private());
        assert!(!evt.is_group());
        assert_eq!(evt.group_id(), None);
    }

    #[test]
    fn test_raw_json_fields() {
        let evt = make_event(json!({
            "user_id": 12345,
            "group_id": 67890,
            "message_id": 999,
            "self_id": 10001,
            "sub_type": "normal",
            "message_type": "group",
            "post_type": "message",
            "notice_type": "group_increase",
            "request_type": "friend",
            "operator_id": 555,
            "target_id": 666,
            "comment": "hello",
            "flag": "abc",
            "duration": 300,
            "sender": {
                "role": "admin",
                "card": "Card",
                "sex": "male",
                "age": 25,
                "level": "10",
                "title": "VIP"
            }
        }));
        assert_eq!(evt.user_id(), Some(12345));
        assert_eq!(evt.group_id_i64(), Some(67890));
        assert_eq!(evt.message_id(), Some(999));
        assert_eq!(evt.self_id(), Some(10001));
        assert_eq!(evt.self_id_str(), Some("10001".to_string()));
        assert_eq!(evt.sub_type(), Some("normal"));
        assert_eq!(evt.message_type(), Some("group"));
        assert_eq!(evt.post_type(), Some("message"));
        assert_eq!(evt.notice_type(), Some("group_increase"));
        assert_eq!(evt.request_type(), Some("friend"));
        assert_eq!(evt.operator_id(), Some(555));
        assert_eq!(evt.target_id(), Some(666));
        assert_eq!(evt.comment(), Some("hello"));
        assert_eq!(evt.flag(), Some("abc"));
        assert_eq!(evt.duration(), Some(300));
        assert_eq!(evt.sender_role(), Some("admin"));
        assert_eq!(evt.sender_card(), Some("Card"));
        assert_eq!(evt.sender_sex(), Some("male"));
        assert_eq!(evt.sender_age(), Some(25));
        assert_eq!(evt.sender_level(), Some("10"));
        assert_eq!(evt.sender_title(), Some("VIP"));
    }

    #[test]
    fn test_plain_text_no_message() {
        let evt = make_event(json!({}));
        assert_eq!(evt.plain_text(), "");
    }

    #[test]
    fn test_plain_text_with_message() {
        let mut evt = make_event(json!({}));
        evt.message = Some(Message::text("hello world"));
        assert_eq!(evt.plain_text(), "hello world");
    }

    #[test]
    fn test_is_at_self() {
        let mut evt = make_event(json!({"self_id": 10001}));
        let msg = Message::builder().text("hi ").at("10001").build();
        evt.message = Some(msg);
        assert!(evt.is_at_self());
    }

    #[test]
    fn test_is_at_self_false() {
        let mut evt = make_event(json!({"self_id": 10001}));
        evt.message = Some(Message::text("no at"));
        assert!(!evt.is_at_self());
    }

    #[test]
    fn test_is_group_admin_or_owner() {
        let evt = make_event(json!({"sender": {"role": "admin"}}));
        assert!(evt.is_group_admin_or_owner());

        let evt = make_event(json!({"sender": {"role": "owner"}}));
        assert!(evt.is_group_admin_or_owner());

        let evt = make_event(json!({"sender": {"role": "member"}}));
        assert!(!evt.is_group_admin_or_owner());

        let evt = make_event(json!({}));
        assert!(!evt.is_group_admin_or_owner());
    }

    #[test]
    fn test_self_id_str_from_string_value() {
        let evt = make_event(json!({"self_id": "bot123"}));
        assert_eq!(evt.self_id_str(), Some("bot123".to_string()));
        // as_i64 returns None for string value
        assert_eq!(evt.self_id(), None);
    }

    #[test]
    fn test_value_to_lossless_id() {
        assert_eq!(value_to_lossless_id(&json!(12345)), "12345");
        assert_eq!(value_to_lossless_id(&json!("abc")), "abc");
        assert_eq!(value_to_lossless_id(&json!(null)), "null");
    }

    #[test]
    fn test_is_poke_self() {
        let evt = make_event(json!({"self_id": 10001, "target_id": 10001}));
        assert!(evt.is_poke_self());

        let evt = make_event(json!({"self_id": 10001, "target_id": 99999}));
        assert!(!evt.is_poke_self());

        let evt = make_event(json!({"self_id": 10001}));
        assert!(!evt.is_poke_self());
    }

    #[test]
    fn test_qq_avatar_urls() {
        assert_eq!(
            qq_avatar_url("12345", 640),
            "https://q1.qlogo.cn/g?b=qq&nk=12345&s=640"
        );
        assert_eq!(
            qq_group_avatar_url("67890", 100),
            "https://p.qlogo.cn/gh/67890/67890/100"
        );
    }
}
