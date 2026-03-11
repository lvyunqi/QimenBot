// ── 拦截器示例 / Interceptor Examples ──
//
// 拦截器在每条消息事件处理前后运行，可用于日志、频率限制、黑名单等。
// Interceptors run before/after every message event — useful for logging,
// rate-limiting, blacklists, and more.

use qimen_plugin_api::prelude::*;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::time::Instant;

// ── LoggingInterceptor ──────────────────────────────────────────────────
// 展示 NormalizedEvent 的各种便捷方法
// Demonstrates NormalizedEvent convenience methods

pub struct LoggingInterceptor;

#[async_trait]
impl MessageEventInterceptor for LoggingInterceptor {
    /// 事件处理前：记录发送者、聊天上下文、消息文本
    /// Before handling: log sender, chat context, message text
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = event.sender_id().unwrap_or("unknown");
        let chat = event.chat_id().unwrap_or("unknown");
        let text = event.plain_text();
        let scope = if event.is_group() {
            "group"
        } else if event.is_private() {
            "private"
        } else {
            "other"
        };

        tracing::info!(
            bot_id,
            sender,
            chat,
            scope,
            text = text.as_str(),
            "incoming message event / 收到消息事件"
        );

        // 返回 true 放行事件 / return true to let the event through
        true
    }

    /// 事件处理完成后记录日志
    /// Log after all plugins have finished processing
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        let sender = event.sender_id().unwrap_or("unknown");
        tracing::debug!(
            bot_id,
            sender,
            "message event processing completed / 消息事件处理完成"
        );
    }
}

// ── CooldownInterceptor ─────────────────────────────────────────────────
// 展示拦截器中的状态管理：每用户冷却时间
// Demonstrates stateful interceptor: per-user cooldown

pub struct CooldownInterceptor {
    /// 每用户最后发送时间 / last message time per user
    last_message: Mutex<HashMap<String, Instant>>,
}

impl CooldownInterceptor {
    /// 冷却时间（秒）/ cooldown duration in seconds
    const COOLDOWN_SECS: u64 = 3;

    pub fn new() -> Self {
        Self {
            last_message: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MessageEventInterceptor for CooldownInterceptor {
    /// 检查用户是否在冷却期内，若是则拦截
    /// Check if user is within cooldown period; block if so
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = match event.sender_id() {
            Some(id) => id.to_string(),
            None => return true, // 无法识别发送者，放行 / unknown sender, allow
        };

        let now = Instant::now();
        let mut map = self.last_message.lock().unwrap();

        if let Some(last) = map.get(&sender) {
            if now.duration_since(*last).as_secs() < Self::COOLDOWN_SECS {
                tracing::debug!(sender = sender.as_str(), "cooldown active, blocking / 冷却中，拦截");
                return false; // 拦截过于频繁的消息 / block too-frequent messages
            }
        }

        map.insert(sender, now);
        true
    }
}
