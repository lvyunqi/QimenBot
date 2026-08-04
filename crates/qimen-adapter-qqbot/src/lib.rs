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
    pub scope: Option<String>,
    pub is_you: Option<bool>,
    pub avatar: Option<String>,
    pub union_openid: Option<String>,
    pub union_user_account: Option<String>,
    pub user_openid: Option<String>,
    pub member_openid: Option<String>,
    pub member_role: Option<String>,
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
    pub voice_wav_url: Option<String>,
    pub asr_refer_text: Option<String>,
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
    pub message_type: Option<i64>,
    pub message_scene: Option<Value>,
    pub ark_data: Option<Value>,
    #[serde(default)]
    pub mentions: Vec<QqBotUser>,
    #[serde(default)]
    pub attachments: Vec<QqBotAttachment>,
    #[serde(default)]
    pub msg_elements: Vec<Value>,
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
                "send_card_message".to_string(),
                "send_input_notify".to_string(),
                "upload_media".to_string(),
                "recall_message".to_string(),
                "ack_interaction".to_string(),
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
        let to_me = qqbot_message_is_to_me(event_type, &message_payload);
        let mut raw_json = qqbot_raw_message_json(&dispatch, &message_payload, message_kind, to_me);
        let mut extensions = qqbot_extensions(&dispatch, &message_payload);
        extensions.insert(
            "event_type".to_string(),
            Value::String(event_type.to_string()),
        );
        if to_me {
            extensions.insert("to_me".to_string(), Value::Bool(true));
        }

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
            time: qqbot_event_timestamp(&dispatch.data),
            kind: EventKind::Message,
            message: Some(message_from_qqbot(&message_payload, message_kind, to_me)),
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
            .or_else(|| packet.payload.get("err_code"))
            .or_else(|| packet.payload.get("retcode"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let status = if retcode == 0 {
            ActionStatus::Ok
        } else {
            ActionStatus::Failed
        };

        let data = packet.payload.get("data").cloned().unwrap_or_else(|| {
            if retcode == 0 {
                packet.payload.clone()
            } else {
                Value::Null
            }
        });

        Ok(NormalizedActionResponse {
            protocol: ProtocolId::QqOfficial,
            bot_instance: packet.bot_instance,
            status,
            retcode,
            data,
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
            | "recall_channel_message" => encode_recall_message_action(req)?,
            "ack_interaction" | "put_interaction" => encode_interaction_ack_action(req)?,
            "send_channel_msg" | "send_channel_message" => build_qqbot_send_payload(
                req,
                "channel_message",
                "channel_id",
                req.params.get("channel_id").cloned(),
                None,
                false,
            )?,
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
            )?,
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
            )?,
            "send_dms" | "send_dms_message" => build_qqbot_send_payload(
                req,
                "dms_message",
                "guild_id",
                req.params.get("guild_id").cloned(),
                None,
                false,
            )?,
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

        let (route, target_key) = match event.chat.as_ref().map(|chat| chat.kind.as_str()) {
            Some("group") => ("group_message", "group_openid"),
            Some("private") => ("c2c_message", "openid"),
            Some("channel") => ("channel_message", "channel_id"),
            Some("channel_private") => ("dms_message", "guild_id"),
            _ => return Ok(None),
        };

        let mut payload = Map::new();
        payload.insert("route".to_string(), Value::String(route.to_string()));
        payload.insert("content".to_string(), Value::String(reply_text.to_string()));
        payload.insert(
            target_key.to_string(),
            event
                .chat
                .as_ref()
                .map(|chat| Value::String(chat.id.clone()))
                .unwrap_or(Value::Null),
        );
        if let Some(message_id) = event.message_id_str() {
            payload.insert("msg_id".to_string(), Value::String(message_id));
            if matches!(route, "group_message" | "c2c_message") {
                payload.insert("msg_seq".to_string(), json!(1));
            }
        } else if let Some(event_id) = event.extensions.get("event_id").cloned() {
            payload.insert("event_id".to_string(), event_id);
        }

        Ok(Some(OutgoingPacket {
            payload: Value::Object(payload),
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
    let source = req
        .params
        .get("url")
        .or_else(|| req.params.get("file"))
        .or_else(|| req.params.get("base64"))
        .cloned();
    let upload_id = req.params.get("upload_id").cloned();
    if source.is_none() && upload_id.is_none() {
        return Err(QimenError::Protocol(
            "qqbot upload_media action requires url, base64, or upload_id".to_string(),
        ));
    }
    let srv_send_msg = req
        .params
        .get("srv_send_msg")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let mut payload = Map::new();
    payload.insert("route".to_string(), Value::String(target.0.to_string()));
    payload.insert(target.1.to_string(), target.2);
    payload.insert("file_type".to_string(), json!(file_type));
    payload.insert("srv_send_msg".to_string(), Value::Bool(srv_send_msg));
    if let Some(source) = source {
        let source = value_to_action_string(&source).ok_or_else(|| {
            QimenError::Protocol("qqbot upload_media source must be a string".to_string())
        })?;
        let (url, base64) = split_media_source(&source).ok_or_else(|| {
            QimenError::Protocol(
                "qqbot upload_media source must be an http(s) URL or base64 data".to_string(),
            )
        })?;
        if let Some(url) = url {
            payload.insert("url".to_string(), Value::String(url));
        }
        if let Some(base64) = base64 {
            payload.insert("base64".to_string(), Value::String(base64));
        }
    }
    if let Some(upload_id) = upload_id {
        payload.insert("upload_id".to_string(), upload_id);
    }
    if let Some(file_name) = req.params.get("file_name").cloned() {
        payload.insert("file_name".to_string(), file_name);
    }
    if let Some(content_type) = req.params.get("content_type").cloned() {
        payload.insert("content_type".to_string(), content_type);
    }
    Ok(Value::Object(payload))
}

fn encode_recall_message_action(req: &NormalizedActionRequest) -> Result<Value> {
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

    let (route, target_key, target) = if let Some(group_openid) = req
        .params
        .get("group_openid")
        .or_else(|| req.params.get("group_id"))
        .cloned()
    {
        ("group_recall_message", "group_openid", group_openid)
    } else if let Some(openid) = req
        .params
        .get("openid")
        .or_else(|| req.params.get("user_id"))
        .cloned()
    {
        ("c2c_recall_message", "openid", openid)
    } else if let Some(channel_id) = req.params.get("channel_id").cloned() {
        ("channel_recall_message", "channel_id", channel_id)
    } else if let Some(guild_id) = req.params.get("guild_id").cloned() {
        ("dms_recall_message", "guild_id", guild_id)
    } else {
        return Err(QimenError::Protocol(
            "qqbot recall action requires group_openid/group_id, openid/user_id, channel_id, or guild_id"
                .to_string(),
        ));
    };

    let mut payload = Map::new();
    payload.insert("route".to_string(), Value::String(route.to_string()));
    payload.insert(target_key.to_string(), target);
    payload.insert("message_id".to_string(), message_id);
    if matches!(route, "channel_recall_message" | "dms_recall_message") {
        payload.insert("hidetip".to_string(), Value::Bool(hidetip));
    }
    Ok(Value::Object(payload))
}

fn encode_interaction_ack_action(req: &NormalizedActionRequest) -> Result<Value> {
    let interaction_id = req
        .params
        .get("interaction_id")
        .or_else(|| req.params.get("id"))
        .cloned()
        .ok_or_else(|| {
            QimenError::Protocol("qqbot interaction ack requires interaction_id".to_string())
        })?;
    let code = req.params.get("code").and_then(Value::as_i64).unwrap_or(0);
    if !(0..=5).contains(&code) {
        return Err(QimenError::Protocol(
            "qqbot interaction ack code must be between 0 and 5".to_string(),
        ));
    }

    Ok(json!({
        "route": "interaction_ack",
        "interaction_id": interaction_id,
        "code": code,
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
    let normalized = intent.trim().to_ascii_lowercase();
    let bit = match normalized.as_str() {
        "guilds" => 1_u64 << 0,
        "guild_members" => 1_u64 << 1,
        "guild_messages" => 1_u64 << 9,
        "guild_message_reactions" => 1_u64 << 10,
        "direct_message" => 1_u64 << 12,
        "open_forum_event" => 1_u64 << 18,
        "audio_or_live_channel_member" => 1_u64 << 19,
        "group_and_c2c_event" | "public_messages" => 1_u64 << 25,
        "interaction" => 1_u64 << 26,
        "message_audit" => 1_u64 << 27,
        "forums_event" | "forums" => 1_u64 << 28,
        "audio_action" => 1_u64 << 29,
        "public_guild_messages" => 1_u64 << 30,
        _ => {
            return Err(QimenError::Protocol(format!(
                "unknown qq-official intent '{}'",
                intent
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
    if let Some(event_type) = dispatch.event_type.as_deref() {
        copy_qqbot_event_object_context(event_type, &dispatch.data, &mut extensions);
    }

    NormalizedEvent {
        protocol: ProtocolId::QqOfficial,
        bot_instance: packet.bot_instance,
        transport_mode: packet.transport_mode,
        time: qqbot_event_timestamp(&dispatch.data),
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
        "GROUP_MEMBER_ADD" => Some("group_member_add"),
        "GROUP_MEMBER_REMOVE" => Some("group_member_remove"),
        "C2C_MSG_REJECT" => Some("c2c_msg_reject"),
        "C2C_MSG_RECEIVE" => Some("c2c_msg_receive"),
        "SUBSCRIBE_MESSAGE_STATUS" => Some("subscribe_message_status"),
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
    if let Some(event_type) = dispatch.event_type.as_deref() {
        copy_qqbot_event_object_context(event_type, &dispatch.data, &mut raw);
    }
    raw.insert("qqbot_payload".to_string(), dispatch.data.clone());
    raw
}

fn actor_from_non_message(data: &Value) -> Option<ActorRef> {
    let id = data
        .get("op_member_openid")
        .or_else(|| data.get("operator_openid"))
        .or_else(|| data.get("op_user_id"))
        .or_else(|| data.get("group_member_openid"))
        .or_else(|| data.get("member_openid"))
        .or_else(|| data.get("openid"))
        .or_else(|| data.get("user_openid"))
        .or_else(|| data.get("user_id"))
        .or_else(|| data.get("operator_id"))
        .or_else(|| data.get("owner_id"))
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
    if matches!(
        event_type,
        Some("CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE")
    ) && let Some(channel_id) = data.get("id").and_then(value_to_action_string)
    {
        return Some(ChatRef {
            id: channel_id,
            kind: "channel".to_string(),
        });
    }
    if matches!(
        event_type,
        Some("GUILD_CREATE" | "GUILD_UPDATE" | "GUILD_DELETE")
    ) && let Some(guild_id) = data.get("id").and_then(value_to_action_string)
    {
        return Some(ChatRef {
            id: guild_id,
            kind: "guild".to_string(),
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
        "member_openid",
        "operator_openid",
        "op_user_id",
        "owner_id",
        "message_type",
        "message_scene",
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
        "member_openid",
        "operator_openid",
        "op_user_id",
        "owner_id",
        "message_type",
        "message_scene",
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
        .or_else(|| data.get("operator_openid"))
        .or_else(|| data.get("op_user_id"))
        .or_else(|| data.get("group_member_openid"))
        .or_else(|| data.get("member_openid"))
        .or_else(|| data.get("openid"))
        .or_else(|| data.get("user_openid"))
        .or_else(|| data.get("user_id"))
        .or_else(|| data.get("owner_id"))
        .or_else(|| data.get("author").and_then(|author| author.get("id")))
        .or_else(|| data.get("user").and_then(|user| user.get("id")))
        .cloned();
    if let Some(user_id) = user_id {
        raw.insert("user_id".to_string(), user_id);
    }
    let message_id = data
        .get("message_id")
        .or_else(|| data.get("msg_id"))
        .or_else(|| data.pointer("/message/id"))
        .or_else(|| data.get("target").and_then(|target| target.get("id")))
        .cloned();
    if let Some(message_id) = message_id {
        raw.insert("message_id".to_string(), message_id);
    }
}

fn copy_qqbot_event_object_context(
    event_type: &str,
    data: &Value,
    target: &mut Map<String, Value>,
) {
    let Some(id) = data.get("id").cloned() else {
        return;
    };
    let key = match event_type {
        "GUILD_CREATE" | "GUILD_UPDATE" | "GUILD_DELETE" => "guild_id",
        "CHANNEL_CREATE" | "CHANNEL_UPDATE" | "CHANNEL_DELETE" => "channel_id",
        _ => return,
    };
    target.entry(key.to_string()).or_insert(id);
}

fn message_kind(event_type: &str) -> Option<QqBotMessageKind> {
    match event_type {
        "GROUP_AT_MESSAGE_CREATE" => Some(QqBotMessageKind::Group),
        "GROUP_MESSAGE_CREATE" => Some(QqBotMessageKind::Group),
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
        QqBotMessageKind::Group => author.member_openid.as_deref().or(author.id.as_deref()),
        QqBotMessageKind::C2c => author.user_openid.as_deref().or(author.id.as_deref()),
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
            .and_then(|author| author.user_openid.clone().or_else(|| author.id.clone()))
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

fn message_from_qqbot(
    payload: &QqBotMessagePayload,
    kind: QqBotMessageKind,
    to_me: bool,
) -> Message {
    let mut segments = Vec::new();
    if let Some(content) = payload.content.as_deref()
        && !content.is_empty()
    {
        segments.extend(qqbot_text_segments(content));
    }

    merge_qqbot_mentions(&mut segments, &payload.mentions, kind);
    if to_me {
        trim_directed_qqbot_text(&mut segments);
    }

    for attachment in &payload.attachments {
        let Some(url) = attachment.url.clone() else {
            continue;
        };
        let content_type = attachment
            .content_type
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let kind = if content_type.starts_with("image/") {
            "image"
        } else if content_type == "voice" || content_type.starts_with("audio/") {
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
        if let Some(voice_wav_url) = attachment.voice_wav_url.clone() {
            segment = segment.with("voice_wav_url", Value::String(voice_wav_url));
        }
        if let Some(asr_refer_text) = attachment.asr_refer_text.clone() {
            segment = segment.with("asr_refer_text", Value::String(asr_refer_text));
        }
        segments.push(segment);
    }

    if let Some(ark_data) = payload.ark_data.clone() {
        segments.push(Segment::new("ark").with("content", ark_data));
    }
    for element in &payload.msg_elements {
        segments.push(Segment::new("qqbot_element").with("content", element.clone()));
    }

    Message::from_segments(segments)
}

fn qqbot_message_is_to_me(event_type: &str, payload: &QqBotMessagePayload) -> bool {
    matches!(event_type, "GROUP_AT_MESSAGE_CREATE" | "AT_MESSAGE_CREATE")
        || payload
            .mentions
            .iter()
            .any(|mention| mention.is_you == Some(true) && mention.scope.as_deref() != Some("all"))
}

fn qqbot_text_segments(content: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut text_start = 0;
    let mut scan_from = 0;

    while let Some(relative_start) = content[scan_from..].find('<') {
        let start = scan_from + relative_start;
        let Some(relative_end) = content[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        let Some(target) = parse_qqbot_mention_tag(&content[start..end]) else {
            scan_from = start + 1;
            continue;
        };

        if text_start < start {
            segments.push(Segment::text(&content[text_start..start]));
        }
        segments.push(Segment::at(target));
        text_start = end;
        scan_from = end;
    }

    if text_start < content.len() {
        segments.push(Segment::text(&content[text_start..]));
    }

    segments
}

fn parse_qqbot_mention_tag(tag: &str) -> Option<String> {
    let inner = tag.strip_prefix('<')?.strip_suffix('>')?.trim();
    if let Some(legacy) = inner.strip_prefix('@') {
        let target = legacy.strip_prefix('!').unwrap_or(legacy);
        return valid_qqbot_mention_id(target).then(|| target.to_string());
    }

    let current = inner.strip_suffix('/').unwrap_or(inner).trim_end();
    if current == "qqbot-at-everyone" {
        return Some("all".to_string());
    }

    let attributes = current.strip_prefix("qqbot-at-user")?;
    if !attributes.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    parse_qqbot_id_attribute(attributes)
}

fn parse_qqbot_id_attribute(attributes: &str) -> Option<String> {
    let value = attributes
        .trim()
        .strip_prefix("id")?
        .trim_start()
        .strip_prefix('=')?
        .trim_start();
    let quote = value.chars().next()?;
    if !matches!(quote, '\'' | '"') {
        return None;
    }
    let value = &value[quote.len_utf8()..];
    let end = value.find(quote)?;
    let target = &value[..end];
    if !value[end + quote.len_utf8()..].trim().is_empty() {
        return None;
    }
    valid_qqbot_mention_id(target).then(|| target.to_string())
}

fn valid_qqbot_mention_id(target: &str) -> bool {
    !target.is_empty()
        && target
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
}

fn merge_qqbot_mentions(
    segments: &mut Vec<Segment>,
    mentions: &[QqBotUser],
    kind: QqBotMessageKind,
) {
    let mut matched = vec![false; segments.len()];
    let mut leading = Vec::new();
    let mut trailing = Vec::new();

    for mention in mentions {
        let Some(target) = qqbot_mention_target(mention, kind) else {
            continue;
        };
        let matched_index = segments.iter().enumerate().find_map(|(index, segment)| {
            (!matched[index] && qqbot_segment_matches_mention(segment, mention, target))
                .then_some(index)
        });

        if let Some(index) = matched_index {
            matched[index] = true;
            enrich_qqbot_mention_segment(&mut segments[index], mention, target);
            continue;
        }

        let mut segment = Segment::at(target);
        enrich_qqbot_mention_segment(&mut segment, mention, target);
        if mention.is_you == Some(true) && mention.scope.as_deref() != Some("all") {
            leading.push(segment);
        } else {
            trailing.push(segment);
        }
    }

    if !leading.is_empty() {
        leading.append(segments);
        *segments = leading;
    }
    segments.extend(trailing);
}

fn qqbot_mention_target(mention: &QqBotUser, kind: QqBotMessageKind) -> Option<&str> {
    if mention.scope.as_deref() == Some("all") {
        return Some("all");
    }
    match kind {
        QqBotMessageKind::Group => mention.member_openid.as_deref().or(mention.id.as_deref()),
        QqBotMessageKind::C2c => mention.user_openid.as_deref().or(mention.id.as_deref()),
        QqBotMessageKind::ChannelMention
        | QqBotMessageKind::ChannelDirect
        | QqBotMessageKind::Channel => mention.id.as_deref(),
    }
}

fn qqbot_segment_matches_mention(segment: &Segment, mention: &QqBotUser, target: &str) -> bool {
    let Some(segment_target) = segment.at_target() else {
        return false;
    };
    segment_target == target
        || mention.id.as_deref() == Some(segment_target.as_str())
        || mention.member_openid.as_deref() == Some(segment_target.as_str())
        || mention.user_openid.as_deref() == Some(segment_target.as_str())
}

fn enrich_qqbot_mention_segment(segment: &mut Segment, mention: &QqBotUser, target: &str) {
    segment
        .data
        .insert("qq".to_string(), Value::String(target.to_string()));
    for (key, value) in [
        ("id", mention.id.as_ref()),
        ("username", mention.username.as_ref()),
        ("scope", mention.scope.as_ref()),
        ("member_openid", mention.member_openid.as_ref()),
        ("user_openid", mention.user_openid.as_ref()),
    ] {
        if let Some(value) = value {
            segment
                .data
                .insert(key.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(bot) = mention.bot {
        segment.data.insert("bot".to_string(), Value::Bool(bot));
    }
    if let Some(is_you) = mention.is_you {
        segment
            .data
            .insert("is_you".to_string(), Value::Bool(is_you));
        if is_you && mention.scope.as_deref() != Some("all") {
            segment
                .data
                .insert("is_self".to_string(), Value::Bool(true));
        }
    }
}

fn trim_directed_qqbot_text(segments: &mut [Segment]) {
    let Some(segment) = segments.iter_mut().find(|segment| segment.is_text()) else {
        return;
    };
    let Some(text) = segment.get_text() else {
        return;
    };
    let trimmed = text
        .trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '\u{feff}' | '\u{200b}'))
        .to_string();
    segment
        .data
        .insert("text".to_string(), Value::String(trimmed));
}

fn qqbot_raw_message_json(
    dispatch: &GatewayDispatch,
    payload: &QqBotMessagePayload,
    kind: QqBotMessageKind,
    to_me: bool,
) -> Map<String, Value> {
    let mut raw = Map::new();
    raw.insert(
        "post_type".to_string(),
        Value::String("message".to_string()),
    );
    if let Some(message_id) = payload.id.clone() {
        raw.insert("message_id".to_string(), Value::String(message_id));
    }
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
        if to_me {
            raw.insert("to_me".to_string(), Value::Bool(true));
        }
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
    if let Some(msg_seq) = payload.msg_seq {
        raw.insert("msg_seq".to_string(), json!(msg_seq));
    }
    if let Some(msg_idx) = qqbot_message_index(payload) {
        raw.insert("msg_idx".to_string(), Value::String(msg_idx));
    }
    if let Some(message_type) = payload.message_type {
        raw.insert("qqbot_message_type".to_string(), json!(message_type));
    }
    if let Some(message_scene) = payload.message_scene.clone() {
        raw.insert("message_scene".to_string(), message_scene);
    }
    if let Some(ark_data) = payload.ark_data.clone() {
        raw.insert("ark_data".to_string(), ark_data);
    }
    if !payload.msg_elements.is_empty() {
        raw.insert("msg_elements".to_string(), json!(payload.msg_elements));
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
                "role": author.member_role,
                "bot": author.bot,
                "union_openid": author.union_openid,
                "union_user_account": author.union_user_account,
            }),
        );
    }
    raw.insert("qqbot_payload".to_string(), dispatch.data.clone());
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
    if let Some(msg_seq) = payload.msg_seq {
        extensions.insert("msg_seq".to_string(), json!(msg_seq));
    }
    if let Some(msg_idx) = qqbot_message_index(payload) {
        extensions.insert("msg_idx".to_string(), Value::String(msg_idx));
    }
    if let Some(message_type) = payload.message_type {
        extensions.insert("qqbot_message_type".to_string(), json!(message_type));
    }
    if let Some(message_scene) = payload.message_scene.clone() {
        extensions.insert("message_scene".to_string(), message_scene);
    }
    extensions
}

fn qqbot_message_index(payload: &QqBotMessagePayload) -> Option<String> {
    payload
        .message_scene
        .as_ref()?
        .get("ext")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(|entry| entry.strip_prefix("msg_idx=").map(ToOwned::to_owned))
}

fn qqbot_event_timestamp(data: &Value) -> Option<i64> {
    match data.get("timestamp").or_else(|| data.get("joined_at"))? {
        Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok())),
        Value::String(value) => value.parse::<i64>().ok().or_else(|| {
            chrono::DateTime::parse_from_rfc3339(value)
                .ok()
                .map(|value| value.timestamp())
        }),
        _ => None,
    }
}

fn encode_send_message_action(req: &NormalizedActionRequest) -> Result<Value> {
    if req.params.get("group_openid").is_some() || req.params.get("group_id").is_some() {
        return build_qqbot_send_payload(
            req,
            "group_message",
            "group_openid",
            req.params
                .get("group_openid")
                .or_else(|| req.params.get("group_id"))
                .cloned(),
            Some(0),
            true,
        );
    }

    if req.params.get("openid").is_some() || req.params.get("user_id").is_some() {
        return build_qqbot_send_payload(
            req,
            "c2c_message",
            "openid",
            req.params
                .get("openid")
                .or_else(|| req.params.get("user_id"))
                .cloned(),
            Some(0),
            true,
        );
    }

    if req.params.get("channel_id").is_some() {
        return build_qqbot_send_payload(
            req,
            "channel_message",
            "channel_id",
            req.params.get("channel_id").cloned(),
            None,
            false,
        );
    }

    if req.params.get("guild_id").is_some() {
        return build_qqbot_send_payload(
            req,
            "dms_message",
            "guild_id",
            req.params.get("guild_id").cloned(),
            None,
            false,
        );
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
) -> Result<Value> {
    let target_value = target_value
        .filter(|value| !value.is_null())
        .ok_or_else(|| {
            QimenError::Protocol(format!("qqbot {route} action requires {target_key}"))
        })?;
    let mut message = encode_action_message(req);
    let group_or_c2c = matches!(route, "group_message" | "c2c_message");
    if message.media.is_none()
        && message.upload.is_none()
        && let Some(image) = message.image.as_deref()
        && let Some(upload) = encoded_media_upload(1, image, None, None)
    {
        message.upload = Some(upload);
    }
    if group_or_c2c
        && message.media.is_none()
        && message.upload.is_some()
        && message.markdown.is_none()
        && message.msg_type.is_none()
    {
        message.msg_type = Some(7);
    }

    if group_or_c2c && message.upload.is_some() {
        message.image = None;
    } else if !group_or_c2c && let Some(upload) = message.upload.take() {
        if upload.file_type != 1 {
            return Err(QimenError::Protocol(format!(
                "qqbot {route} only supports image media"
            )));
        }
        if let Some(url) = upload.url {
            message.image = Some(url);
        } else {
            message.upload = Some(upload);
        }
    }

    if message.ark.is_some() && message.embed.is_some() {
        return Err(QimenError::Protocol(
            "qqbot messages cannot contain both ark and embed".to_string(),
        ));
    }

    let explicit_msg_type = req
        .params
        .get("msg_type")
        .map(|value| {
            value.as_i64().ok_or_else(|| {
                QimenError::Protocol("qqbot msg_type must be an integer".to_string())
            })
        })
        .transpose()?;
    let mut msg_type = explicit_msg_type.or(message.msg_type).or(default_msg_type);
    match route {
        "group_message" => {
            if message.ark.is_some() || message.embed.is_some() || message.input_notify.is_some() {
                return Err(QimenError::Protocol(
                    "qqbot group messages support text, markdown, media, and card payloads"
                        .to_string(),
                ));
            }
            if message.image.is_some() {
                return Err(QimenError::Protocol(
                    "qqbot group image messages require a supported media source".to_string(),
                ));
            }
            if !matches!(msg_type, Some(0 | 2 | 7 | 8)) {
                return Err(QimenError::Protocol(format!(
                    "unsupported qqbot group msg_type {}",
                    msg_type.unwrap_or_default()
                )));
            }
        }
        "c2c_message" => {
            if message.ark.is_some() || message.embed.is_some() || message.card.is_some() {
                return Err(QimenError::Protocol(
                    "qqbot c2c messages support text, markdown, input notification, and media payloads"
                        .to_string(),
                ));
            }
            if message.image.is_some() {
                return Err(QimenError::Protocol(
                    "qqbot c2c image messages require a supported media source".to_string(),
                ));
            }
            if !matches!(msg_type, Some(0 | 2 | 6 | 7)) {
                return Err(QimenError::Protocol(format!(
                    "unsupported qqbot c2c msg_type {}",
                    msg_type.unwrap_or_default()
                )));
            }
        }
        "channel_message" | "dms_message" => {
            if message.card.is_some() || message.input_notify.is_some() || message.media.is_some() {
                return Err(QimenError::Protocol(format!(
                    "qqbot {route} does not support group/C2C-only payload fields"
                )));
            }
            msg_type = None;
        }
        _ => {}
    }

    match msg_type {
        Some(2) => {
            if message.markdown.is_none() {
                return Err(QimenError::Protocol(
                    "qqbot msg_type 2 requires markdown".to_string(),
                ));
            }
            message.content = None;
        }
        Some(6) => {
            if message.input_notify.is_none() {
                return Err(QimenError::Protocol(
                    "qqbot msg_type 6 requires input_notify".to_string(),
                ));
            }
            message.content = None;
        }
        Some(7) => {
            if message.media.is_none() && message.upload.is_none() {
                return Err(QimenError::Protocol(
                    "qqbot msg_type 7 requires media or an uploadable URL".to_string(),
                ));
            }
            message.content = None;
        }
        Some(8) => {
            if message.card.is_none() {
                return Err(QimenError::Protocol(
                    "qqbot msg_type 8 requires card".to_string(),
                ));
            }
            message.content = None;
        }
        _ if message.markdown.is_some() => message.content = None,
        _ => {}
    }

    let srv_send_msg = req.params.get("srv_send_msg").and_then(Value::as_bool);
    if srv_send_msg == Some(true)
        && (!group_or_c2c
            || message.upload.is_none()
            || message.content.is_some()
            || message.markdown.is_some()
            || message.keyboard.is_some()
            || message.card.is_some()
            || message.input_notify.is_some()
            || message.message_reference.is_some()
            || message.reply_msg_id.is_some()
            || req.params.get("msg_id").is_some()
            || req.params.get("event_id").is_some()
            || req.params.get("is_wakeup").and_then(Value::as_bool) == Some(true))
    {
        return Err(QimenError::Protocol(
            "qqbot srv_send_msg=true only supports proactive, media-only group or C2C uploads"
                .to_string(),
        ));
    }

    let mut payload = Map::new();
    payload.insert("route".to_string(), Value::String(route.to_string()));
    payload.insert(target_key.to_string(), target_value);

    if let Some(msg_type) = msg_type {
        payload.insert("msg_type".to_string(), json!(msg_type));
    }
    if let Some(content) = message.content {
        payload.insert("content".to_string(), Value::String(content));
    }
    let msg_id = req.params.get("msg_id").cloned().or(message.reply_msg_id);
    let event_id = req.params.get("event_id").cloned();
    let is_wakeup = req.params.get("is_wakeup").and_then(Value::as_bool);
    if is_wakeup == Some(true) && !group_or_c2c {
        return Err(QimenError::Protocol(
            "qqbot is_wakeup is only supported for group and C2C messages".to_string(),
        ));
    }
    if is_wakeup == Some(true) && (msg_id.is_some() || event_id.is_some()) {
        return Err(QimenError::Protocol(
            "qqbot is_wakeup is mutually exclusive with msg_id and event_id".to_string(),
        ));
    }
    if let Some(msg_id) = msg_id {
        payload.insert("msg_id".to_string(), msg_id);
        if include_msg_seq && let Some(msg_seq) = req.params.get("msg_seq") {
            let msg_seq = msg_seq.as_i64().filter(|value| *value > 0).ok_or_else(|| {
                QimenError::Protocol("qqbot msg_seq must be a positive integer".to_string())
            })?;
            payload.insert("msg_seq".to_string(), json!(msg_seq));
        }
    } else if let Some(event_id) = event_id {
        payload.insert("event_id".to_string(), event_id);
    }
    if is_wakeup == Some(true) {
        payload.insert("is_wakeup".to_string(), Value::Bool(true));
    }
    if let Some(value) = srv_send_msg
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
    if let Some(card) = message.card {
        payload.insert("card".to_string(), card);
    }
    if let Some(input_notify) = message.input_notify {
        payload.insert("input_notify".to_string(), input_notify);
    }
    if let Some(message_reference) = message.message_reference {
        payload.insert("message_reference".to_string(), message_reference);
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
            encoded_media_upload_value(&upload),
        );
    }
    if !message.unsupported_segments.is_empty() {
        payload.insert(
            "unsupported_segments".to_string(),
            json!(message.unsupported_segments),
        );
    }

    Ok(Value::Object(payload))
}

#[derive(Debug, Default)]
struct EncodedActionMessage {
    msg_type: Option<i64>,
    content: Option<String>,
    markdown: Option<Value>,
    keyboard: Option<Value>,
    ark: Option<Value>,
    embed: Option<Value>,
    card: Option<Value>,
    input_notify: Option<Value>,
    message_reference: Option<Value>,
    media: Option<Value>,
    image: Option<String>,
    upload: Option<EncodedMediaUpload>,
    reply_msg_id: Option<Value>,
    unsupported_segments: Vec<String>,
}

#[derive(Debug, Clone)]
struct EncodedMediaUpload {
    file_type: i64,
    url: Option<String>,
    base64: Option<String>,
    file_name: Option<String>,
    content_type: Option<String>,
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
        if self.card.is_none() {
            self.card = other.card;
        }
        if self.input_notify.is_none() {
            self.input_notify = other.input_notify;
        }
        if self.message_reference.is_none() {
            self.message_reference = other.message_reference;
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
    if let Some(card) = req.params.get("card") {
        encoded.card = Some(card.clone());
    }
    if let Some(input_notify) = req.params.get("input_notify") {
        encoded.input_notify = Some(input_notify.clone());
    }
    if let Some(message_reference) = req.params.get("message_reference") {
        encoded.message_reference = Some(message_reference.clone());
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
            "card" => {
                if encoded.card.is_none() {
                    encoded.card = Some(rich_object_segment_payload(segment));
                }
            }
            "input_notify" => {
                if encoded.input_notify.is_none() {
                    encoded.input_notify = Some(rich_object_segment_payload(segment));
                }
            }
            "message_reference" => {
                if encoded.message_reference.is_none() {
                    encoded.message_reference = Some(rich_object_segment_payload(segment));
                }
            }
            "reply" => {
                if let Some(message_id) = segment.data.get("id").cloned() {
                    if encoded.reply_msg_id.is_none() {
                        encoded.reply_msg_id = Some(message_id.clone());
                    }
                    if encoded.message_reference.is_none() {
                        encoded.message_reference = Some(json!({ "message_id": message_id }));
                    }
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
                    if segment.kind == "image"
                        && let Some(url) = upload.url.as_ref()
                    {
                        encoded.image = Some(url.clone());
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
    } else if encoded.input_notify.is_some() {
        Some(6)
    } else if encoded.media.is_some() {
        Some(7)
    } else if encoded.card.is_some() {
        Some(8)
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
    let source = segment
        .data
        .get("url")
        .or_else(|| segment.data.get("file"))
        .or_else(|| segment.data.get("base64"))
        .and_then(value_to_action_string)?;
    encoded_media_upload(
        file_type,
        &source,
        segment
            .data
            .get("file_name")
            .or_else(|| segment.data.get("name"))
            .and_then(value_to_action_string),
        segment
            .data
            .get("content_type")
            .or_else(|| segment.data.get("mime_type"))
            .and_then(value_to_action_string),
    )
}

fn is_remote_media_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn split_media_source(value: &str) -> Option<(Option<String>, Option<String>)> {
    if is_remote_media_url(value) {
        return Some((Some(value.to_string()), None));
    }
    if let Some(value) = value.strip_prefix("base64://") {
        return (!value.is_empty()).then(|| (None, Some(value.to_string())));
    }
    if value.starts_with("data:") && value.contains(";base64,") {
        return Some((None, Some(value.to_string())));
    }
    // A value supplied through the explicit `base64` field may omit the
    // `base64://` prefix. Runtime validation still rejects malformed data.
    (!value.is_empty() && !value.contains("://")).then(|| (None, Some(value.to_string())))
}

fn encoded_media_upload(
    file_type: i64,
    source: &str,
    file_name: Option<String>,
    content_type: Option<String>,
) -> Option<EncodedMediaUpload> {
    let (url, base64) = split_media_source(source)?;
    Some(EncodedMediaUpload {
        file_type,
        url,
        base64,
        file_name,
        content_type,
        srv_send_msg: false,
    })
}

fn encoded_media_upload_value(upload: &EncodedMediaUpload) -> Value {
    let mut value = Map::new();
    value.insert("file_type".to_string(), json!(upload.file_type));
    if let Some(url) = upload.url.as_ref() {
        value.insert("url".to_string(), Value::String(url.clone()));
    }
    if let Some(base64) = upload.base64.as_ref() {
        value.insert("base64".to_string(), Value::String(base64.clone()));
    }
    if let Some(file_name) = upload.file_name.as_ref() {
        value.insert("file_name".to_string(), Value::String(file_name.clone()));
    }
    if let Some(content_type) = upload.content_type.as_ref() {
        value.insert(
            "content_type".to_string(),
            Value::String(content_type.clone()),
        );
    }
    value.insert("srv_send_msg".to_string(), Value::Bool(upload.srv_send_msg));
    Value::Object(value)
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
    async fn decode_current_openapi_response_shapes() {
        let success = QqBotAdapter
            .decode_action_response(packet(json!({
                "id": "sent-message",
                "timestamp": "2026-08-01T00:00:00+08:00"
            })))
            .await
            .unwrap();
        let failure = QqBotAdapter
            .decode_action_response(packet(json!({
                "err_code": 40034005,
                "message": "expired"
            })))
            .await
            .unwrap();

        assert!(matches!(success.status, ActionStatus::Ok));
        assert_eq!(
            success.data.get("id").and_then(Value::as_str),
            Some("sent-message")
        );
        assert!(matches!(failure.status, ActionStatus::Failed));
        assert_eq!(failure.retcode, 40034005);
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
        assert!(event.is_at_self());
        assert_eq!(event.message.unwrap().plain_text(), "/ping");
        assert_eq!(
            event.extensions.get("event_type").and_then(Value::as_str),
            Some("GROUP_AT_MESSAGE_CREATE")
        );
    }

    #[tokio::test]
    async fn decode_group_message_create() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 43,
                "t": "GROUP_MESSAGE_CREATE",
                "id": "event-full-group",
                "d": {
                    "id": "msg-full-group",
                    "content": "/ping",
                    "group_openid": "group-openid",
                    "author": {
                        "id": "member-openid",
                        "member_openid": "member-openid",
                        "member_role": "admin",
                        "union_openid": "union-openid"
                    },
                    "message_type": 0,
                    "message_scene": {
                        "source": "default",
                        "ext": ["msg_idx=message-index", "auth_token=token"]
                    },
                    "mentions": [{"member_openid": "mentioned-member"}],
                    "attachments": [{
                        "content_type": "voice",
                        "filename": "voice.silk",
                        "url": "https://example.invalid/voice.silk",
                        "voice_wav_url": "https://example.invalid/voice.wav",
                        "asr_refer_text": "voice text"
                    }],
                    "future_field": {"kept": true},
                    "timestamp": "2026-08-01T07:07:53+08:00"
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.protocol, ProtocolId::QqOfficial);
        assert_eq!(event.kind, EventKind::Message);
        assert_eq!(event.message_id_str(), Some("msg-full-group".to_string()));
        assert_eq!(event.chat.as_ref().unwrap().kind, "group");
        assert_eq!(event.chat.as_ref().unwrap().id, "group-openid");
        assert_eq!(event.sender_id(), Some("member-openid"));
        let message = event.message.as_ref().unwrap();
        assert_eq!(message.plain_text(), "/ping");
        assert_eq!(message.at_list(), vec!["mentioned-member"]);
        assert!(message.has_record());
        assert!(event.is_group_admin_or_owner());
        assert!(event.time.is_some());
        assert_eq!(
            event.extensions.get("event_type").and_then(Value::as_str),
            Some("GROUP_MESSAGE_CREATE")
        );
        assert_eq!(
            event.extensions.get("event_id").and_then(Value::as_str),
            Some("event-full-group")
        );
        assert_eq!(
            event
                .extensions
                .get("qqbot_message_type")
                .and_then(Value::as_i64),
            Some(0)
        );
        assert_eq!(
            event.extensions.get("msg_idx").and_then(Value::as_str),
            Some("message-index")
        );
        assert_eq!(
            event
                .raw_json
                .pointer("/qqbot_payload/future_field/kept")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn decode_full_group_bot_mention_preserves_command_text() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 44,
                "t": "GROUP_MESSAGE_CREATE",
                "id": "event-full-group-at",
                "d": {
                    "id": "msg-full-group-at",
                    "content": "<qqbot-at-user id=\"bot-user-id\" /> /ping",
                    "group_openid": "group-openid",
                    "author": {
                        "id": "member-openid",
                        "member_openid": "member-openid"
                    },
                    "mentions": [{
                        "scope": "single",
                        "bot": true,
                        "id": "bot-user-id",
                        "is_you": true,
                        "member_openid": "bot-member-openid",
                        "username": "bot"
                    }]
                }
            })))
            .await
            .unwrap();

        let message = event.message.as_ref().unwrap();
        assert_eq!(message.plain_text().trim(), "/ping");
        assert_eq!(
            message.segments[0].at_target().as_deref(),
            Some("bot-member-openid")
        );
        assert_eq!(
            message.segments[0]
                .data
                .get("is_self")
                .and_then(Value::as_bool),
            Some(true)
        );
        assert!(event.is_at_self());
        assert_eq!(
            event.extensions.get("to_me").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[tokio::test]
    async fn decode_full_group_bot_mention_synthesizes_removed_prefix() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 45,
                "t": "GROUP_MESSAGE_CREATE",
                "d": {
                    "id": "msg-full-group-at-clean",
                    "content": " \u{a0}/ping",
                    "group_openid": "group-openid",
                    "author": {"member_openid": "member-openid"},
                    "mentions": [{
                        "scope": "single",
                        "bot": true,
                        "id": "bot-user-id",
                        "is_you": true,
                        "member_openid": "bot-member-openid",
                        "username": "bot"
                    }]
                }
            })))
            .await
            .unwrap();

        let message = event.message.as_ref().unwrap();
        assert_eq!(message.plain_text(), "/ping");
        assert_eq!(
            message.segments[0].at_target().as_deref(),
            Some("bot-member-openid")
        );
        assert!(event.is_at_self());
    }

    #[tokio::test]
    async fn quick_op_group_reply_uses_msg_id_without_event_id() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 44,
                "t": "GROUP_MESSAGE_CREATE",
                "id": "event-id",
                "d": {
                    "id": "message-id",
                    "content": "/ping",
                    "group_openid": "group-openid",
                    "author": {"member_openid": "member-openid"}
                }
            })))
            .await
            .unwrap();

        let packet = QqBotAdapter
            .quick_op_from_event_and_patch(
                &event,
                &QuickOpPatch {
                    reply_text: Some("pong".to_string()),
                    ..QuickOpPatch::default()
                },
            )
            .unwrap()
            .unwrap();

        assert_eq!(
            packet.payload.get("msg_id").and_then(Value::as_str),
            Some("message-id")
        );
        assert_eq!(
            packet.payload.get("group_openid").and_then(Value::as_str),
            Some("group-openid")
        );
        assert!(packet.payload.get("target").is_none());
        assert_eq!(
            packet.payload.get("msg_seq").and_then(Value::as_i64),
            Some(1)
        );
        assert!(packet.payload.get("event_id").is_none());
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
        assert!(event.is_at_self());
        assert_eq!(event.message.unwrap().plain_text(), "/help");
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
        assert!(event.is_private());
    }

    #[tokio::test]
    async fn decode_official_group_member_events_as_notices() {
        for (event_type, notice_type) in [
            ("GROUP_MEMBER_ADD", "group_member_add"),
            ("GROUP_MEMBER_REMOVE", "group_member_remove"),
        ] {
            let event = QqBotAdapter
                .decode_event(packet(json!({
                    "op": 0,
                    "s": 50,
                    "t": event_type,
                    "id": "member-event",
                    "d": {
                        "group_openid": "group-openid",
                        "member_openid": "member-openid",
                        "user_openid": "user-openid",
                        "timestamp": 1784276757
                    }
                })))
                .await
                .unwrap();

            assert_eq!(event.kind, EventKind::Notice);
            assert_eq!(event.chat.as_ref().unwrap().kind, "group");
            assert_eq!(event.chat.as_ref().unwrap().id, "group-openid");
            assert_eq!(event.sender_id(), Some("member-openid"));
            assert_eq!(event.time, Some(1784276757));
            assert_eq!(
                event.raw_json.get("notice_type").and_then(Value::as_str),
                Some(notice_type)
            );
            assert_eq!(
                event
                    .extensions
                    .get("member_openid")
                    .and_then(Value::as_str),
                Some("member-openid")
            );
        }
    }

    #[tokio::test]
    async fn decode_subscribe_message_status_as_notice() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 51,
                "t": "SUBSCRIBE_MESSAGE_STATUS",
                "id": "subscribe-event",
                "d": {
                    "group_openid": "group-openid",
                    "openid": "user-openid",
                    "result": [
                        {
                            "template_id": 10001,
                            "custom_template_id": "tpl-1",
                            "op": 1,
                            "subscribe_id": "sub-1"
                        }
                    ]
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.kind, EventKind::Notice);
        assert_eq!(
            event.raw_json.get("notice_type").and_then(Value::as_str),
            Some("subscribe_message_status")
        );
        assert_eq!(event.chat.as_ref().unwrap().kind, "group");
        assert_eq!(event.actor.as_ref().unwrap().id, "user-openid");
    }

    #[tokio::test]
    async fn decode_interaction_keeps_inner_id_distinct_from_message_id() {
        let event = QqBotAdapter
            .decode_event(packet(json!({
                "op": 0,
                "s": 52,
                "t": "INTERACTION_CREATE",
                "id": "gateway-event-id",
                "d": {
                    "id": "interaction-id",
                    "type": 11,
                    "scene": "group",
                    "group_openid": "group-openid",
                    "group_member_openid": "member-openid",
                    "data": {
                        "type": 11,
                        "resolved": {"button_data": "/ping"}
                    }
                }
            })))
            .await
            .unwrap();

        assert_eq!(event.kind, EventKind::Notice);
        assert_eq!(event.message_id_str(), None);
        assert_eq!(
            event.extensions.get("event_id").and_then(Value::as_str),
            Some("gateway-event-id")
        );
        assert_eq!(
            event
                .raw_json
                .pointer("/qqbot_payload/id")
                .and_then(Value::as_str),
            Some("interaction-id")
        );
        assert_eq!(event.sender_id(), Some("member-openid"));
        assert_eq!(event.chat.as_ref().unwrap().id, "group-openid");
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
        assert!(packet.payload.get("event_id").is_none());
    }

    #[tokio::test]
    async fn encode_proactive_group_message_omits_reply_fields() {
        let action = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "message": "announcement",
            }),
        );

        let packet = QqBotAdapter.encode_action(&action).await.unwrap();

        assert_eq!(
            packet.payload.get("content").and_then(Value::as_str),
            Some("announcement")
        );
        assert!(packet.payload.get("msg_id").is_none());
        assert!(packet.payload.get("msg_seq").is_none());
        assert!(packet.payload.get("event_id").is_none());
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
        assert!(packet.payload.get("content").is_none());
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
    async fn encode_channel_ark_and_embed_without_group_msg_type() {
        let ark_action = action(
            "send_channel_msg",
            json!({
                "channel_id": "channel-1",
                "message": Message::from_segments(vec![
                    Segment::new("ark")
                        .with("template_id", json!(37))
                        .with("kv", json!([{ "key": "#TITLE#", "value": "title" }])),
                ])
                .to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );
        let embed_action = action(
            "send_channel_msg",
            json!({
                "channel_id": "channel-1",
                "message": Message::from_segments(vec![
                    Segment::new("embed")
                        .with("title", json!("embed message"))
                        .with("fields", json!([{ "name": "hello world" }])),
                ])
                .to_onebot_value(),
                "msg_id": "msg-1",
            }),
        );

        let ark_packet = QqBotAdapter.encode_action(&ark_action).await.unwrap();
        let embed_packet = QqBotAdapter.encode_action(&embed_action).await.unwrap();

        assert!(ark_packet.payload.get("msg_type").is_none());
        assert!(embed_packet.payload.get("msg_type").is_none());
        assert_eq!(
            ark_packet
                .payload
                .get("ark")
                .and_then(|value| value.get("template_id"))
                .and_then(Value::as_i64),
            Some(37)
        );
        assert_eq!(
            embed_packet
                .payload
                .get("embed")
                .and_then(|value| value.get("title"))
                .and_then(Value::as_str),
            Some("embed message")
        );
    }

    #[tokio::test]
    async fn encode_c2c_embed_is_rejected() {
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

        let error = QqBotAdapter.encode_action(&action).await.unwrap_err();
        assert!(error.to_string().contains("c2c messages support"));
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
        assert!(packet.payload.get("image").is_none());
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
    async fn encode_inline_base64_media_for_group_and_channel() {
        let image = format!(
            "base64://{}",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        );
        let message =
            Message::from_segments(vec![Segment::new("image").with("file", json!(image))]);

        let group_packet = QqBotAdapter
            .encode_action(&action(
                "send_group_msg",
                json!({
                    "group_openid": "group-openid",
                    "message": message.to_onebot_value(),
                }),
            ))
            .await
            .unwrap();
        assert_eq!(
            group_packet
                .payload
                .get("media_upload")
                .and_then(|value| value.get("base64"))
                .and_then(Value::as_str),
            Some(
                "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
            )
        );
        assert!(group_packet.payload.get("image").is_none());

        let channel_packet = QqBotAdapter
            .encode_action(&action(
                "send_channel_msg",
                json!({
                    "channel_id": "channel-1",
                    "message": message.to_onebot_value(),
                }),
            ))
            .await
            .unwrap();
        assert!(channel_packet.payload.get("image").is_none());
        assert!(channel_packet.payload.get("media_upload").is_some());
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
    async fn encode_current_group_and_c2c_message_fields() {
        let group = action(
            "send_group_msg",
            json!({
                "group_openid": "group-openid",
                "msg_type": 8,
                "card": {
                    "type": "tuwen",
                    "content": {"title": "Title", "url": "https://example.invalid"}
                },
                "msg_id": "message-id",
                "message_reference": {"message_id": "quoted-id"}
            }),
        );
        let c2c = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "input_notify": {"input_type": 1, "input_second": 30},
                "msg_id": "message-id"
            }),
        );

        let group_packet = QqBotAdapter.encode_action(&group).await.unwrap();
        let c2c_packet = QqBotAdapter.encode_action(&c2c).await.unwrap();

        assert_eq!(
            group_packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(8)
        );
        assert_eq!(
            group_packet
                .payload
                .pointer("/message_reference/message_id")
                .and_then(Value::as_str),
            Some("quoted-id")
        );
        assert_eq!(
            c2c_packet.payload.get("msg_type").and_then(Value::as_i64),
            Some(6)
        );
        assert_eq!(
            c2c_packet
                .payload
                .pointer("/input_notify/input_second")
                .and_then(Value::as_i64),
            Some(30)
        );
    }

    #[tokio::test]
    async fn encode_wakeup_message_enforces_reply_exclusivity() {
        let proactive = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "message": "wake up",
                "is_wakeup": true
            }),
        );
        let conflicting = action(
            "send_private_msg",
            json!({
                "openid": "user-openid",
                "message": "wake up",
                "is_wakeup": true,
                "msg_id": "message-id"
            }),
        );

        let packet = QqBotAdapter.encode_action(&proactive).await.unwrap();
        assert_eq!(
            packet.payload.get("is_wakeup").and_then(Value::as_bool),
            Some(true)
        );
        assert!(QqBotAdapter.encode_action(&conflicting).await.is_err());
    }

    #[tokio::test]
    async fn encode_interaction_ack_route() {
        let packet = QqBotAdapter
            .encode_action(&action(
                "ack_interaction",
                json!({"interaction_id": "interaction-id", "code": 0}),
            ))
            .await
            .unwrap();

        assert_eq!(
            packet.payload.get("route").and_then(Value::as_str),
            Some("interaction_ack")
        );
        assert_eq!(
            packet.payload.get("interaction_id").and_then(Value::as_str),
            Some("interaction-id")
        );
    }

    #[tokio::test]
    async fn encode_recall_routes_all_official_message_scenes() {
        for (params, route, target_key, target) in [
            (
                json!({"channel_id": "channel-1", "message_id": "message-1"}),
                "channel_recall_message",
                "channel_id",
                "channel-1",
            ),
            (
                json!({"group_openid": "group-1", "message_id": "message-1"}),
                "group_recall_message",
                "group_openid",
                "group-1",
            ),
            (
                json!({"openid": "user-1", "message_id": "message-1"}),
                "c2c_recall_message",
                "openid",
                "user-1",
            ),
            (
                json!({"guild_id": "guild-1", "message_id": "message-1"}),
                "dms_recall_message",
                "guild_id",
                "guild-1",
            ),
        ] {
            let packet = QqBotAdapter
                .encode_action(&action("recall_msg", params))
                .await
                .unwrap();
            assert_eq!(
                packet.payload.get("route").and_then(Value::as_str),
                Some(route)
            );
            assert_eq!(
                packet.payload.get(target_key).and_then(Value::as_str),
                Some(target)
            );
            assert_eq!(
                packet.payload.get("message_id").and_then(Value::as_str),
                Some("message-1")
            );
        }
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
    async fn decode_guild_and_channel_object_ids_as_chat_context() {
        for (event_type, data, kind, context_key, context_id) in [
            (
                "GUILD_CREATE",
                json!({
                    "id": "guild-1",
                    "op_user_id": "operator-1",
                    "joined_at": "2026-01-01T00:00:00+08:00"
                }),
                "guild",
                "guild_id",
                "guild-1",
            ),
            (
                "CHANNEL_CREATE",
                json!({
                    "id": "channel-1",
                    "guild_id": "guild-1",
                    "owner_id": "owner-1"
                }),
                "channel",
                "channel_id",
                "channel-1",
            ),
        ] {
            let event = QqBotAdapter
                .decode_event(packet(json!({
                    "op": 0,
                    "t": event_type,
                    "d": data
                })))
                .await
                .unwrap();

            assert_eq!(event.chat.as_ref().unwrap().kind, kind);
            assert_eq!(event.chat.as_ref().unwrap().id, context_id);
            assert_eq!(
                event.extensions.get(context_key).and_then(Value::as_str),
                Some(context_id)
            );
            assert!(event.sender_id().is_some());
        }
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
        assert_eq!(
            packet
                .payload
                .pointer("/message_reference/message_id")
                .and_then(Value::as_str),
            Some("reply-msg")
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

        let official_names = vec![
            "GROUP_AND_C2C_EVENT".to_string(),
            "PUBLIC_GUILD_MESSAGES".to_string(),
            "DIRECT_MESSAGE".to_string(),
            "FORUMS_EVENT".to_string(),
        ];
        assert_eq!(
            qq_official_intents_value(&official_names).unwrap(),
            (1_u64 << 25) | (1_u64 << 30) | (1_u64 << 12) | (1_u64 << 28)
        );
    }

    #[test]
    fn recognizes_all_current_autogenerated_events() {
        for event_type in [
            "C2C_MESSAGE_CREATE",
            "GROUP_AT_MESSAGE_CREATE",
            "GROUP_MESSAGE_CREATE",
        ] {
            assert!(message_kind(event_type).is_some(), "{event_type}");
        }
        for event_type in [
            "C2C_MSG_RECEIVE",
            "C2C_MSG_REJECT",
            "CHANNEL_CREATE",
            "CHANNEL_DELETE",
            "CHANNEL_UPDATE",
            "FRIEND_ADD",
            "FRIEND_DEL",
            "GROUP_ADD_ROBOT",
            "GROUP_DEL_ROBOT",
            "GROUP_MEMBER_ADD",
            "GROUP_MEMBER_REMOVE",
            "GROUP_MSG_RECEIVE",
            "GROUP_MSG_REJECT",
            "GUILD_CREATE",
            "GUILD_DELETE",
            "GUILD_UPDATE",
            "INTERACTION_CREATE",
            "SUBSCRIBE_MESSAGE_STATUS",
        ] {
            assert!(qqbot_notice_type(event_type).is_some(), "{event_type}");
        }
    }
}
