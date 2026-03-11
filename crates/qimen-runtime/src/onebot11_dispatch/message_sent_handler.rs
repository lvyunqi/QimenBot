use async_trait::async_trait;

use super::{
    MessageSentRoute, OneBotSystemDispatchSignal, OneBotSystemEventHandler, OneBotSystemRoute,
    SystemEventContext, field_string,
};

pub struct LoggingMessageSentHandler;

#[async_trait]
impl OneBotSystemEventHandler for LoggingMessageSentHandler {
    async fn on_message_sent(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &MessageSentRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        handle_message_sent(ctx.bot_id, route, ctx.payload);
        Some(OneBotSystemDispatchSignal::Continue(
            OneBotSystemRoute::MessageSent(route.clone()),
        ))
    }
}

fn handle_message_sent(
    bot_id: &str,
    route: &MessageSentRoute,
    payload: &serde_json::Value,
) {
    match route {
        MessageSentRoute::Private => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                message_id = %field_string(payload, "message_id"),
                "handled message_sent (private)"
            );
        }
        MessageSentRoute::Group => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                message_id = %field_string(payload, "message_id"),
                "handled message_sent (group)"
            );
        }
        MessageSentRoute::Unknown(message_type) => {
            tracing::info!(
                bot_id = %bot_id,
                message_type = %message_type,
                message_id = %field_string(payload, "message_id"),
                "handled message_sent (unknown type)"
            );
        }
    }
}
