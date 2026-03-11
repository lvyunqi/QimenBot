//! Typed DTOs for OneBot 11 events and helper functions for parsing raw JSON into them.
//!
//! Each struct mirrors a specific OneBot 11 event payload. The `try_parse_*`
//! functions check the `post_type` / `message_type` / `notice_type` fields
//! before attempting deserialization.

use serde::{Deserialize, Serialize};

// ─── Message Events ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateMessageEvent {
    pub self_id: i64,
    pub user_id: i64,
    pub message_id: i32,
    pub message: serde_json::Value, // Can be string or array
    #[serde(default)]
    pub raw_message: Option<String>,
    #[serde(default)]
    pub font: Option<i32>,
    pub sender: MessageSender,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessageEvent {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    pub message_id: i32,
    pub message: serde_json::Value,
    #[serde(default)]
    pub raw_message: Option<String>,
    #[serde(default)]
    pub font: Option<i32>,
    pub sender: GroupMessageSender,
    #[serde(default)]
    pub anonymous: Option<AnonymousSender>,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSender {
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub sex: Option<String>,
    #[serde(default)]
    pub age: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMessageSender {
    #[serde(default)]
    pub user_id: Option<i64>,
    #[serde(default)]
    pub nickname: Option<String>,
    #[serde(default)]
    pub card: Option<String>,
    #[serde(default)]
    pub sex: Option<String>,
    #[serde(default)]
    pub age: Option<i32>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub level: Option<String>,
    #[serde(default)]
    pub role: Option<String>, // "owner", "admin", "member"
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnonymousSender {
    pub id: i64,
    pub name: String,
    pub flag: String,
}

// ─── Notice Events ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupUploadNotice {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    pub file: UploadFile,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadFile {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub size: i64,
    #[serde(default)]
    pub busid: Option<i64>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAdminNotice {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    pub sub_type: String, // "set" or "unset"
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupMemberChangeNotice {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub sub_type: String, // "leave", "kick", "kick_me", "approve", "invite"
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupBanNotice {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub duration: i64, // 0 = lift ban
    pub sub_type: String, // "ban" or "lift_ban"
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendAddNotice {
    pub self_id: i64,
    pub user_id: i64,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupRecallNotice {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub operator_id: Option<i64>,
    pub message_id: i64,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendRecallNotice {
    pub self_id: i64,
    pub user_id: i64,
    pub message_id: i64,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokeNotice {
    pub self_id: i64,
    pub user_id: i64,
    pub target_id: i64,
    #[serde(default)]
    pub group_id: Option<i64>, // None for private poke
    pub time: i64,
}

// ─── Request Events ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendAddRequest {
    pub self_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub comment: Option<String>,
    pub flag: String,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAddRequest {
    pub self_id: i64,
    pub group_id: i64,
    pub user_id: i64,
    #[serde(default)]
    pub comment: Option<String>,
    pub flag: String,
    pub sub_type: String, // "add" or "invite"
    pub time: i64,
}

// ─── Meta Events ───

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatMeta {
    pub self_id: i64,
    pub status: serde_json::Value,
    pub interval: i64,
    pub time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleMeta {
    pub self_id: i64,
    pub sub_type: String, // "connect", "enable", "disable"
    pub time: i64,
}

// ─── Parsing helper ───

/// Try to parse a raw JSON event into a specific typed event.
/// Returns None if the JSON doesn't match the expected structure.
pub fn try_parse_private_message(raw: &serde_json::Value) -> Option<PrivateMessageEvent> {
    if raw.get("post_type")?.as_str()? != "message" {
        return None;
    }
    if raw.get("message_type")?.as_str()? != "private" {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

/// Try to parse a raw JSON event as a group message event.
pub fn try_parse_group_message(raw: &serde_json::Value) -> Option<GroupMessageEvent> {
    if raw.get("post_type")?.as_str()? != "message" {
        return None;
    }
    if raw.get("message_type")?.as_str()? != "group" {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

/// Try to parse a raw JSON event as a poke notice.
pub fn try_parse_poke_notice(raw: &serde_json::Value) -> Option<PokeNotice> {
    if raw.get("post_type")?.as_str()? != "notice" {
        return None;
    }
    if raw.get("notice_type")?.as_str()? != "notify" {
        return None;
    }
    if raw.get("sub_type")?.as_str()? != "poke" {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

/// Try to parse a raw JSON event as a friend add request.
pub fn try_parse_friend_request(raw: &serde_json::Value) -> Option<FriendAddRequest> {
    if raw.get("post_type")?.as_str()? != "request" {
        return None;
    }
    if raw.get("request_type")?.as_str()? != "friend" {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}

/// Try to parse a raw JSON event as a group add/invite request.
pub fn try_parse_group_request(raw: &serde_json::Value) -> Option<GroupAddRequest> {
    if raw.get("post_type")?.as_str()? != "request" {
        return None;
    }
    if raw.get("request_type")?.as_str()? != "group" {
        return None;
    }
    serde_json::from_value(raw.clone()).ok()
}
