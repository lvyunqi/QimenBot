//! Ordered chain of [`MessageEventInterceptor`]s for pre/post event processing.

use crate::plugin_access::plugin_allowed;
use qimen_plugin_api::MessageEventInterceptor;
use qimen_protocol_core::NormalizedEvent;
use std::sync::Arc;

/// An ordered chain of interceptors that run before and after message event dispatch.
#[derive(Clone)]
pub struct InterceptorChain {
    interceptors: Vec<InterceptorEntry>,
}

#[derive(Clone)]
struct InterceptorEntry {
    plugin_id: Option<String>,
    interceptor: Arc<dyn MessageEventInterceptor>,
}

impl InterceptorChain {
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    pub fn add(&mut self, interceptor: Arc<dyn MessageEventInterceptor>) {
        self.interceptors.push(InterceptorEntry {
            plugin_id: None,
            interceptor,
        });
    }

    pub fn add_for_plugin(
        &mut self,
        plugin_id: impl Into<String>,
        interceptor: Arc<dyn MessageEventInterceptor>,
    ) {
        self.interceptors.push(InterceptorEntry {
            plugin_id: Some(plugin_id.into()),
            interceptor,
        });
    }

    pub async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        self.pre_handle_for_plugins(bot_id, event, None).await
    }

    pub async fn pre_handle_for_plugins(
        &self,
        bot_id: &str,
        event: &NormalizedEvent,
        enabled_plugins: Option<&[String]>,
    ) -> bool {
        for entry in &self.interceptors {
            if !interceptor_is_enabled(entry, enabled_plugins) {
                continue;
            }
            if !entry.interceptor.pre_handle(bot_id, event).await {
                return false;
            }
        }
        true
    }

    pub async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        self.after_completion_for_plugins(bot_id, event, None).await;
    }

    pub async fn after_completion_for_plugins(
        &self,
        bot_id: &str,
        event: &NormalizedEvent,
        enabled_plugins: Option<&[String]>,
    ) {
        for entry in self.interceptors.iter().rev() {
            if interceptor_is_enabled(entry, enabled_plugins) {
                entry.interceptor.after_completion(bot_id, event).await;
            }
        }
    }
}

fn interceptor_is_enabled(entry: &InterceptorEntry, enabled_plugins: Option<&[String]>) -> bool {
    match entry.plugin_id.as_deref() {
        Some(plugin_id) => plugin_allowed(enabled_plugins, plugin_id),
        None => enabled_plugins.is_none(),
    }
}

impl Default for InterceptorChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use qimen_message::Message;
    use qimen_protocol_core::{EventKind, ProtocolId, TransportMode};
    use serde_json::Map;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct BlockingInterceptor;

    #[async_trait]
    impl MessageEventInterceptor for BlockingInterceptor {
        async fn pre_handle(&self, _bot_id: &str, _event: &NormalizedEvent) -> bool {
            false
        }
    }

    struct PassthroughInterceptor {
        completed: AtomicBool,
    }

    #[async_trait]
    impl MessageEventInterceptor for PassthroughInterceptor {
        async fn pre_handle(&self, _bot_id: &str, _event: &NormalizedEvent) -> bool {
            true
        }
        async fn after_completion(&self, _bot_id: &str, _event: &NormalizedEvent) {
            self.completed.store(true, Ordering::SeqCst);
        }
    }

    fn sample_event() -> NormalizedEvent {
        NormalizedEvent {
            protocol: ProtocolId::OneBot11,
            bot_instance: "test".to_string(),
            transport_mode: TransportMode::WsForward,
            time: Some(1),
            kind: EventKind::Message,
            message: Some(Message::text("hello")),
            actor: None,
            chat: None,
            raw_json: serde_json::json!({}),
            raw_bytes: None,
            extensions: Map::new(),
        }
    }

    #[tokio::test]
    async fn empty_chain_passes() {
        let chain = InterceptorChain::new();
        assert!(chain.pre_handle("test", &sample_event()).await);
    }

    #[tokio::test]
    async fn blocking_interceptor_stops_chain() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(BlockingInterceptor));
        assert!(!chain.pre_handle("test", &sample_event()).await);
    }

    #[tokio::test]
    async fn after_completion_runs_in_reverse() {
        let p1 = Arc::new(PassthroughInterceptor {
            completed: AtomicBool::new(false),
        });
        let p2 = Arc::new(PassthroughInterceptor {
            completed: AtomicBool::new(false),
        });
        let mut chain = InterceptorChain::new();
        chain.add(p1.clone());
        chain.add(p2.clone());

        let event = sample_event();
        assert!(chain.pre_handle("test", &event).await);
        chain.after_completion("test", &event).await;
        assert!(p1.completed.load(Ordering::SeqCst));
        assert!(p2.completed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn bot_allowlist_skips_unselected_plugin_interceptors() {
        let selected = Arc::new(PassthroughInterceptor {
            completed: AtomicBool::new(false),
        });
        let unselected = Arc::new(PassthroughInterceptor {
            completed: AtomicBool::new(false),
        });
        let mut chain = InterceptorChain::new();
        chain.add_for_plugin("selected-plugin", selected.clone());
        chain.add_for_plugin("other-plugin", unselected.clone());
        chain.add_for_plugin("blocking-plugin", Arc::new(BlockingInterceptor));

        let event = sample_event();
        let enabled = ["selected-plugin".to_string()];
        assert!(
            chain
                .pre_handle_for_plugins("test", &event, Some(&enabled))
                .await
        );
        chain
            .after_completion_for_plugins("test", &event, Some(&enabled))
            .await;

        assert!(selected.completed.load(Ordering::SeqCst));
        assert!(!unselected.completed.load(Ordering::SeqCst));
    }
}
