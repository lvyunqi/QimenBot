use async_trait::async_trait;
use serde_json::Value;

use super::{
    MetaRoute, OneBotSystemDispatchSignal, OneBotSystemEventHandler, SystemEventContext,
};

pub struct LoggingMetaHandler;

#[async_trait]
impl OneBotSystemEventHandler for LoggingMetaHandler {
    async fn on_meta(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &MetaRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        Some(handle_meta(ctx.bot_id, route, ctx.payload))
    }
}

pub fn handle_meta(bot_id: &str, route: &MetaRoute, payload: &Value) -> OneBotSystemDispatchSignal {
    match route {
        MetaRoute::LifecycleEnable => {
            tracing::info!(bot_id = %bot_id, "bot enabled (lifecycle.enable)");
        }
        MetaRoute::LifecycleDisable => {
            tracing::warn!(bot_id = %bot_id, "bot disabled (lifecycle.disable)");
        }
        MetaRoute::LifecycleConnect => {
            tracing::info!(bot_id = %bot_id, "bot connected (lifecycle.connect)");
        }
        MetaRoute::LifecycleOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled meta.lifecycle.other");
        }
        MetaRoute::Heartbeat => {
            let interval = payload.get("interval").and_then(Value::as_u64).unwrap_or(0);
            tracing::debug!(bot_id = %bot_id, interval_ms = interval, "handled meta.heartbeat");
            return OneBotSystemDispatchSignal::Heartbeat(interval);
        }
        MetaRoute::Unknown(meta_event_type) => {
            tracing::info!(bot_id = %bot_id, meta_event_type = %meta_event_type, "handled meta.unknown");
        }
    }

    OneBotSystemDispatchSignal::Continue(super::OneBotSystemRoute::Meta(route.clone()))
}
