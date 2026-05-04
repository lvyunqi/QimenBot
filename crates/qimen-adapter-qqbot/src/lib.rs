use async_trait::async_trait;
use qimen_error::{QimenError, Result};
use qimen_message::{Message, Segment};
use qimen_protocol_core::{
    ActionStatus, ActorRef, CapabilitySet, ChatRef, EventKind, IncomingPacket,
    NormalizedActionRequest, NormalizedActionResponse, NormalizedEvent, OutgoingPacket,
    ProtocolAdapter, ProtocolId, QuickOpPatch, TransportMode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Default)]
pub struct QqBotAdapter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayDispatch {
    #[serde(rename = "op")]
    pub opcode: i64,
    #[serde(rename = "s")]
    pub sequence: Option<i64>,
    #[serde(rename = "t")]
    pub event_type: Option<String>,
    #[serde(rename = "id")]
    pub event_id: Option<String>,
    #[serde(rename = "d")]
    pub data: Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QqBotUser {
    pub id: Option<String>,
    pub username: Option<String>,
    pub bot: Option<bool>,
    pub avatar: Option<String>,
    pub user_openid: Option<String>,
    pub member_openid: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QqBotMember {
    pub nick: Option<String>,
    pub roles: Option<Vec<String>>,
    pub joined_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QqBotAttachment {
    pub content_type: Option<String>,
    pub filename: Option<String>,
    pub height: Option<i64>,
    pub width: Option<i64>,
    pub id: Option<String>,
    pub size: Option<i64>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QqBotMessagePayload {
    pub id: Option<String>,
    pub content: Option<String>,
    pub channel_id: Option<String>,
    pub guild_id: Option<String>,
    pub group_openid: Option<String>,
    pub author: Option<QqBotUser>,
    pub member: Option<QqBotMember>,
    #[serde(default)]
    pub mentions: Vec<QqBotUser>,
    #[serde(default)]
    pub attachments: Vec<QqBotAttachment>,
    pub seq: Option<i64>,
    pub seq_in_channel: Option<String>,
    pub msg_seq: Option<i64>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QqBotMessageKind {
    Group,
    C2c,
    ChannelMention,
    ChannelDirect,
    Channel,
}

#[async_trait]
impl ProtocolAdapter for QqBotAdapter {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::QqOfficial
    }

    fn supported_transports(&self) -> &'static [TransportMode] {
        const SUPPORTED: &[TransportMode] = &[TransportMode::Gateway];
        SUPPORTED
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet {
            features: vec![
                "gateway_events".to_string(),
                "send_channel_message".to_string(),
                "send_group_message".to_string(),
                "send_c2c_message".to_string(),
                "send_dms_message".to_string(),
                "send_markdown_message".to_string(),
                "send_keyboard_message".to_string(),
                "send_ark_message".to_string(),
                "send_embed_message".to_string(),
            ],
        }
    }

    async fn decode_event(&self, packet: IncomingPacket) -> Result<NormalizedEvent> {
        ensure_qqbot_gateway_dispatch(&packet.payload)?;
        let dispatch = parse_dispatch(&packet.payload)?;

        let event_type = dispatch
            .event_type
            .as_deref()
            .ok_or_else(|| QimenError::Protocol("qqbot dispatch missing event type".to_string()))?;

        let Some(message_kind) = message_kind(event_type) else {
            return Ok(normalized_non_message_event(packet, dispatch));
        };

        let message_payload: QqBotMessagePayload =
            serde_json::from_value(dispatch.data.clone()).map_err(QimenError::Json)?;
        let mut raw_json = qqbot_raw_message_json(&dispatch, &message_payload, message_kind);
        let mut extensions = qqbot_extensions(&dispatch, &message_payload);
        extensions.insert(
            "event_type".to_string(),
            Value::String(event_type.to_string()),
        );

        if let Some(message_type) = message_type(message_kind) {
            raw_json.insert(
                "message_type".to_string(),
                Value::String(message_type.to_string()),
            );
        }

        Ok(NormalizedEvent {
            protocol: ProtocolId::QqOfficial,
            bot_instance: packet.bot_instance,
            transport_mode: packet.transport_mode,
            time: None,
            kind: EventKind::Message,
            message: Some(message_from_qqbot(&message_payload)),
            actor: actor_from_message(&message_payload, message_kind),
            chat: chat_from_message(&message_payload, message_kind),
            raw_json: Value::Object(raw_json),
            raw_bytes: packet.raw_bytes,
            extensions,
        })
    }

    async fn decode_action_response(
        &self,
        packet: IncomingPacket,
    ) -> Result<NormalizedActionResponse> {
        let retcode = packet
            .payload
            .get("code")
            .or_else(|| packet.payload.get("retcode"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let status = if retcode == 0 {
            ActionStatus::Ok
        } else {
            ActionStatus::Failed
        };

        Ok(NormalizedActionResponse {
            protocol: ProtocolId::QqOfficial,
            bot_instance: packet.bot_instance,
            status,
            retcode,
            data: packet.payload.get("data").cloned().unwrap_or(Value::Null),
            echo: packet.payload.get("echo").cloned(),
            latency_ms: 0,
            raw_json: packet.payload,
        })
    }

    async fn encode_action(&self, req: &NormalizedActionRequest) -> Result<OutgoingPacket> {
        let payload = match req.action.as_str() {
            "send_msg" | "send_message" => encode_send_message_action(req)?,
            "upload_media" | "upload_file" => encode_upload_media_action(req)?,
            "recall_msg"
            | "delete_msg"
            | "delete_message"
            | "recall_message"
            | "recall_channel_msg"
            | "recall_channel_message" => encode_recall_channel_message_action(req)?,
            "send_channel_msg" | "send_channel_message" => build_qqbot_send_payload(
                req,
                "channel_message",
                "channel_id",
                req.params.get("channel_id").cloned(),
                None,
                false,
            ),
            "send_group_msg" | "send_group_message" => build_qqbot_send_payload(
                req,
                "group_message",
                "group_openid",
                req.params
                    .get("group_openid")
                    .or_else(|| req.params.get("group_id"))
                    .cloned(),
                Some(0),
                true,
            ),
            "send_private_msg" | "send_c2c_msg" | "send_c2c_message" => build_qqbot_send_payload(
                req,
                "c2c_message",
                "openid",
                req.params
                    .get("openid")
                    .or_else(|| req.params.get("user_id"))
                    .cloned(),
                Some(0),
                true,
            ),
            "send_dms" | "send_dms_message" => build_qqbot_send_payload(
                req,
                "dms_message",
                "guild_id",
                req.params.get("guild_id").cloned(),
                None,
                false,
            ),
            _ => {
                return Err(QimenError::Protocol(format!(
                    "unsupported qqbot action '{}'",
                    req.action
                )));
            }
        };

        Ok(OutgoingPacket { payload })
    }

    fn quick_op_from_event_and_patch(
        &self,
        event: &NormalizedEvent,
        patch: &QuickOpPatch,
    ) -> Result<Option<OutgoingPacket>> {
        let Some(reply_text) = patch.reply_text.as_deref() else {
            return Ok(None);
        };

        let route = match event.chat.as_ref().map(|chat| chat.kind.as_str()) {
            Some("group") => "group_message",
            Some("private") => "c2c_message",
            Some("channel") => "channel_message",
            Some("channel_private") => "dms_message",
            _ => return Ok(None),
        };

        Ok(Some(OutgoingPacket {
            payload: json!({
                "route": route,
                "content": reply_text,
                "msg_id": event.message_id_str(),
                "event_id": event.extensions.get("event_id").cloned(),
                "msg_seq": event.extensions.get("msg_seq").cloned(),
                "target": event.chat.as_ref().map(|chat| chat.id.clone()),
            }),
        }))
    }
}

pub fn ensure_qqbot_gateway_dispatch(payload: &Value) -> Result<()> {
    if payload.get("op").is_none() || payload.get("d").is_none() {
        return Err(QimenError::Protocol(
            "payload is not a recognized QQ official Gateway dispatch".to_string(),
        ));
    }
    Ok(())
}

fn encode_upload_media_action(req: &NormalizedActionRequest) -> Result<Value> {
    let target = if let Some(group_openid) = req
        .params
        .get("group_openid")
        .or_else(|| req.params.get("group_id"))
        .cloned()
    {
        ("group_file", "group_openid", group_openid)
    } else if let Some(openid) = req
        .params
        .get("openid")
        .or_else(|| req.params.get("user_id"))
        .cloned()
    {
        ("c2c_file", "openid", openid)
    } else {
        return Err(QimenError::Protocol(
            "qqbot upload_media action requires group_openid/group_id or openid/user_id"
                .to_string(),
        ));
    };

    let file_type = req
        .params
        .get("file_type")
        .and_then(Value::as_i64)
        .or_else(|| {
            req.params
                .get("media_type")
                .and_then(Value::as_str)
                .and_then(qqbot_file_type)
        })
        .unwrap_or(1);
    let url = req
        .params
        .get("url")
        .or_else(|| req.params.get("file"))
        .cloned()
        .ok_or_else(|| {
            QimenError::Protocol("qqbot upload_media action requires url".to_string())
        })?;
    let srv_send_msg = req
        .params
        .get("srv_send_msg")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(json!({
        "route": target.0,
        target.1: target.2,
        "file_type": file_type,
        "url": url,
        "srv_send_msg": srv_send_msg,
    }))
}

fn encode_recall_channel_message_action(req: &NormalizedActionRequest) -> Result<Value> {
    let channel_id = req.params.get("channel_id").cloned().ok_or_else(|| {
        QimenError::Protocol("qqbot recall action requires channel_id".to_string())
    })?;
    let message_id = req
        .params
        .get("message_id")
        .or_else(|| req.params.get("msg_id"))
        .cloned()
        .ok_or_else(|| {
            QimenError::Protocol("qqbot recall action requires message_id/msg_id".to_string())
        })?;
    let hidetip = req
        .params
        .get("hidetip")
        .or_else(|| req.params.get("hide_tip"))
        .and_then(Value::as_bool)
        .unwrap_or(false);

    Ok(json!({
        "route": "channel_recall_message",
        "channel_id": channel_id,
        "message_id": message_id,
        "hidetip": hidetip,
    }))
}

fn qqbot_file_type(media_type: &str) -> Option<i64> {
    match media_type {
        "image" => Some(1),
        "video" => Some(2),
        "record" | "audio" | "voice" => Some(3),
        "file" => Some(4),
        _ => None,
    }
}

pub fn qq_official_intent_bit(intent: &str) -> Result<u64> {
    let bit = match intent {
        "guilds" => 1_u64 << 0,
        "guild_members" => 1_u64 << 1,
        "guild_messages" => 1_u64 << 9,
        "guild_message_reactions" => 1_u64 << 10,
        "direct_message" => 1_u64 << 12,
        "open_forum_event" => 1_u64 << 18,
        "audio_or_live_channel_member" => 1_u64 << 19,
        "public_messages" => 1_u64 << 25,
        "interaction" => 1_u64 << 26,
        "message_audit" => 1_u64 << 27,
        "forums" => 1_u64 << 28,
        "audio_action" => 1_u64 << 29,
        "public_guild_messages" => 1_u64 << 30,
        other => {
            return Err(QimenError::Protocol(format!(
                "unknown qq-official intent '{}'",
                other
            )));
        }
    };
    Ok(bit)
}

pub fn qq_official_intents_value(intents: &[String]) -> Result<u64> {
    let mut value = 0_u64;
    for intent in intents {
        value |= qq_official_intent_bit(intent)?;
    }
    Ok(value)
}

fn parse_dispatch(payload: &Value) -> Result<GatewayDispatch> {
    serde_json::from_value(payload.clone()).map_err(QimenError::Json)
}

fn normalized_non_message_event(
    packet: IncomingPacket,
    dispatch: GatewayDispatch,
) -> NormalizedEvent {
    let event_type = dispatch.event_type.clone();
    let event_kind = event_type
        .as_deref()
        .map(qqbot_non_message_event_kind)
        .unwrap_or_else(|| EventKind::Internal("unknown".to_string()));
    let raw_json = qqbot_non_message_raw_json(&dispatch, &event_kind);
    let mut extensions = Map::new();
    if let Some(event_type) = dispatch.event_type.clone() {
        extensions.insert("event_type".to_string(), Value::String(event_type));
    }
    if let Some(event_id) = dispatch.event_id.clone() {
        extensions.insert("event_id".to_string(), Value::String(event_id));
    }
    if let Some(sequence) = dispatch.sequence {
        extensions.insert("sequence".to_string(), json!(sequence));
    }
    copy_qqbot_context_extensions(&dispatch.data, &mut extensions);

    NormalizedEvent {
        protocol: ProtocolId::QqOfficial,
        bot_instance: packet.bot_instance,
        transport_mode: packet.transport_mode,
        time: None,
        kind: event_kind,
        message: None,
        actor: actor_from_non_message(&dispatch.data),
        chat: chat_from_non_message(&dispatch.data, dispatch.event_type.as_deref()),
        raw_json: Value::Object(raw_json),
        raw_bytes: packet.raw_bytes,
        extensions,
    }
}

fn qqbot_non_message_event_kind(event_type: &str) -> EventKind {
    match event_type {
        "READY" | "RESUMED" => EventKind::Meta,
        event if qqbot_notice_type(event).is_some() => EventKind::Notice,
        other => EventKind::Internal(other.to_string()),
    }
}

fn qqbot_notice_type(event_type: &str) -> Option<&'static str> {
    match event_type {
        "GUILD_CREATE" => Some("guild_create"),
        "GUILD_UPDATE" => Some("guild_update"),
        "GUILD_DELETE" => Some("guild_delete"),
        "CHANNEL_CREATE" => Some("channel_created"),
        "CHANNEL_UPDATE" => Some("channel_updated"),
        "CHANNEL_DELETE" => Some("channel_destroyed"),
        "GUILD_MEMBER_ADD" => Some("guild_member_add"),
        "GUILD_MEMBER_UPDATE" => Some("guild_member_update"),
        "GUILD_MEMBER_REMOVE" => Some("guild_member_remove"),
        "MESSAGE_DELETE" => Some("message_delete"),
        "PUBLIC_MESSAGE_DELETE" => Some("public_message_delete"),
        "DIRECT_MESSAGE_DELETE" => Some("direct_message_delete"),
        "MESSAGE_REACTION_ADD" => Some("message_reaction_add"),
        "MESSAGE_REACTION_REMOVE" => Some("message_reaction_remove"),
        "GROUP_ADD_ROBOT" => Some("group_add_robot"),
        "GROUP_DEL_ROBOT" => Some("group_del_robot"),
        "GROUP_MSG_REJECT" => Some("group_msg_reject"),
        "GROUP_MSG_RECEIVE" => Some("group_msg_receive"),
        "FRIEND_ADD" => Some("friend_add"),
        "FRIEND_DEL" => Some("friend_del"),
        "C2C_MSG_REJECT" => Some("c2c_msg_reject"),
        "C2C_MSG_RECEIVE" => Some("c2c_msg_receive"),
        "INTERACTION_CREATE" => Some("interaction_create"),
        "MESSAGE_AUDIT_PASS" => Some("message_audit_pass"),
        "MESSAGE_AUDIT_REJECT" => Some("message_audit_reject"),
        "AUDIO_START" => Some("audio_start"),
        "AUDIO_FINISH" => Some("audio_finish"),
        "AUDIO_ON_MIC" | "ON_MIC" => Some("audio_on_mic"),
        "AUDIO_OFF_MIC" | "OFF_MIC" => Some("audio_off_mic"),
        "AUDIO_OR_LIVE_CHANNEL_MEMBER_ENTER" => Some("audio_or_live_channel_member_enter"),
        "AUDIO_OR_LIVE_CHANNEL_MEMBER_EXIT" => Some("audio_or_live_channel_member_exit"),
        "FORUM_THREAD_CREATE" => Some("forum_thread_create"),
        "FORUM_THREAD_UPDATE" => Some("forum_thread_update"),
        "FORUM_THREAD_DELETE" => Some("forum_thread_delete"),
        "FORUM_POST_CREATE" => Some("forum_post_create"),
        "FORUM_POST_DELETE" => Some("forum_post_delete"),
        "FORUM_REPLY_CREATE" => Some("forum_reply_create"),
        "FORUM_REPLY_DELETE" => Some("forum_reply_delete"),
        "FORUM_PUBLISH_AUDIT_RESULT" => Some("forum_publish_audit_result"),
        "OPEN_FORUM_THREAD_CREATE" => Some("open_forum_thread_create"),
        "OPEN_FORUM_THREAD_UPDATE" => Some("open_forum_thread_update"),
        "OPEN_FORUM_THREAD_DELETE" => Some("open_forum_thread_delete"),
        "OPEN_FORUM_POST_CREATE" => Some("open_forum_post_create"),
        "OPEN_FORUM_POST_DELETE" => Some("open_forum_post_delete"),
        "OPEN_FORUM_REPLY_CREATE" => Some("open_forum_reply_create"),
        "OPEN_FORUM_REPLY_DELETE" => Some("open_forum_reply_delete"),
        _ => None,
    }
}

fn qqbot_non_message_raw_json(
    dispatch: &GatewayDispatch,
    event_kind: &EventKind,
) -> Map<String, Value> {
    let mut raw = Map::new();
    match event_kind {
        EventKind::Notice => {
            raw.insert("post_type".to_string(), Value::String("notice".to_string()));
            if let Some(event_type) = dispatch.event_type.as_deref()
                && let Some(notice_type) = qqbot_notice_type(event_type)
            {
                raw.insert(
                    "notice_type".to_string(),
                    Value::String(notice_type.to_string()),
                );
            }
        }
        EventKind::Meta => {
            raw.insert(
                "post_type".to_string(),
                Value::String("meta_event".to_string()),
            );
            raw.insert(
                "meta_event_type".to_string(),
                Value::String(
                    dispatch
                        .event_type
                        .as_deref()
                        .unwrap_or("unknown")
                        .to_ascii_lowercase(),
                ),
            );
        }
        EventKind::Internal(kind) => {
            raw.insert(
                "post_type".to_string(),
                Value::String("internal".to_string()),
            );
            raw.insert("internal_type".to_string(), Value::String(kind.clone()));
        }
        _ => {}
    }

    if let Some(event_type) = dispatch.event_type.clone() {
        raw.insert("event_type".to_string(), Value::String(event_type));
    }
    if let Some(event_id) = dispatch.event_id.clone() {
        raw.insert("event_id".to_string(), Value::String(event_id));
    }
    if let Some(sequence) = dispatch.sequence {
        raw.insert("sequence".to_string(), json!(sequence));
    }

    copy_qqbot_context_fields(&dispatch.data, &mut raw);
    raw.insert("qqbot_payload".to_string(), dispatch.data.clone());
    raw
}

fn actor_from_non_message(data: &Value) -> Option<ActorRef> {
    let id = data
        .get("op_member_openid")
        .or_else(|| data.get("group_member_openid"))
        .or_else(|| data.get("member_openid"))
        .or_else(|| data.get("openid"))
        .or_else(|| data.get("user_openid"))
        .or_else(|| data.get("user_id"))
        .or_else(|| data.get("operator_id"))
        .or_else(|| data.get("author").and_then(|author| author.get("id")))
        .or_else(|| data.get("user").and_then(|user| user.get("id")))
        .and_then(value_to_action_string)?;
    let display_name = data
        .get("user")
        .and_then(|user| user.get("username"))
        .or_else(|| data.get("author").and_then(|author| author.get("username")))
        .or_else(|| data.get("nick"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    Some(ActorRef { id, display_name })
}

fn chat_from_non_message(data: &Value, event_type: Option<&str>) -> Option<ChatRef> {
    if let Some(group_openid) = data.get("group_openid").and_then(value_to_action_string) {
        return Some(ChatRef {
            id: group_openid,
            kind: "group".to_string(),
        });
    }
    if let Some(openid) = data
        .get("openid")
        .or_else(|| data.get("user_openid"))
        .and_then(value_to_action_string)
    {
        return Some(ChatRef {
            id: openid,
            kind: "private".to_string(),
        });
    }
    if matches!(event_type, Some("DIRECT_MESSAGE_DELETE"))
        && let Some(guild_id) = data.get("guild_id").and_then(value_to_action_string)
    {
        return Some(ChatRef {
            id: guild_id,
            kind: "channel_private".to_string(),
        });
    }
    if let Some(channel_id) = data.get("channel_id").and_then(value_to_action_string) {
        return Some(ChatRef {
            id: channel_id,
            kind: "channel".to_string(),
        });
    }
    data.get("guild_id")
        .and_then(value_to_action_string)
        .map(|guild_id| ChatRef {
            id: guild_id,
            kind: "guild".to_string(),
        })
}

fn copy_qqbot_context_extensions(data: &Value, extensions: &mut Map<String, Value>) {
    for key in [
        "guild_id",
        "channel_id",
        "group_openid",
        "openid",
        "user_openid",
        "group_member_openid",
        "op_member_openid",
        "timestamp",
        "version",
    ] {
        if let Some(value) = data.get(key).cloned() {
            extensions.insert(key.to_string(), value);
        }
    }
}

fn copy_qqbot_context_fields(data: &Value, raw: &mut Map<String, Value>) {
    for key in [
        "guild_id",
        "channel_id",
        "group_openid",
        "openid",
        "user_openid",
        "group_member_openid",
        "op_member_openid",
        "timestamp",
        "version",
    ] {
        if let Some(value) = data.get(key).cloned() {
            raw.insert(key.to_string(), value);
        }
    }

    if let Some(group_openid) = data.get("group_openid").cloned() {
        raw.insert("group_id".to_string(), group_openid);
    }
    let user_id = data
        .get("op_member_openid")
        .or_else(|| data.get("group_member_openid"))
        .or_else(|| data.get("member_openid"))
        .or_else(|| data.get("openid"))
        .or_else(|| data.get("user_openid"))
        .or_else(|| data.get("user_id"))
        .or_else(|| data.get("author").and_then(|author| author.get("id")))
        .or_else(|| data.get("user").and_then(|user| user.get("id")))
        .cloned();
    if let Some(user_id) = user_id {
        raw.insert("user_id".to_string(), user_id);
    }
    let message_id = data
        .get("message_id")
        .or_else(|| data.get("msg_id"))
        .or_else(|| data.get("id"))
        .or_else(|| data.get("target").and_then(|target| target.get("id")))
        .cloned();
    if let Some(message_id) = message_id {
        raw.insert("message_id".to_string(), message_id);
    }
}

fn message_kind(event_type: &str) -> Option<QqBotMessageKind> {
    match event_type {
        "GROUP_AT_MESSAGE_CREATE" => Some(QqBotMessageKind::Group),
        "C2C_MESSAGE_CREATE" => Some(QqBotMessageKind::C2c),
        "AT_MESSAGE_CREATE" => Some(QqBotMessageKind::ChannelMention),
        "DIRECT_MESSAGE_CREATE" => Some(QqBotMessageKind::ChannelDirect),
        "MESSAGE_CREATE" => Some(QqBotMessageKind::Channel),
        _ => None,
    }
}

fn message_type(kind: QqBotMessageKind) -> Option<&'static str> {
    match kind {
        QqBotMessageKind::Group => Some("group"),
        QqBotMessageKind::C2c => Some("private"),
        QqBotMessageKind::ChannelMention | QqBotMessageKind::Channel => Some("channel"),
        QqBotMessageKind::ChannelDirect => Some("channel_private"),
    }
}

fn actor_from_message(payload: &QqBotMessagePayload, kind: QqBotMessageKind) -> Option<ActorRef> {
    let author = payload.author.as_ref()?;
    let id = match kind {
        QqBotMessageKind::Group => author.member_openid.as_deref(),
        QqBotMessageKind::C2c => author.user_openid.as_deref(),
        QqBotMessageKind::ChannelMention
        | QqBotMessageKind::ChannelDirect
        | QqBotMessageKind::Channel => author.id.as_deref(),
    }?;

    let display_name = payload
        .member
        .as_ref()
        .and_then(|member| member.nick.clone())
        .or_else(|| author.username.clone());

    Some(ActorRef {
        id: id.to_string(),
        display_name,
    })
}

fn chat_from_message(payload: &QqBotMessagePayload, kind: QqBotMessageKind) -> Option<ChatRef> {
    match kind {
        QqBotMessageKind::Group => payload.group_openid.as_ref().map(|id| ChatRef {
            id: id.clone(),
            kind: "group".to_string(),
        }),
        QqBotMessageKind::C2c => payload
            .author
            .as_ref()
            .and_then(|author| author.user_openid.clone())
            .map(|id| ChatRef {
                id,
                kind: "private".to_string(),
            }),
        QqBotMessageKind::ChannelMention | QqBotMessageKind::Channel => {
            payload.channel_id.as_ref().map(|id| ChatRef {
                id: id.clone(),
                kind: "channel".to_string(),
            })
        }
        QqBotMessageKind::ChannelDirect => payload.guild_id.as_ref().map(|id| ChatRef {
            id: id.clone(),
            kind: "channel_private".to_string(),
        }),
    }
}

fn message_from_qqbot(payload: &QqBotMessagePayload) -> Message {
    let mut segments = Vec::new();
    if let Some(content) = payload.content.as_deref()
        && !content.is_empty()
    {
        segments.push(Segment::text(content));
    }

    for attachment in &payload.attachments {
        let Some(url) = attachment.url.clone() else {
            continue;
        };
        let content_type = attachment.content_type.as_deref().unwrap_or_default();
        let kind = if content_type.starts_with("image/") {
            "image"
        } else if content_type.starts_with("audio/") {
            "record"
        } else if content_type.starts_with("video/") {
            "video"
        } else {
            "file"
        };
        let mut segment = Segment::new(kind).with("url", Value::String(url.clone()));
        if kind == "image" || kind == "record" || kind == "video" {
            segment = segment.with("file", Value::String(url));
        }
        if let Some(filename) = attachment.filename.clone() {
            segment = segment.with("filename", Value::String(filename));
        }
        segments.push(segment);
    }

    Message::from_segments(segments)
}

fn qqbot_raw_message_json(
    dispatch: &GatewayDispatch,
    payload: &QqBotMessagePayload,
    kind: QqBotMessageKind,
) -> Map<String, Value> {
    let mut raw = Map::new();
    raw.insert(
        "post_type".to_string(),
        Value::String("message".to_string()),
    );
    raw.insert(
        "message_id".to_string(),
        payload.id.clone().map(Value::String).unwrap_or(Value::Null),
    );
    raw.insert(
        "raw_message".to_string(),
        payload
            .content
            .clone()
            .map(Value::String)
            .unwrap_or_default(),
    );
    raw.insert(
        "message".to_string(),
        payload
            .content
            .clone()
            .map(Value::String)
            .unwrap_or_default(),
    );
    if let Some(event_id) = dispatch.event_id.clone() {
        raw.insert("event_id".to_string(), Value::String(event_id));
    }
    if let Some(event_type) = dispatch.event_type.clone() {
        raw.insert("event_type".to_string(), Value::String(event_type));
    }
    if let Some(sequence) = dispatch.sequence {
        raw.insert("sequence".to_string(), json!(sequence));
    }
    if let Some(group_openid) = payload.group_openid.clone() {
        raw.insert("group_openid".to_string(), Value::String(group_openid));
    }
    if let Some(channel_id) = payload.channel_id.clone() {
        raw.insert("channel_id".to_string(), Value::String(channel_id));
    }
    if let Some(guild_id) = payload.guild_id.clone() {
        raw.insert("guild_id".to_string(), Value::String(guild_id));
    }
    if let Some(msg_seq) = payload.msg_seq.or(payload.seq) {
        raw.insert("msg_seq".to_string(), json!(msg_seq));
    }
    if let Some(timestamp) = payload.timestamp.clone() {
        raw.insert("timestamp".to_string(), Value::String(timestamp));
    }
    if let Some(author) = payload.author.as_ref() {
        let actor_id = match kind {
            QqBotMessageKind::Group => author.member_openid.clone(),
            QqBotMessageKind::C2c => author.user_openid.clone(),
            QqBotMessageKind::ChannelMention
            | QqBotMessageKind::ChannelDirect
            | QqBotMessageKind::Channel => author.id.clone(),
        };
        if let Some(user_id) = actor_id {
            raw.insert("user_id".to_string(), Value::String(user_id));
        }
        raw.insert(
            "sender".to_string(),
            json!({
                "nickname": payload.member.as_ref().and_then(|member| member.nick.clone()).or_else(|| author.username.clone()),
                "openid": author.user_openid.clone().or_else(|| author.member_openid.clone()),
                "id": author.id,
            }),
        );
    }
    raw.insert("qqbot_payload".to_string(), json!(payload));
    raw
}

fn qqbot_extensions(
    dispatch: &GatewayDispatch,
    payload: &QqBotMessagePayload,
) -> Map<String, Value> {
    let mut extensions = Map::new();
    if let Some(event_id) = dispatch.event_id.clone() {
        extensions.insert("event_id".to_string(), Value::String(event_id));
    }
    if let Some(sequence) = dispatch.sequence {
        extensions.insert("sequence".to_string(), json!(sequence));
    }
    if let Some(group_openid) = payload.group_openid.clone() {
        extensions.insert("group_openid".to_string(), Value::String(group_openid));
    }
    if let Some(channel_id) = payload.channel_id.clone() {
        extensions.insert("channel_id".to_string(), Value::String(channel_id));
    }
    if let Some(guild_id) = payload.guild_id.clone() {
        extensions.insert("guild_id".to_string(), Value::String(guild_id));
    }
    if let Some(msg_seq) = payload.msg_seq.or(payload.seq) {
        extensions.insert("msg_seq".to_string(), json!(msg_seq));
    }
    extensions
}

fn encode_send_message_action(req: &NormalizedActionRequest) -> Result<Value> {
    if req.params.get("group_openid").is_some() || req.params.get("group_id").is_some() {
        return Ok(build_qqbot_send_payload(
            req,
            "group_message",
            "group_openid",
            req.params
                .get("group_openid")
                .or_else(|| req.params.get("group_id"))
                .cloned(),
            Some(0),
            true,
        ));
    }

    if req.params.get("openid").is_some() || req.params.get("user_id").is_some() {
        return Ok(build_qqbot_send_payload(
            req,
            "c2c_message",
            "openid",
            req.params
                .get("openid")
                .or_else(|| req.params.get("user_id"))
                .cloned(),
            Some(0),
            true,
        ));
    }

    if req.params.get("channel_id").is_some() {
        return Ok(build_qqbot_send_payload(
            req,
            "channel_message",
            "channel_id",
            req.params.get("channel_id").cloned(),
            None,
            false,
        ));
    }

    if req.params.get("guild_id").is_some() {
        return Ok(build_qqbot_send_payload(
            req,
            "dms_message",
            "guild_id",
            req.params.get("guild_id").cloned(),
            None,
            false,
        ));
    }

    Err(QimenError::Protocol(
        "qqbot send_msg action requires group_openid, openid/user_id, channel_id, or guild_id"
            .to_string(),
    ))
}

fn build_qqbot_send_payload(
    req: &NormalizedActionRequest,
    route: &str,
    target_key: &str,
    target_value: Option<Value>,
    default_msg_type: Option<i64>,
    include_msg_seq: bool,
) -> Value {
    let mut message = encode_action_message(req);
    if matches!(route, "group_message" | "c2c_message") && message.media.is_none() {
        if message.upload.is_none()
            && let Some(image) = message.image.as_deref()
            && is_remote_media_url(image)
        {
            message.upload = Some(EncodedMediaUpload {
                file_type: 1,
                url: image.to_string(),
                srv_send_msg: false,
            });
        }
        if message.upload.is_some() && message.markdown.is_none() && message.msg_type.is_none() {
            message.msg_type = Some(7);
        }
    }
    let mut payload = Map::new();
    payload.insert("route".to_string(), Value::String(route.to_string()));
    payload.insert(target_key.to_string(), target_value.unwrap_or(Value::Null));

    let explicit_msg_type = req.params.get("msg_type").and_then(Value::as_i64);
    let inferred_msg_type = message.msg_type.or(default_msg_type);
    if let Some(msg_type) = explicit_msg_type.or(inferred_msg_type) {
        payload.insert("msg_type".to_string(), json!(msg_type));
    }
    if let Some(content) = message.content {
        payload.insert("content".to_string(), Value::String(content));
    }
    if let Some(msg_id) = req.params.get("msg_id").cloned().or(message.reply_msg_id) {
        payload.insert("msg_id".to_string(), msg_id);
    }
    if let Some(msg_seq) = req
        .params
        .get("msg_seq")
        .cloned()
        .or_else(|| include_msg_seq.then_some(json!(1)))
    {
        payload.insert("msg_seq".to_string(), msg_seq);
    }
    if let Some(event_id) = req.params.get("event_id").cloned() {
        payload.insert("event_id".to_string(), event_id);
    }
    if let Some(value) = req.params.get("srv_send_msg").and_then(Value::as_bool)
        && let Some(upload) = message.upload.as_mut()
    {
        upload.srv_send_msg = value;
    }
    if let Some(markdown) = message.markdown {
        payload.insert("markdown".to_string(), markdown);
    }
    if let Some(keyboard) = message.keyboard {
        payload.insert("keyboard".to_string(), keyboard);
    }
    if let Some(ark) = message.ark {
        payload.insert("ark".to_string(), ark);
    }
    if let Some(embed) = message.embed {
        payload.insert("embed".to_string(), embed);
    }
    if let Some(media) = message.media {
        payload.insert("media".to_string(), media);
    }
    if let Some(image) = message.image {
        payload.insert("image".to_string(), Value::String(image));
    }
    if let Some(upload) = message.upload {
        payload.insert(
            "media_upload".to_string(),
            json!({
                "file_type": upload.file_type,
                "url": upload.url,
                "srv_send_msg": upload.srv_send_msg,
            }),
        );
    }
    if !message.unsupported_segments.is_empty() {
        payload.insert(
            "unsupported_segments".to_string(),
            json!(message.unsupported_segments),
        );
    }

    Value::Object(payload)
}

#[derive(Debug, Default)]
struct EncodedActionMessage {
    msg_type: Option<i64>,
    content: Option<String>,
    markdown: Option<Value>,
    keyboard: Option<Value>,
    ark: Option<Value>,
    embed: Option<Value>,
    media: Option<Value>,
    image: Option<String>,
    upload: Option<EncodedMediaUpload>,
    reply_msg_id: Option<Value>,
    unsupported_segments: Vec<String>,
}

#[derive(Debug, Clone)]
struct EncodedMediaUpload {
    file_type: i64,
    url: String,
    srv_send_msg: bool,
}

impl EncodedActionMessage {
    fn merge_missing(&mut self, other: Self) {
        if self.msg_type.is_none() {
            self.msg_type = other.msg_type;
        }
        if self.content.is_none() {
            self.content = other.content;
        }
        if self.markdown.is_none() {
            self.markdown = other.markdown;
        }
        if self.keyboard.is_none() {
            self.keyboard = other.keyboard;
        }
        if self.ark.is_none() {
            self.ark = other.ark;
        }
        if self.embed.is_none() {
            self.embed = other.embed;
        }
        if self.media.is_none() {
            self.media = other.media;
        }
        if self.image.is_none() {
            self.image = other.image;
        }
        if self.upload.is_none() {
            self.upload = other.upload;
        }
        if self.reply_msg_id.is_none() {
            self.reply_msg_id = other.reply_msg_id;
        }
        self.unsupported_segments.extend(other.unsupported_segments);
    }
}

fn encode_action_message(req: &NormalizedActionRequest) -> EncodedActionMessage {
    let mut encoded = EncodedActionMessage {
        msg_type: req.params.get("msg_type").and_then(Value::as_i64),
        ..EncodedActionMessage::default()
    };

    if let Some(content) = req.params.get("content").and_then(value_to_action_string) {
        encoded.content = Some(content);
    }
    if let Some(markdown) = req.params.get("markdown") {
        encoded.markdown = Some(normalize_markdown_payload(markdown));
    }
    if let Some(keyboard) = req.params.get("keyboard") {
        encoded.keyboard = Some(normalize_keyboard_payload(keyboard));
    }
    if let Some(ark) = req.params.get("ark") {
        encoded.ark = Some(ark.clone());
    }
    if let Some(embed) = req.params.get("embed") {
        encoded.embed = Some(embed.clone());
    }
    if let Some(media) = req.params.get("media") {
        encoded.media = Some(media.clone());
        if encoded.msg_type.is_none() {
            encoded.msg_type = Some(7);
        }
    }
    if let Some(image) = req.params.get("image").and_then(value_to_action_string) {
        encoded.image = Some(image);
    }
    if let Some(message) = req.params.get("message") {
        encoded.merge_missing(encode_message_value(message));
    }
    infer_official_msg_type(&mut encoded);

    encoded
}

fn encode_message_value(value: &Value) -> EncodedActionMessage {
    match value {
        Value::Null => EncodedActionMessage::default(),
        Value::String(text) => EncodedActionMessage {
            content: Some(text.clone()),
            ..EncodedActionMessage::default()
        },
        Value::Number(number) => EncodedActionMessage {
            content: Some(number.to_string()),
            ..EncodedActionMessage::default()
        },
        Value::Bool(flag) => EncodedActionMessage {
            content: Some(flag.to_string()),
            ..EncodedActionMessage::default()
        },
        Value::Array(_) | Value::Object(_) => {
            let message = Message::from_onebot_value(value);
            encode_message_segments(&message)
        }
    }
}

fn encode_message_segments(message: &Message) -> EncodedActionMessage {
    let mut encoded = EncodedActionMessage::default();
    let mut content = String::new();

    for segment in &message.segments {
        match segment.kind.as_str() {
            "text" => {
                if let Some(text) = segment.data_str("text") {
                    content.push_str(text);
                }
            }
            "markdown" => {
                if encoded.markdown.is_none() {
                    encoded.markdown = Some(markdown_segment_payload(segment));
                }
            }
            "keyboard" => {
                if encoded.keyboard.is_none() {
                    encoded.keyboard = Some(keyboard_segment_payload(segment));
                }
            }
            "ark" => {
                if encoded.ark.is_none() {
                    encoded.ark = Some(rich_object_segment_payload(segment));
                }
            }
            "embed" => {
                if encoded.embed.is_none() {
                    encoded.embed = Some(rich_object_segment_payload(segment));
                }
            }
            "reply" => {
                if encoded.reply_msg_id.is_none() {
                    encoded.reply_msg_id = segment.data.get("id").cloned();
                }
            }
            "at" => {
                let target = segment
                    .at_target()
                    .map(|value| format!("@{value}"))
                    .unwrap_or_else(|| "@".to_string());
                append_fallback_text(&mut content, &target);
                encoded.unsupported_segments.push(segment.kind.clone());
            }
            "face" => {
                let label = segment
                    .data_lossless("id")
                    .map(|id| format!("face:{id}"))
                    .unwrap_or_else(|| "face".to_string());
                append_fallback_segment(&mut content, &label);
                encoded.unsupported_segments.push(segment.kind.clone());
            }
            "image" | "record" | "video" | "file" => {
                if encoded.upload.is_none()
                    && let Some(upload) = media_upload_from_segment(segment)
                {
                    if segment.kind == "image" {
                        encoded.image = Some(upload.url.clone());
                    }
                    encoded.upload = Some(upload);
                } else {
                    append_fallback_segment(&mut content, segment.kind.as_str());
                    encoded.unsupported_segments.push(segment.kind.clone());
                }
            }
            other => {
                append_fallback_segment(&mut content, &format!("unsupported:{other}"));
                encoded.unsupported_segments.push(segment.kind.clone());
            }
        }
    }

    if !content.is_empty() {
        encoded.content = Some(content);
    }
    infer_official_msg_type(&mut encoded);

    encoded
}

fn infer_official_msg_type(encoded: &mut EncodedActionMessage) {
    if encoded.msg_type.is_some() {
        return;
    }

    encoded.msg_type = if encoded.markdown.is_some() {
        Some(2)
    } else if encoded.ark.is_some() {
        Some(3)
    } else if encoded.embed.is_some() {
        Some(4)
    } else if encoded.media.is_some() {
        Some(7)
    } else {
        None
    };
}

fn markdown_segment_payload(segment: &Segment) -> Value {
    match segment.data.get("content") {
        Some(Value::String(content)) => json!({ "content": content }),
        _ => Value::Object(segment.data.clone()),
    }
}

fn keyboard_segment_payload(segment: &Segment) -> Value {
    normalize_keyboard_payload(&Value::Object(segment.data.clone()))
}

fn rich_object_segment_payload(segment: &Segment) -> Value {
    match segment.data.get("content") {
        Some(Value::Object(_)) => segment.data.get("content").cloned().unwrap_or(Value::Null),
        _ => Value::Object(segment.data.clone()),
    }
}

fn normalize_markdown_payload(value: &Value) -> Value {
    match value {
        Value::String(content) => json!({ "content": content }),
        Value::Object(_) => value.clone(),
        other => json!({ "content": other.to_string() }),
    }
}

fn normalize_keyboard_payload(value: &Value) -> Value {
    match value {
        Value::String(id) => json!({ "id": id }),
        Value::Object(map) if map.contains_key("id") => value.clone(),
        Value::Object(map) => match map.get("content") {
            Some(content) => {
                let mut normalized = map.clone();
                normalized.insert(
                    "content".to_string(),
                    normalize_keyboard_content_payload(content),
                );
                Value::Object(normalized)
            }
            None if map.contains_key("rows") => {
                json!({ "content": normalize_keyboard_content_payload(value) })
            }
            _ => value.clone(),
        },
        _ => value.clone(),
    }
}

fn normalize_keyboard_content_payload(value: &Value) -> Value {
    let Some(rows) = value.get("rows").and_then(Value::as_array) else {
        return value.clone();
    };

    let rows = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let Some(buttons) = row.get("buttons").and_then(Value::as_array) else {
                return row.clone();
            };
            let buttons = buttons
                .iter()
                .enumerate()
                .map(|(button_index, button)| {
                    normalize_keyboard_button_payload(button, row_index, button_index)
                })
                .collect::<Vec<_>>();
            let mut normalized = row.as_object().cloned().unwrap_or_default();
            normalized.insert("buttons".to_string(), Value::Array(buttons));
            Value::Object(normalized)
        })
        .collect::<Vec<_>>();

    json!({ "rows": rows })
}

fn normalize_keyboard_button_payload(
    value: &Value,
    row_index: usize,
    button_index: usize,
) -> Value {
    let Some(button) = value.as_object() else {
        return value.clone();
    };
    if button.contains_key("render_data") && button.contains_key("action") {
        return value.clone();
    }

    let label = button
        .get("label")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let visited_label = button
        .get("visited_label")
        .and_then(Value::as_str)
        .unwrap_or(label);
    let style = button.get("style").and_then(Value::as_i64).unwrap_or(0);
    let action_type = button
        .get("action_type")
        .and_then(Value::as_i64)
        .unwrap_or(2);
    let action_data = button
        .get("action_data")
        .and_then(value_to_action_string)
        .unwrap_or_default();
    let permission_type = button
        .get("permission_type")
        .and_then(Value::as_i64)
        .unwrap_or(2);
    let specified_role_ids = button
        .get("specified_role_ids")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let specified_user_ids = button
        .get("specified_user_ids")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let mut normalized = Map::new();
    normalized.insert(
        "id".to_string(),
        button
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .map(|id| Value::String(id.to_string()))
            .unwrap_or_else(|| Value::String(format!("{}-{}", row_index + 1, button_index + 1))),
    );
    normalized.insert(
        "render_data".to_string(),
        json!({
            "label": label,
            "visited_label": visited_label,
            "style": style,
        }),
    );
    normalized.insert(
        "action".to_string(),
        json!({
            "type": action_type,
            "permission": {
                "type": permission_type,
                "specify_role_ids": specified_role_ids,
                "specify_user_ids": specified_user_ids,
            },
            "data": action_data,
            "click_limit": 10,
            "at_bot_show_channel_list": true,
        }),
    );
    if let Some(tips) = button.get("unsupport_tips").and_then(Value::as_str) {
        normalized.insert(
            "unsupport_tips".to_string(),
            Value::String(tips.to_string()),
        );
    }

    Value::Object(normalized)
}

fn media_upload_from_segment(segment: &Segment) -> Option<EncodedMediaUpload> {
    let file_type = qqbot_file_type(segment.kind.as_str())?;
    let url = segment
        .data
        .get("url")
        .or_else(|| segment.data.get("file"))
        .and_then(value_to_action_string)?;
    if !is_remote_media_url(&url) {
        return None;
    }
    Some(EncodedMediaUpload {
        file_type,
        url,
        srv_send_msg: false,
    })
}

fn is_remote_media_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn value_to_action_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        other => Some(other.to_string()),
    }
}

fn append_fallback_segment(buffer: &mut String, label: &str) {
    append_fallback_text(buffer, &format!("[{label}]"));
}

fn append_fallback_text(buffer: &mut String, text: &str) {
    if !buffer.is_empty() && !buffer.chars().last().is_some_and(char::is_whitespace) {
        buffer.push(' ');
    }
    buffer.push_str(text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_message::{
        Message, Segment,
        keyboard::{ButtonStyle, KeyboardBuilder},
    };
    use qimen_protocol_core::ProtocolAdapter;

    fn packet(payload: Value) -> IncomingPacket {
        IncomingPacket {
            protocol: ProtocolId::QqOfficial,
            transport_mode: TransportMode::Gateway,
            bot_instance: "qq-official".to_string(),
            payload,
            raw_bytes: None,
        }
    }

    fn action(action: &str, params: Value) -> NormalizedActionRequest {
        NormalizedActionRequest {
            protocol: ProtocolId::QqOfficial,
            bot_instance: "qq-official".to_string(),
            action: action.to_string(),
            params,
            echo: None,
            timeout_ms: 5000,
            metadata: qimen_protocol_core::ActionMeta {
                source: "test".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn decode_group_at_message_create() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 42,
                "t": "GROUP_AT_MESSAGE_CREATE",
                "id": "event-1",
                "d": {
                    "id": "msg-1",
                    "content": "/ping",
                    "group_openid": "group-openid",
                    "author": {"member_openid": "member-openid"},
                    "msg_seq": 7
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.protocol, ProtocolId::QqOfficial);
        assert_eq!(event.kind, EventKind::Message);
        assert_eq!(event.message_id_str(), Some("msg-1".to_string()));
        assert_eq!(event.chat.as_ref().unwrap().kind, "group");
        assert_eq!(event.sender_id(), Some("member-openid"));
        assert_eq!(event.message.unwrap().plain_text(), "/ping");
        assert_eq!(
            event.extensions.get("event_type").and_then(Value::as_str),
            Some("GROUP_AT_MESSAGE_CREATE")
        );
    }

    #[tokio::test]
    async fn decode_c2c_message_create() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "t": "C2C_MESSAGE_CREATE",
                "id": "event-2",
                "d": {
                    "id": "msg-2",
                    "content": "hello",
                    "author": {"user_openid": "user-openid"},
                    "msg_seq": 1
                }
            })))
            .await
            .unwrap();

        let chat = event.chat.as_ref().unwrap();
        assert_eq!(chat.kind, "private");
        assert_eq!(chat.id, "user-openid");
        assert_eq!(event.sender_id(), Some("user-openid"));
    }

    #[tokio::test]
    async fn decode_at_message_create() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "t": "AT_MESSAGE_CREATE",
                "id": "event-3",
                "d": {
                    "id": "msg-3",
                    "content": "<@!1024> /help",
                    "channel_id": "channel-1",
                    "guild_id": "guild-1",
                    "author": {"id": "user-1", "username": "Alice"}
                }
            })))
            .await
            .unwrap();

        let chat = event.chat.as_ref().unwrap();
        assert_eq!(chat.kind, "channel");
        assert_eq!(chat.id, "channel-1");
        assert_eq!(event.sender_id(), Some("user-1"));
        assert_eq!(event.sender_nickname(), Some("Alice"));
    }

    #[tokio::test]
    async fn decode_direct_message_create() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "t": "DIRECT_MESSAGE_CREATE",
                "id": "event-4",
                "d": {
                    "id": "msg-4",
                    "content": "/ping",
                    "channel_id": "dm-channel",
                    "guild_id": "dm-guild",
                    "author": {"id": "user-2", "username": "Bob"}
                }
            })))
            .await
            .unwrap();

        let chat = event.chat.as_ref().unwrap();
        assert_eq!(chat.kind, "channel_private");
        assert_eq!(chat.id, "dm-guild");
        assert_eq!(event.sender_id(), Some("user-2"));
    }

    #[tokio::test]
    async fn encode_group_reply_action_preserves_reply_context() {
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": "pong",
                "msg_id": "msg-1",
                "msg_seq": 7,
                "event_id": "event-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();
        assert_eq!(
            packet.payload.get("route").and_then(Value::as_str),
            Some("group_message")
        );
        assert_eq!(
            packet.payload.get("group_openid").and_then(Value::as_str),
            Some("group-openid")
        );
        assert_eq!(
            packet.payload.get("msg_seq").and_then(Value::as_i64),
            Some(7)
        );
        assert_eq!(
            packet.payload.get("msg_id").and_then(Value::as_str),
            Some("msg-1")
        );
    }

    #[tokio::test]
    async fn encode_dms_send_msg_routes_by_guild_id() {
        let action = action(
            "send_msg",
            json!({
                "guild_id": "guild-1",
                "message": "pong",
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();
        assert_eq!(
            packet.payload.get("route").and_then(Value::as_str),
            Some("dms_message")
        );
        assert_eq!(
            packet.payload.get("guild_id").and_then(Value::as_str),
            Some("guild-1")
        );
    }

    #[tokio::test]
    async fn encode_markdown_message_segment_sets_official_payload() {
        let message = Message::builder()
            .text("fallback")
            .markdown("# Title\ncontent")
            .build();
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("fallback")
        );
        assert_eq!(
            packet
                .payload
                .get("markdown")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str),
            Some("# Title\ncontent")
        );
    }

    #[tokio::test]
    async fn encode_markdown_param_string_normalizes_to_content_object() {
        let action = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "markdown": "# Title",
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            packet
                .payload
                .get("markdown")
                .and_then(|value| value.get("content"))
                .and_then(Value::as_str),
            Some("# Title")
        );
    }

    #[tokio::test]
    async fn encode_keyboard_message_segment_sets_official_payload() {
        let keyboard = KeyboardBuilder::new()
            .command_button("Help", "/help")
            .style(ButtonStyle::Blue)
            .build();
        let message = Message::builder().text("choose").keyboard(keyboard).build();
        let action = action(
            "send_channel_msg",
            json!({
                "channel_id": "channel-1",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("choose")
        );
        assert_eq!(
            packet
                .payload
                .get("keyboard")
                .and_then(|value| value.get("content"))
                .and_then(|content| content.get("rows"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/id")
                .and_then(Value::as_str),
            Some("1-1")
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/render_data/label")
                .and_then(Value::as_str),
            Some("Help")
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/render_data/visited_label")
                .and_then(Value::as_str),
            Some("Help")
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/render_data/style")
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/action/type")
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/action/permission/type")
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/action/data")
                .and_then(Value::as_str),
            Some("/help")
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/action/click_limit")
                .and_then(Value::as_i64),
            Some(10)
        );
        assert_eq!(
            packet
                .payload
                .pointer("/keyboard/content/rows/0/buttons/0/action/at_bot_show_channel_list")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn encode_template_keyboard_id_passes_through() {
        let action = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "markdown": "# Title",
                "keyboard": { "id": "62" },
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(packet.payload.get("keyboard"), Some(&json!({ "id": "62" })));
    }

    #[tokio::test]
    async fn encode_ark_and_embed_segments_set_official_payloads() {
        let message = Message::from_segments(vec![
            Segment::new("ark")
                .with("template_id", json!(37))
                .with("kv", json!([{ "key": "#TITLE#", "value": "标题" }])),
            Segment::new("embed")
                .with("title", json!("embed消息"))
                .with("fields", json!([{ "name": "hello world" }])),
        ]);
        let action = action(
            "send_channel_msg",
            json!({
                "channel_id": "channel-1",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(3)
        );
        assert_eq!(
            packet
                .payload
                .get("ark")
                .and_then(|value| value.get("template_id"))
                .and_then(Value::as_i64),
            Some(37)
        );
        assert_eq!(
            packet
                .payload
                .get("embed")
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str),
            Some("embed消息")
        );
    }

    #[tokio::test]
    async fn encode_embed_segment_infers_embed_msg_type() {
        let message = Message::from_segments(vec![
            Segment::new("embed")
                .with("title", json!("embed消息"))
                .with("fields", json!([{ "name": "hello world" }])),
        ]);
        let action = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("route").and_then(Value::as_str),
            Some("c2c_message")
        );
        assert_eq!(
            packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(4)
        );
        assert_eq!(
            packet
                .payload
                .get("embed")
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str),
            Some("embed消息")
        );
    }

    #[tokio::test]
    async fn encode_group_image_segment_prepares_media_upload() {
        let message = Message::builder()
            .text("photo")
            .image("https://example.invalid/a.png")
            .build();
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("file_type"))
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(
            packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str),
            Some("https://example.invalid/a.png")
        );
        assert_eq!(
            packet.payload.get("image").and_then(Value::as_str),
            Some("https://example.invalid/a.png")
        );
        assert!(packet.payload.get("unsupported_segments").is_none());
    }

    #[tokio::test]
    async fn encode_group_image_param_prepares_media_upload() {
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "image": "https://example.invalid/a.png",
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(7)
        );
        assert_eq!(
            packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("url"))
                .and_then(Value::as_str),
            Some("https://example.invalid/a.png")
        );
    }

    #[tokio::test]
    async fn encode_group_record_and_video_segments_prepare_media_upload() {
        let record = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": Message::builder()
                    .record("https://example.invalid/a.silk")
                    .build()
                    .to_onebot_value(),
            }),
        );
        let video = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": Message::builder()
                    .video("https://example.invalid/a.mp4")
                    .build()
                    .to_onebot_value(),
            }),
        );

        let record_packet = QqBotAdapter.encode_action(&record).await.unwrap();
        let video_packet = QqBotAdapter.encode_action(&video).await.unwrap();

        assert_eq!(
            record_packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("file_type"))
                .and_then(Value::as_i64),
            Some(3)
        );
        assert_eq!(
            video_packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("file_type"))
                .and_then(Value::as_i64),
            Some(2)
        );
        assert_eq!(
            record_packet
                .payload
                .get("msg_type")
                .and_then(Value::as_i64),
            Some(7)
        );
        assert_eq!(
            video_packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(7)
        );
    }

    #[tokio::test]
    async fn encode_channel_image_segment_uses_image_field() {
        let message = Message::builder()
            .text("photo")
            .image("https://example.invalid/a.png")
            .build();
        let action = action(
            "send_channel_msg",
            json!({
                "channel_id": "channel-1",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("image").and_then(Value::as_str),
            Some("https://example.invalid/a.png")
        );
        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("photo")
        );
    }

    #[tokio::test]
    async fn encode_upload_media_routes_group_and_c2c_files() {
        let group = action(
            "upload_media",
            json!({
                "group_openid": "group-openid",
                "media_type": "image",
                "url": "https://example.invalid/a.png",
            }),
        );
        let c2c = action(
            "upload_media",
            json!({
                "openid": "user-openid",
                "file_type": 2,
                "url": "https://example.invalid/a.mp4",
                "srv_send_msg": true,
            }),
        );

        let group_packet = QqBotAdapter.encode_action(&group).await.unwrap();
        let c2c_packet = QqBotAdapter.encode_action(&c2c).await.unwrap();

        assert_eq!(
            group_packet.payload.get("route").and_then(Value::as_str),
            Some("group_file")
        );
        assert_eq!(
            group_packet
                .payload
                .get("file_type")
                .and_then(Value::as_i64),
            Some(1)
        );
        assert_eq!(
            c2c_packet.payload.get("route").and_then(Value::as_str),
            Some("c2c_file")
        );
        assert_eq!(
            c2c_packet
                .payload
                .get("srv_send_msg")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn encode_channel_recall_action_requires_channel_and_message() {
        let action = action(
            "recall_msg",
            json!({
                "channel_id": "channel-1",
                "message_id": "message-1",
                "hidetip": true,
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("route").and_then(Value::as_str),
            Some("channel_recall_message")
        );
        assert_eq!(
            packet.payload.get("channel_id").and_then(Value::as_str),
            Some("channel-1")
        );
        assert_eq!(
            packet.payload.get("message_id").and_then(Value::as_str),
            Some("message-1")
        );
        assert_eq!(
            packet.payload.get("hidetip").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn decode_group_manage_event_as_notice() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 9,
                "t": "GROUP_ADD_ROBOT",
                "id": "event-manage-1",
                "d": {
                    "group_openid": "group-openid",
                    "op_member_openid": "member-openid",
                    "timestamp": "2026-05-03T00:00:00+08:00"
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.kind, EventKind::Notice);
        assert_eq!(
            event.raw_json.get("notice_type").and_then(Value::as_str),
            Some("group_add_robot")
        );
        assert_eq!(event.chat.as_ref().unwrap().kind, "group");
        assert_eq!(event.chat.as_ref().unwrap().id, "group-openid");
        assert_eq!(event.sender_id(), Some("member-openid"));
        assert_eq!(
            event.extensions.get("event_type").and_then(Value::as_str),
            Some("GROUP_ADD_ROBOT")
        );
    }

    #[tokio::test]
    async fn decode_channel_delete_event_as_notice() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "t": "PUBLIC_MESSAGE_DELETE",
                "id": "event-delete-1",
                "d": {
                    "guild_id": "guild-1",
                    "channel_id": "channel-1",
                    "message_id": "message-1",
                    "author": {"id": "user-1"}
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.kind, EventKind::Notice);
        assert_eq!(
            event.raw_json.get("notice_type").and_then(Value::as_str),
            Some("public_message_delete")
        );
        assert_eq!(
            event.raw_json.get("message_id").and_then(Value::as_str),
            Some("message-1")
        );
        assert_eq!(event.chat.as_ref().unwrap().kind, "channel");
        assert_eq!(event.sender_id(), Some("user-1"));
    }

    #[tokio::test]
    async fn decode_direct_message_delete_event_as_channel_private_notice() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "t": "DIRECT_MESSAGE_DELETE",
                "id": "event-dm-delete-1",
                "d": {
                    "guild_id": "dm-guild",
                    "channel_id": "dm-channel",
                    "message_id": "message-1",
                    "author": {"id": "user-1"}
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.kind, EventKind::Notice);
        assert_eq!(
            event.raw_json.get("notice_type").and_then(Value::as_str),
            Some("direct_message_delete")
        );
        assert_eq!(event.chat.as_ref().unwrap().kind, "channel_private");
        assert_eq!(event.chat.as_ref().unwrap().id, "dm-guild");
    }

    #[tokio::test]
    async fn encode_unsupported_media_segments_degrades_to_text() {
        let message = Message::builder()
            .text("photo")
            .image("file://local/a.png")
            .record("file://local/a.mp3")
            .build();
        let action = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "message": message.to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("photo [image] [record]")
        );
        assert_eq!(
            packet
                .payload
                .get("unsupported_segments")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
            vec![json!("image"), json!("record")]
        );
        assert!(packet.payload.get("media").is_none());
    }

    #[tokio::test]
    async fn encode_reply_segment_fills_msg_id_when_param_missing() {
        let message = Message::builder().reply("reply-msg").text("pong").build();
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": message.to_onebot_value(),
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("msg_id").and_then(Value::as_str),
            Some("reply-msg")
        );
        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("pong")
        );
    }

    #[test]
    fn parse_intents_value() {
        let intents = vec![
            "public_messages".to_string(),
            "public_guild_messages".to_string(),
            "direct_message".to_string(),
        ];
        assert_eq!(
            qq_official_intents_value(&intents).unwrap(),
            (1_u64 << 25) | (1_u64 << 30) | (1_u64 << 12)
        );
    }
}
