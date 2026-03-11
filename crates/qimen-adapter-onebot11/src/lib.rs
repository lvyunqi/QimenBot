use async_trait::async_trait;
use qimen_error::{QimenError, Result};
use qimen_message::Message;
use qimen_protocol_core::{
    ActionStatus, CapabilitySet, EventKind, IncomingPacket, NormalizedActionRequest,
    NormalizedActionResponse, NormalizedEvent, OutgoingPacket, ProtocolAdapter, ProtocolId,
    QuickOpPatch, TransportMode, value_to_lossless_id,
};
use serde_json::{Map, Value, json};

#[derive(Default)]
pub struct OneBot11Adapter;

#[async_trait]
impl ProtocolAdapter for OneBot11Adapter {
    fn protocol_id(&self) -> ProtocolId {
        ProtocolId::OneBot11
    }

    fn supported_transports(&self) -> &'static [TransportMode] {
        const SUPPORTED: &[TransportMode] = &[
            TransportMode::WsForward,
            TransportMode::WsReverse,
            TransportMode::HttpApi,
            TransportMode::HttpPost,
        ];
        SUPPORTED
    }

    fn capabilities(&self) -> CapabilitySet {
        CapabilitySet {
            features: vec![
                "send_private_message".to_string(),
                "send_group_message".to_string(),
                "quick_operation".to_string(),
                "echo_correlation".to_string(),
            ],
        }
    }

    async fn decode_event(&self, packet: IncomingPacket) -> Result<NormalizedEvent> {
        let actor = packet.payload.get("user_id").map(|user_id| qimen_protocol_core::ActorRef {
            id: value_to_lossless_id(user_id),
            display_name: packet
                .payload
                .get("sender")
                .and_then(Value::as_object)
                .and_then(|sender| sender.get("nickname"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        });

        let chat = match packet.payload.get("message_type").and_then(Value::as_str) {
            Some("private") => packet.payload.get("user_id").map(|user_id| qimen_protocol_core::ChatRef {
                id: value_to_lossless_id(user_id),
                kind: "private".to_string(),
            }),
            Some("group") => packet.payload.get("group_id").map(|group_id| qimen_protocol_core::ChatRef {
                id: value_to_lossless_id(group_id),
                kind: "group".to_string(),
            }),
            Some(other) => Some(qimen_protocol_core::ChatRef {
                id: packet
                    .payload
                    .get("group_id")
                    .or_else(|| packet.payload.get("user_id"))
                    .map(value_to_lossless_id)
                    .unwrap_or_default(),
                kind: other.to_string(),
            }),
            None => None,
        };

        let kind = match packet
            .payload
            .get("post_type")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
        {
            "message" => EventKind::Message,
            "message_sent" => EventKind::MessageSent,
            "notice" => EventKind::Notice,
            "request" => EventKind::Request,
            "meta_event" => EventKind::Meta,
            other => EventKind::Internal(other.to_string()),
        };

        let message = packet.payload.get("message").map(Message::from_onebot_value);

        let extensions = packet
            .payload
            .as_object()
            .map(clone_known_extensions)
            .unwrap_or_default();

        Ok(NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: packet.bot_instance,
            transport_mode: packet.transport_mode,
            time: packet.payload.get("time").and_then(Value::as_i64),
            kind,
            message,
            actor,
            chat,
            raw_json: packet.payload,
            raw_bytes: packet.raw_bytes,
            extensions,
        })
    }

    async fn decode_action_response(
        &self,
        packet: IncomingPacket,
    ) -> Result<NormalizedActionResponse> {
        let status = match packet
            .payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("failed")
        {
            "ok" => ActionStatus::Ok,
            "async" => ActionStatus::Async,
            _ => ActionStatus::Failed,
        };

        Ok(NormalizedActionResponse {
            protocol: ProtocolId::OneBot11,
            bot_instance: packet.bot_instance,
            status,
            retcode: packet
                .payload
                .get("retcode")
                .and_then(Value::as_i64)
                .unwrap_or(-1),
            data: packet.payload.get("data").cloned().unwrap_or(Value::Null),
            echo: packet.payload.get("echo").cloned(),
            latency_ms: 0,
            raw_json: packet.payload,
        })
    }

    async fn encode_action(&self, req: &NormalizedActionRequest) -> Result<OutgoingPacket> {
        Ok(OutgoingPacket {
            payload: json!({
                "action": req.action,
                "params": req.params,
                "echo": req.echo,
            }),
        })
    }

    fn quick_op_from_event_and_patch(
        &self,
        _event: &NormalizedEvent,
        patch: &QuickOpPatch,
    ) -> Result<Option<OutgoingPacket>> {
        if patch.reply_text.is_none() && patch.approve.is_none() && patch.reason.is_none() {
            return Ok(None);
        }

        let payload = json!({
            "reply": patch.reply_text,
            "approve": patch.approve,
            "reason": patch.reason,
        });

        Ok(Some(OutgoingPacket { payload }))
    }
}

pub fn ensure_onebot11(payload: &Value) -> Result<()> {
    if payload.get("post_type").is_none() && payload.get("status").is_none() {
        return Err(QimenError::Protocol(
            "payload is not a recognized OneBot11 event or action response".to_string(),
        ));
    }
    Ok(())
}

fn clone_known_extensions(payload: &Map<String, Value>) -> Map<String, Value> {
    payload
        .iter()
        .filter(|(key, _)| !is_core_onebot_field(key))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn is_core_onebot_field(key: &str) -> bool {
    matches!(
        key,
        "time"
            | "self_id"
            | "post_type"
            | "message_type"
            | "sub_type"
            | "message_id"
            | "user_id"
            | "group_id"
            | "message"
            | "raw_message"
            | "font"
            | "sender"
            | "anonymous"
            | "notice_type"
            | "request_type"
            | "meta_event_type"
            | "comment"
            | "flag"
            | "status"
            | "interval"
            | "operator_id"
            | "target_id"
            | "file"
    )
}


#[cfg(test)]
mod tests {
    use super::*;
    use qimen_protocol_core::{EventKind, IncomingPacket, ProtocolAdapter, ProtocolId, TransportMode};

    fn make_packet(payload: Value) -> IncomingPacket {
        IncomingPacket {
            protocol: ProtocolId::OneBot11,
            transport_mode: TransportMode::WsForward,
            bot_instance: "123456".to_string(),
            payload,
            raw_bytes: None,
        }
    }

    #[tokio::test]
    async fn decode_private_message_event() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "message",
            "message_type": "private",
            "self_id": 123456,
            "user_id": 10001,
            "message_id": 1,
            "message": "hello",
            "raw_message": "hello",
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::Message);
        assert_eq!(event.bot_instance, "123456");
        assert!(event.chat.is_some());
        let chat = event.chat.unwrap();
        assert_eq!(chat.id, "10001");
        assert_eq!(chat.kind, "private");
    }

    #[tokio::test]
    async fn decode_group_message_event() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "message",
            "message_type": "group",
            "self_id": 123456,
            "group_id": 20002,
            "user_id": 10001,
            "message_id": 2,
            "message": "test",
            "raw_message": "test",
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert!(event.chat.is_some());
        let chat = event.chat.unwrap();
        assert_eq!(chat.id, "20002");
        assert_eq!(chat.kind, "group");
    }

    #[tokio::test]
    async fn decode_notice_event() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "notice",
            "notice_type": "group_upload",
            "self_id": 123456,
            "group_id": 20002,
            "user_id": 10001,
            "time": 1234567890,
            "file": { "name": "test.txt", "size": 1024 }
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::Notice);
    }

    #[tokio::test]
    async fn decode_request_event() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "request",
            "request_type": "friend",
            "self_id": 123456,
            "user_id": 10001,
            "comment": "please add me",
            "flag": "abc123",
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::Request);
    }

    #[tokio::test]
    async fn decode_meta_heartbeat() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "meta_event",
            "meta_event_type": "heartbeat",
            "self_id": 123456,
            "time": 1234567890,
            "status": {},
            "interval": 5000
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::Meta);
    }

    #[tokio::test]
    async fn decode_message_sent_event() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "message_sent",
            "message_type": "private",
            "self_id": 123456,
            "user_id": 10001,
            "message_id": 3,
            "message": "sent by bot",
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::MessageSent);
    }

    #[tokio::test]
    async fn decode_unknown_post_type_returns_internal() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "custom_event",
            "self_id": 123456,
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert_eq!(event.kind, EventKind::Internal("custom_event".to_string()));
    }

    #[test]
    fn ensure_onebot11_rejects_unrecognized_payload() {
        let raw = serde_json::json!({"invalid": "data"});
        let result = ensure_onebot11(&raw);
        assert!(result.is_err());
    }

    #[test]
    fn ensure_onebot11_accepts_event_payload() {
        let raw = serde_json::json!({"post_type": "message"});
        assert!(ensure_onebot11(&raw).is_ok());
    }

    #[test]
    fn ensure_onebot11_accepts_action_response() {
        let raw = serde_json::json!({"status": "ok", "retcode": 0, "data": null});
        assert!(ensure_onebot11(&raw).is_ok());
    }

    #[test]
    fn value_to_lossless_id_handles_number() {
        let val = serde_json::json!(12345);
        assert_eq!(value_to_lossless_id(&val), "12345");
    }

    #[test]
    fn value_to_lossless_id_handles_string() {
        let val = serde_json::json!("myid");
        assert_eq!(value_to_lossless_id(&val), "myid");
    }

    #[tokio::test]
    async fn actor_extracted_from_user_id() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "message",
            "message_type": "private",
            "self_id": 123456,
            "user_id": 99999,
            "message_id": 1,
            "message": "hi",
            "time": 1234567890
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert!(event.actor.is_some());
        assert_eq!(event.actor.unwrap().id, "99999");
    }

    #[tokio::test]
    async fn extensions_exclude_core_fields() {
        let adapter = OneBot11Adapter;
        let packet = make_packet(serde_json::json!({
            "post_type": "message",
            "message_type": "private",
            "self_id": 123456,
            "user_id": 10001,
            "message_id": 1,
            "message": "hello",
            "time": 1234567890,
            "custom_field": "custom_value"
        }));
        let event = adapter.decode_event(packet).await.unwrap();
        assert!(event.extensions.contains_key("custom_field"));
        assert!(!event.extensions.contains_key("post_type"));
        assert!(!event.extensions.contains_key("user_id"));
    }
}
