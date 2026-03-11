use qimen_message::Message;
use qimen_error::{QimenError, Result};
use qimen_plugin_api::{OwnedTaskFuture, RuntimeBotContext, TaskHandle};
use qimen_runtime::command_dispatch::{CommandDispatchSignal, CommandDispatcher};
use qimen_runtime::onebot11_dispatch::{
    NoticeRoute, OneBotSystemDispatchSignal, OneBotSystemDispatcher, SystemEventContext,
    route_onebot_system_event,
};
use qimen_protocol_core::{
    ActionStatus, CapabilitySet, EventKind, NormalizedActionRequest, NormalizedActionResponse,
    NormalizedEvent, ProtocolId, TransportMode,
};
use serde_json::Map;

struct TestRuntimeBotContext;

#[async_trait::async_trait]
impl RuntimeBotContext for TestRuntimeBotContext {
    fn bot_instance(&self) -> &str {
        "qq-main"
    }

    fn protocol(&self) -> ProtocolId {
        ProtocolId::OneBot11
    }

    fn capabilities(&self) -> &CapabilitySet {
        static CAPABILITIES: std::sync::OnceLock<CapabilitySet> = std::sync::OnceLock::new();
        CAPABILITIES.get_or_init(CapabilitySet::default)
    }

    async fn send_action(&self, _req: NormalizedActionRequest) -> Result<NormalizedActionResponse> {
        Err(QimenError::Runtime("test runtime does not send actions".to_string()))
    }

    async fn reply(&self, _event: &NormalizedEvent, _message: Message) -> Result<NormalizedActionResponse> {
        Ok(NormalizedActionResponse {
            protocol: ProtocolId::OneBot11,
            bot_instance: "qq-main".to_string(),
            status: ActionStatus::Ok,
            retcode: 0,
            data: serde_json::Value::Null,
            echo: None,
            latency_ms: 0,
            raw_json: serde_json::json!({
                "status": "ok",
                "retcode": 0,
                "data": null
            }),
        })
    }

    fn spawn_owned(&self, name: &str, _fut: OwnedTaskFuture) -> TaskHandle {
        TaskHandle {
            name: name.to_string(),
        }
    }
}

static TEST_RUNTIME: TestRuntimeBotContext = TestRuntimeBotContext;

fn sample_private_event(text: &str) -> NormalizedEvent {
    NormalizedEvent {
        protocol: ProtocolId::OneBot11,
        bot_instance: "qq-main".to_string(),
        transport_mode: TransportMode::WsForward,
        time: Some(1),
        kind: EventKind::Message,
        message: Some(Message::text(text)),
        actor: None,
        chat: Some(qimen_protocol_core::ChatRef {
            id: "10001".to_string(),
            kind: "private".to_string(),
        }),
        raw_json: serde_json::json!({
            "self_id": 123456,
            "post_type": "message",
            "message_type": "private",
            "user_id": 10001,
            "message": text,
        }),
        raw_bytes: None,
        extensions: Map::new(),
    }
}

#[tokio::test]
async fn builtin_command_dispatcher_replies_to_ping() {
    let dispatcher = CommandDispatcher::with_default_handlers();
    let event = sample_private_event("ping");

    let result = dispatcher
        .dispatch("qq-main", &event, &TEST_RUNTIME)
        .execute()
        .await;
    match result {
        Some(CommandDispatchSignal::Reply(message)) => {
            assert_eq!(message.plain_text(), "pong");
        }
        Some(CommandDispatchSignal::Builtin(_)) => panic!("expected reply signal, got builtin action"),
        Some(CommandDispatchSignal::DynamicCommand { .. }) => panic!("expected reply signal, got dynamic command action"),
        None => panic!("expected reply signal"),
    }
}

#[tokio::test]
async fn system_dispatcher_replies_to_poke_notice() {
    let dispatcher = OneBotSystemDispatcher::with_default_handlers();
    let payload = serde_json::json!({
        "post_type": "notice",
        "notice_type": "notify",
        "sub_type": "poke",
        "self_id": 123456,
        "user_id": 10001,
        "group_id": 20002,
        "target_id": 123456
    });

    let result = dispatcher
        .dispatch(SystemEventContext {
            bot_id: "qq-main",
            payload: &payload,
            runtime: &TEST_RUNTIME,
            auto_approve_friend_requests: false,
            auto_approve_group_invites: false,
            auto_reply_poke_enabled: true,
            auto_reply_poke_message: Some("你好 {user_id} 来自 {group_id}"),
            auto_approve_friend_request_user_whitelist: &[],
            auto_approve_friend_request_user_blacklist: &[],
            auto_approve_friend_request_comment_keywords: &[],
            auto_reject_friend_request_comment_keywords: &[],
            auto_approve_friend_request_remark: None,
            auto_approve_group_invite_user_whitelist: &[],
            auto_approve_group_invite_user_blacklist: &[],
            auto_approve_group_invite_group_whitelist: &[],
            auto_approve_group_invite_group_blacklist: &[],
            auto_approve_group_invite_comment_keywords: &[],
            auto_reject_group_invite_comment_keywords: &[],
            auto_reject_group_invite_reason: None,
        })
        .await;

    match result {
        Some(OneBotSystemDispatchSignal::NoticeReply { message }) => {
            assert_eq!(message, "你好 10001 来自 20002");
        }
        other => panic!("unexpected dispatch result: {other:?}"),
    }
}

#[tokio::test]
async fn system_dispatcher_ignores_poke_targeting_others() {
    let dispatcher = OneBotSystemDispatcher::with_default_handlers();
    let payload = serde_json::json!({
        "post_type": "notice",
        "notice_type": "notify",
        "sub_type": "poke",
        "self_id": 123456,
        "user_id": 10001,
        "group_id": 20002,
        "target_id": 99999
    });

    let result = dispatcher
        .dispatch(SystemEventContext {
            bot_id: "qq-main",
            payload: &payload,
            runtime: &TEST_RUNTIME,
            auto_approve_friend_requests: false,
            auto_approve_group_invites: false,
            auto_reply_poke_enabled: true,
            auto_reply_poke_message: Some("你好 {user_id}"),
            auto_approve_friend_request_user_whitelist: &[],
            auto_approve_friend_request_user_blacklist: &[],
            auto_approve_friend_request_comment_keywords: &[],
            auto_reject_friend_request_comment_keywords: &[],
            auto_approve_friend_request_remark: None,
            auto_approve_group_invite_user_whitelist: &[],
            auto_approve_group_invite_user_blacklist: &[],
            auto_approve_group_invite_group_whitelist: &[],
            auto_approve_group_invite_group_blacklist: &[],
            auto_approve_group_invite_comment_keywords: &[],
            auto_reject_group_invite_comment_keywords: &[],
            auto_reject_group_invite_reason: None,
        })
        .await;

    // AutoReplyPokeHandler should NOT emit a reply when target is not the bot
    match result {
        Some(OneBotSystemDispatchSignal::NoticeReply { .. }) => {
            panic!("should not reply when poke targets another user");
        }
        _ => {} // Continue or other signals are fine
    }
}

#[test]
fn routes_notify_notice_to_subtype() {
    let payload = serde_json::json!({
        "post_type": "notice",
        "notice_type": "notify",
        "sub_type": "poke"
    });

    let route = route_onebot_system_event(&payload);
    assert_eq!(route, Some(qimen_runtime::onebot11_dispatch::OneBotSystemRoute::Notice(NoticeRoute::PrivatePoke)));
}
