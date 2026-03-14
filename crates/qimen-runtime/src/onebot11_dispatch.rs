mod message_sent_handler;
mod meta_handler;
mod notice_handler;
mod policy;
mod request_handler;

use async_trait::async_trait;
use qimen_plugin_api::{
    PluginBundle, RuntimeBotContext, SystemMetaRoute, SystemNoticeRoute, SystemPlugin,
    SystemPluginContext, SystemPluginSignal, SystemRequestRoute,
};
use serde_json::Value;
use std::sync::Arc;

use self::message_sent_handler::LoggingMessageSentHandler;
use self::meta_handler::LoggingMetaHandler;
use self::notice_handler::{AutoReplyPokeHandler, LoggingNoticeHandler};
use self::request_handler::{AutoApproveRequestHandler, LoggingRequestHandler};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneBotSystemRoute {
    Notice(NoticeRoute),
    Request(RequestRoute),
    Meta(MetaRoute),
    MessageSent(MessageSentRoute),
}

impl From<NoticeRoute> for OneBotSystemRoute {
    fn from(value: NoticeRoute) -> Self {
        OneBotSystemRoute::Notice(value)
    }
}

impl From<RequestRoute> for OneBotSystemRoute {
    fn from(value: RequestRoute) -> Self {
        OneBotSystemRoute::Request(value)
    }
}

impl From<MetaRoute> for OneBotSystemRoute {
    fn from(value: MetaRoute) -> Self {
        OneBotSystemRoute::Meta(value)
    }
}

impl From<MessageSentRoute> for OneBotSystemRoute {
    fn from(value: MessageSentRoute) -> Self {
        OneBotSystemRoute::MessageSent(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeRoute {
    GroupUpload,
    GroupAdminSet,
    GroupAdminUnset,
    GroupDecreaseLeave,
    GroupDecreaseKick,
    GroupDecreaseKickMe,
    GroupDecreaseOther(String),
    GroupIncreaseApprove,
    GroupIncreaseInvite,
    GroupIncreaseOther(String),
    GroupBanBan,
    GroupBanLiftBan,
    GroupBanOther(String),
    FriendAdd,
    GroupRecall,
    FriendRecall,
    GroupPoke,
    PrivatePoke,
    NotifyLuckyKing,
    NotifyHonor(Option<String>),
    NotifyOther(String),
    GroupCard,
    OfflineFile,
    ClientStatus,
    EssenceAdd,
    EssenceDelete,
    EssenceOther(String),
    GroupReaction,
    MessageEmojiLike,
    ChannelCreated,
    ChannelDestroyed,
    ChannelUpdated,
    GuildMessageReactionsUpdated,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestRoute {
    Friend,
    GroupAdd,
    GroupInvite,
    Unknown {
        request_type: String,
        sub_type: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetaRoute {
    LifecycleEnable,
    LifecycleDisable,
    LifecycleConnect,
    LifecycleOther(String),
    Heartbeat,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageSentRoute {
    Private,
    Group,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OneBotSystemDispatchSignal {
    Continue(OneBotSystemRoute),
    Heartbeat(u64),
    AutoApproveFriend {
        flag: String,
        remark: Option<String>,
    },
    AutoRejectFriend {
        flag: String,
        reason: Option<String>,
    },
    AutoApproveGroupInvite {
        flag: String,
        sub_type: String,
    },
    AutoRejectGroupInvite {
        flag: String,
        sub_type: String,
        reason: Option<String>,
    },
    NoticeReply {
        message: String,
    },
}

#[derive(Clone)]
pub struct SystemEventContext<'a> {
    pub bot_id: &'a str,
    pub payload: &'a Value,
    pub runtime: &'a dyn RuntimeBotContext,
    pub auto_approve_friend_requests: bool,
    pub auto_approve_group_invites: bool,
    pub auto_reply_poke_enabled: bool,
    pub auto_reply_poke_message: Option<&'a str>,
    pub auto_approve_friend_request_user_whitelist: &'a [String],
    pub auto_approve_friend_request_user_blacklist: &'a [String],
    pub auto_approve_friend_request_comment_keywords: &'a [String],
    pub auto_reject_friend_request_comment_keywords: &'a [String],
    pub auto_approve_friend_request_remark: Option<&'a str>,
    pub auto_approve_group_invite_user_whitelist: &'a [String],
    pub auto_approve_group_invite_user_blacklist: &'a [String],
    pub auto_approve_group_invite_group_whitelist: &'a [String],
    pub auto_approve_group_invite_group_blacklist: &'a [String],
    pub auto_approve_group_invite_comment_keywords: &'a [String],
    pub auto_reject_group_invite_comment_keywords: &'a [String],
    pub auto_reject_group_invite_reason: Option<&'a str>,
}

#[async_trait]
pub trait OneBotSystemEventHandler: Send + Sync {
    async fn on_notice(
        &self,
        _ctx: &SystemEventContext<'_>,
        _route: &NoticeRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        None
    }

    async fn on_request(
        &self,
        _ctx: &SystemEventContext<'_>,
        _route: &RequestRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        None
    }

    async fn on_meta(
        &self,
        _ctx: &SystemEventContext<'_>,
        _route: &MetaRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        None
    }

    async fn on_message_sent(
        &self,
        _ctx: &SystemEventContext<'_>,
        _route: &MessageSentRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        None
    }
}

pub struct OneBotSystemDispatcher {
    handlers: Vec<Arc<dyn OneBotSystemEventHandler>>,
    plugins: Vec<Arc<dyn SystemPlugin>>,
    dynamic_notice_routes: Vec<(String, String, String)>,
    dynamic_request_routes: Vec<(String, String, String)>,
    dynamic_meta_routes: Vec<(String, String, String)>,
}

impl OneBotSystemDispatcher {
    pub fn with_default_handlers() -> Self {
        Self {
            handlers: vec![
                Arc::new(LoggingNoticeHandler),
                Arc::new(AutoReplyPokeHandler),
                Arc::new(LoggingRequestHandler),
                Arc::new(AutoApproveRequestHandler),
                Arc::new(LoggingMetaHandler),
                Arc::new(LoggingMessageSentHandler),
            ],
            plugins: Vec::new(),
            dynamic_notice_routes: Vec::new(),
            dynamic_request_routes: Vec::new(),
            dynamic_meta_routes: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn register_handler(&mut self, handler: Arc<dyn OneBotSystemEventHandler>) {
        self.handlers.push(handler);
    }

    pub fn register_plugin(&mut self, plugin: Arc<dyn SystemPlugin>) {
        self.plugins.push(plugin);
    }

    pub fn register_plugins(&mut self, bundle: &PluginBundle) {
        for plugin in &bundle.system_plugins {
            self.register_plugin(plugin.clone());
        }
    }

    pub fn register_dynamic_notice_route(
        &mut self,
        plugin_id: String,
        route: String,
        callback_symbol: String,
    ) {
        self.dynamic_notice_routes.push((plugin_id, route, callback_symbol));
    }

    pub fn register_dynamic_request_route(
        &mut self,
        plugin_id: String,
        route: String,
        callback_symbol: String,
    ) {
        self.dynamic_request_routes.push((plugin_id, route, callback_symbol));
    }

    pub fn register_dynamic_meta_route(
        &mut self,
        plugin_id: String,
        route: String,
        callback_symbol: String,
    ) {
        self.dynamic_meta_routes.push((plugin_id, route, callback_symbol));
    }

    pub async fn dispatch(
        &self,
        ctx: SystemEventContext<'_>,
    ) -> Option<OneBotSystemDispatchSignal> {
        let route = route_onebot_system_event(ctx.payload)?;

        for handler in &self.handlers {
            let signal = match &route {
                OneBotSystemRoute::Notice(route) => handler.on_notice(&ctx, route).await,
                OneBotSystemRoute::Request(route) => handler.on_request(&ctx, route).await,
                OneBotSystemRoute::Meta(route) => handler.on_meta(&ctx, route).await,
                OneBotSystemRoute::MessageSent(route) => handler.on_message_sent(&ctx, route).await,
            };

            if signal.is_some() {
                return signal;
            }
        }

        let plugin_ctx = SystemPluginContext {
            bot_id: ctx.bot_id,
            event: ctx.payload,
            runtime: ctx.runtime,
        };

        let mut sorted_plugins: Vec<_> = self.plugins.iter().collect();
        sorted_plugins.sort_by_key(|plugin| plugin.priority());

        for plugin in sorted_plugins {
            let signal = match &route {
                OneBotSystemRoute::Notice(route) => plugin.on_notice(&plugin_ctx, &to_public_notice_route(route)).await,
                OneBotSystemRoute::Request(route) => plugin.on_request(&plugin_ctx, &to_public_request_route(route)).await,
                OneBotSystemRoute::Meta(route) => plugin.on_meta(&plugin_ctx, &to_public_meta_route(route)).await,
                OneBotSystemRoute::MessageSent(_) => None,
            };

            if let Some(signal) = signal {
                tracing::info!(plugin = plugin.metadata().id, "system plugin produced signal");
                match map_system_plugin_signal(signal) {
                    MappedSystemSignal::Dispatch(mapped) => return Some(mapped),
                    MappedSystemSignal::Block(mapped) => {
                        tracing::info!(plugin = plugin.metadata().id, "system plugin blocked chain with reply");
                        return Some(mapped);
                    }
                    MappedSystemSignal::Ignore => {
                        tracing::info!(plugin = plugin.metadata().id, "system plugin blocked chain silently");
                        return None;
                    }
                    MappedSystemSignal::Continue => {}
                }
            }
        }

        if let OneBotSystemRoute::Notice(route) = &route {
            let route_name = match route {
                NoticeRoute::GroupPoke => Some("GroupPoke"),
                NoticeRoute::PrivatePoke => Some("PrivatePoke"),
                NoticeRoute::NotifyLuckyKing => Some("NotifyLuckyKing"),
                NoticeRoute::NotifyHonor(_) => Some("NotifyHonor"),
                _ => None,
            };

            if let Some(route_name) = route_name
                && self
                    .dynamic_notice_routes
                    .iter()
                    .any(|(_, registered, _)| registered == route_name)
            {
                return Some(OneBotSystemDispatchSignal::Continue(route.clone().into()));
            }
        }

        if let OneBotSystemRoute::Request(route) = &route {
            let route_name = match route {
                RequestRoute::Friend => Some("Friend"),
                RequestRoute::GroupAdd => Some("GroupAdd"),
                RequestRoute::GroupInvite => Some("GroupInvite"),
                _ => None,
            };

            if let Some(route_name) = route_name
                && self
                    .dynamic_request_routes
                    .iter()
                    .any(|(_, registered, _)| registered == route_name)
            {
                return Some(OneBotSystemDispatchSignal::Continue(route.clone().into()));
            }
        }

        if let OneBotSystemRoute::Meta(route) = &route {
            let route_name = match route {
                MetaRoute::Heartbeat => Some("Heartbeat"),
                MetaRoute::LifecycleConnect => Some("LifecycleConnect"),
                MetaRoute::LifecycleEnable => Some("LifecycleEnable"),
                MetaRoute::LifecycleDisable => Some("LifecycleDisable"),
                _ => None,
            };

            if let Some(route_name) = route_name
                && self
                    .dynamic_meta_routes
                    .iter()
                    .any(|(_, registered, _)| registered == route_name)
            {
                return Some(OneBotSystemDispatchSignal::Continue(route.clone().into()));
            }
        }

        Some(OneBotSystemDispatchSignal::Continue(route))
    }
}

pub fn route_onebot_system_event(payload: &Value) -> Option<OneBotSystemRoute> {
    match payload.get("post_type").and_then(Value::as_str)? {
        "notice" => Some(OneBotSystemRoute::Notice(route_notice(payload))),
        "request" => Some(OneBotSystemRoute::Request(route_request(payload))),
        "meta_event" => Some(OneBotSystemRoute::Meta(route_meta(payload))),
        "message_sent" => Some(OneBotSystemRoute::MessageSent(route_message_sent(payload))),
        _ => None,
    }
}

fn route_message_sent(payload: &Value) -> MessageSentRoute {
    match payload
        .get("message_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "private" => MessageSentRoute::Private,
        "group" => MessageSentRoute::Group,
        other => MessageSentRoute::Unknown(other.to_string()),
    }
}

fn route_notice(payload: &Value) -> NoticeRoute {
    match payload
        .get("notice_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "group_upload" => NoticeRoute::GroupUpload,
        "group_admin" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "set" => NoticeRoute::GroupAdminSet,
            "unset" => NoticeRoute::GroupAdminUnset,
            other => NoticeRoute::Unknown(format!("group_admin:{other}")),
        },
        "group_decrease" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "leave" => NoticeRoute::GroupDecreaseLeave,
            "kick" => NoticeRoute::GroupDecreaseKick,
            "kick_me" => NoticeRoute::GroupDecreaseKickMe,
            other => NoticeRoute::GroupDecreaseOther(other.to_string()),
        },
        "group_increase" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "approve" => NoticeRoute::GroupIncreaseApprove,
            "invite" => NoticeRoute::GroupIncreaseInvite,
            other => NoticeRoute::GroupIncreaseOther(other.to_string()),
        },
        "group_ban" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "ban" => NoticeRoute::GroupBanBan,
            "lift_ban" => NoticeRoute::GroupBanLiftBan,
            other => NoticeRoute::GroupBanOther(other.to_string()),
        },
        "friend_add" => NoticeRoute::FriendAdd,
        "group_recall" => NoticeRoute::GroupRecall,
        "friend_recall" => NoticeRoute::FriendRecall,
        "notify" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "poke" => {
                let has_group = payload
                    .get("group_id")
                    .and_then(|v| v.as_i64())
                    .map(|id| id != 0)
                    .unwrap_or(false);
                if has_group {
                    NoticeRoute::GroupPoke
                } else {
                    NoticeRoute::PrivatePoke
                }
            }
            "lucky_king" => NoticeRoute::NotifyLuckyKing,
            "honor" => NoticeRoute::NotifyHonor(
                payload
                    .get("honor_type")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            ),
            "msg_emoji_like" => NoticeRoute::MessageEmojiLike,
            other => NoticeRoute::NotifyOther(other.to_string()),
        },
        "group_msg_emoji_like" => NoticeRoute::GroupReaction,
        "channel_created" => NoticeRoute::ChannelCreated,
        "channel_destroyed" => NoticeRoute::ChannelDestroyed,
        "channel_updated" => NoticeRoute::ChannelUpdated,
        "message_reactions_updated" => NoticeRoute::GuildMessageReactionsUpdated,
        "group_card" => NoticeRoute::GroupCard,
        "offline_file" => NoticeRoute::OfflineFile,
        "client_status" => NoticeRoute::ClientStatus,
        "essence" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "add" => NoticeRoute::EssenceAdd,
            "delete" => NoticeRoute::EssenceDelete,
            other => NoticeRoute::EssenceOther(other.to_string()),
        },
        other => NoticeRoute::Unknown(other.to_string()),
    }
}

fn route_request(payload: &Value) -> RequestRoute {
    match payload
        .get("request_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "friend" => RequestRoute::Friend,
        "group" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "add" => RequestRoute::GroupAdd,
            "invite" => RequestRoute::GroupInvite,
            other => RequestRoute::Unknown {
                request_type: "group".to_string(),
                sub_type: Some(other.to_string()),
            },
        },
        other => RequestRoute::Unknown {
            request_type: other.to_string(),
            sub_type: payload
                .get("sub_type")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        },
    }
}

fn route_meta(payload: &Value) -> MetaRoute {
    match payload
        .get("meta_event_type")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
    {
        "lifecycle" => match payload
            .get("sub_type")
            .and_then(Value::as_str)
            .unwrap_or("")
        {
            "enable" => MetaRoute::LifecycleEnable,
            "disable" => MetaRoute::LifecycleDisable,
            "connect" => MetaRoute::LifecycleConnect,
            other => MetaRoute::LifecycleOther(other.to_string()),
        },
        "heartbeat" => MetaRoute::Heartbeat,
        other => MetaRoute::Unknown(other.to_string()),
    }
}

pub(super) fn field_string(payload: &Value, field: &str) -> String {
    payload.get(field).map(value_to_string).unwrap_or_default()
}

pub(super) fn nested_field_string(
    payload: &Value,
    object_field: &str,
    nested_field: &str,
) -> String {
    payload
        .get(object_field)
        .and_then(Value::as_object)
        .and_then(|object| object.get(nested_field))
        .map(value_to_string)
        .unwrap_or_default()
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Maps a system plugin signal to a dispatch signal.
/// Returns `Ok(Some(signal))` for signals that produce a dispatch result,
/// `Ok(None)` for `Continue`, and `Err(BlockKind)` for chain-stopping signals.
enum MappedSystemSignal {
    Dispatch(OneBotSystemDispatchSignal),
    Continue,
    Block(OneBotSystemDispatchSignal),
    Ignore,
}

fn map_system_plugin_signal(signal: SystemPluginSignal) -> MappedSystemSignal {
    match signal {
        SystemPluginSignal::Continue => MappedSystemSignal::Continue,
        SystemPluginSignal::Reply(message) => MappedSystemSignal::Dispatch(OneBotSystemDispatchSignal::NoticeReply {
            message: message.plain_text(),
        }),
        SystemPluginSignal::ApproveFriend { flag, remark } => {
            MappedSystemSignal::Dispatch(OneBotSystemDispatchSignal::AutoApproveFriend { flag, remark })
        }
        SystemPluginSignal::RejectFriend { flag, reason } => {
            MappedSystemSignal::Dispatch(OneBotSystemDispatchSignal::AutoRejectFriend { flag, reason })
        }
        SystemPluginSignal::ApproveGroupInvite { flag, sub_type } => {
            MappedSystemSignal::Dispatch(OneBotSystemDispatchSignal::AutoApproveGroupInvite { flag, sub_type })
        }
        SystemPluginSignal::RejectGroupInvite {
            flag,
            sub_type,
            reason,
        } => MappedSystemSignal::Dispatch(OneBotSystemDispatchSignal::AutoRejectGroupInvite {
            flag,
            sub_type,
            reason,
        }),
        SystemPluginSignal::Block(message) => {
            MappedSystemSignal::Block(OneBotSystemDispatchSignal::NoticeReply {
                message: message.plain_text(),
            })
        }
        SystemPluginSignal::Ignore => MappedSystemSignal::Ignore,
    }
}

fn to_public_notice_route(route: &NoticeRoute) -> SystemNoticeRoute {
    match route {
        NoticeRoute::GroupUpload => SystemNoticeRoute::GroupUpload,
        NoticeRoute::GroupAdminSet => SystemNoticeRoute::GroupAdminSet,
        NoticeRoute::GroupAdminUnset => SystemNoticeRoute::GroupAdminUnset,
        NoticeRoute::GroupDecreaseLeave => SystemNoticeRoute::GroupDecreaseLeave,
        NoticeRoute::GroupDecreaseKick => SystemNoticeRoute::GroupDecreaseKick,
        NoticeRoute::GroupDecreaseKickMe => SystemNoticeRoute::GroupDecreaseKickMe,
        NoticeRoute::GroupIncreaseApprove => SystemNoticeRoute::GroupIncreaseApprove,
        NoticeRoute::GroupIncreaseInvite => SystemNoticeRoute::GroupIncreaseInvite,
        NoticeRoute::GroupBanBan => SystemNoticeRoute::GroupBanBan,
        NoticeRoute::GroupBanLiftBan => SystemNoticeRoute::GroupBanLiftBan,
        NoticeRoute::FriendAdd => SystemNoticeRoute::FriendAdd,
        NoticeRoute::GroupRecall => SystemNoticeRoute::GroupRecall,
        NoticeRoute::FriendRecall => SystemNoticeRoute::FriendRecall,
        NoticeRoute::GroupPoke => SystemNoticeRoute::GroupPoke,
        NoticeRoute::PrivatePoke => SystemNoticeRoute::PrivatePoke,
        NoticeRoute::NotifyLuckyKing => SystemNoticeRoute::NotifyLuckyKing,
        NoticeRoute::NotifyHonor(_) => SystemNoticeRoute::NotifyHonor,
        NoticeRoute::GroupCard => SystemNoticeRoute::GroupCard,
        NoticeRoute::OfflineFile => SystemNoticeRoute::OfflineFile,
        NoticeRoute::ClientStatus => SystemNoticeRoute::ClientStatus,
        NoticeRoute::EssenceAdd => SystemNoticeRoute::EssenceAdd,
        NoticeRoute::EssenceDelete => SystemNoticeRoute::EssenceDelete,
        NoticeRoute::GroupReaction => SystemNoticeRoute::GroupReaction,
        NoticeRoute::MessageEmojiLike => SystemNoticeRoute::MessageEmojiLike,
        NoticeRoute::ChannelCreated => SystemNoticeRoute::ChannelCreated,
        NoticeRoute::ChannelDestroyed => SystemNoticeRoute::ChannelDestroyed,
        NoticeRoute::ChannelUpdated => SystemNoticeRoute::ChannelUpdated,
        NoticeRoute::GuildMessageReactionsUpdated => SystemNoticeRoute::GuildMessageReactionsUpdated,
        NoticeRoute::NotifyOther(other)
        | NoticeRoute::GroupDecreaseOther(other)
        | NoticeRoute::GroupIncreaseOther(other)
        | NoticeRoute::GroupBanOther(other)
        | NoticeRoute::EssenceOther(other)
        | NoticeRoute::Unknown(other) => SystemNoticeRoute::Unknown(other.clone()),
    }
}

fn to_public_request_route(route: &RequestRoute) -> SystemRequestRoute {
    match route {
        RequestRoute::Friend => SystemRequestRoute::Friend,
        RequestRoute::GroupAdd => SystemRequestRoute::GroupAdd,
        RequestRoute::GroupInvite => SystemRequestRoute::GroupInvite,
        RequestRoute::Unknown {
            request_type,
            sub_type,
        } => SystemRequestRoute::Unknown {
            request_type: request_type.clone(),
            sub_type: sub_type.clone(),
        },
    }
}

fn to_public_meta_route(route: &MetaRoute) -> SystemMetaRoute {
    match route {
        MetaRoute::LifecycleEnable => SystemMetaRoute::LifecycleEnable,
        MetaRoute::LifecycleDisable => SystemMetaRoute::LifecycleDisable,
        MetaRoute::LifecycleConnect => SystemMetaRoute::LifecycleConnect,
        MetaRoute::LifecycleOther(other) => SystemMetaRoute::LifecycleOther(other.clone()),
        MetaRoute::Heartbeat => SystemMetaRoute::Heartbeat,
        MetaRoute::Unknown(other) => SystemMetaRoute::Unknown(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MetaRoute, NoticeRoute, OneBotSystemDispatcher, OneBotSystemDispatchSignal,
        OneBotSystemEventHandler, OneBotSystemRoute, RequestRoute, SystemEventContext,
        route_onebot_system_event,
    };
    use async_trait::async_trait;
    use qimen_error::{QimenError, Result};
    use qimen_message::Message;
    use qimen_plugin_api::{OwnedTaskFuture, RuntimeBotContext, TaskHandle};
    use qimen_protocol_core::{
        ActionStatus, CapabilitySet, NormalizedActionRequest, NormalizedActionResponse,
        NormalizedEvent, ProtocolId,
    };
    use std::sync::Arc;

    struct TestRuntimeBotContext;

    #[async_trait]
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

    #[test]
    fn routes_notice_notify_honor() {
        let payload = serde_json::json!({
            "post_type": "notice",
            "notice_type": "notify",
            "sub_type": "honor",
            "honor_type": "talkative"
        });

        assert_eq!(
            route_onebot_system_event(&payload),
            Some(OneBotSystemRoute::Notice(NoticeRoute::NotifyHonor(Some(
                "talkative".to_string()
            ))))
        );
    }

    #[test]
    fn routes_request_group_invite() {
        let payload = serde_json::json!({
            "post_type": "request",
            "request_type": "group",
            "sub_type": "invite"
        });

        assert_eq!(
            route_onebot_system_event(&payload),
            Some(OneBotSystemRoute::Request(RequestRoute::GroupInvite))
        );
    }

    #[test]
    fn routes_meta_heartbeat() {
        let payload = serde_json::json!({
            "post_type": "meta_event",
            "meta_event_type": "heartbeat",
            "interval": 5000
        });

        assert_eq!(
            route_onebot_system_event(&payload),
            Some(OneBotSystemRoute::Meta(MetaRoute::Heartbeat))
        );
    }

    struct TestHandler;

    #[async_trait::async_trait]
    impl OneBotSystemEventHandler for TestHandler {
        async fn on_notice(
            &self,
            _ctx: &SystemEventContext<'_>,
            route: &NoticeRoute,
        ) -> Option<OneBotSystemDispatchSignal> {
            Some(OneBotSystemDispatchSignal::Continue(OneBotSystemRoute::Notice(
                route.clone(),
            )))
        }
    }

    #[tokio::test]
    async fn dispatcher_can_mount_custom_handler() {
        let mut dispatcher = OneBotSystemDispatcher::with_default_handlers();
        dispatcher.register_handler(Arc::new(TestHandler));

        let payload = serde_json::json!({
            "post_type": "notice",
            "notice_type": "friend_add",
            "user_id": 1
        });

        let signal = dispatcher.dispatch(SystemEventContext {
            bot_id: "qq-main",
            payload: &payload,
            runtime: &TEST_RUNTIME,
            auto_approve_friend_requests: false,
            auto_approve_group_invites: false,
            auto_reply_poke_enabled: false,
            auto_reply_poke_message: None,
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
        }).await;
        assert!(signal.is_some());
    }
}
