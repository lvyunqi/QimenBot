# 拦截器

拦截器（Interceptor）是在消息事件到达命令插件**之前**和**之后**执行的钩子。它们适合用于实现横切关注点（cross-cutting concerns），比如日志记录、权限校验、频率限制等。

## 工作原理

```
收到消息事件
    ↓
[拦截器 A] pre_handle → true（放行）
    ↓
[拦截器 B] pre_handle → true（放行）
    ↓
[拦截器 C] pre_handle → false（拦截！）
    ↓ ← 处理到此中止，后续拦截器和插件都不会执行
    ✗

// 如果所有拦截器都放行：
[拦截器 A] pre_handle → true
[拦截器 B] pre_handle → true
    ↓
命令插件处理
    ↓
[拦截器 B] after_completion  ← 逆序执行
[拦截器 A] after_completion
```

关键点：
- `pre_handle` 按**注册顺序**执行
- `after_completion` 按**逆序**执行
- `pre_handle` 返回 `false` 会中止整个处理链
- `after_completion` 无论如何都会执行（用于清理）

## 定义拦截器

实现 `MessageEventInterceptor` trait：

```rust
use qimen_plugin_api::prelude::*;

pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    /// 事件到达插件之前调用
    /// 返回 true 放行，false 拦截
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        true
    }

    /// 所有插件处理完毕后调用（逆序）
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        // 默认实现为空，可以不重写
    }
}
```

## 注册拦截器

在 `#[module]` 宏中通过 `interceptors` 属性注册：

```rust
#[module(
    id = "my-plugin",
    interceptors = [LoggingInterceptor, CooldownInterceptor]
)]
#[commands]
impl MyPlugin {
    // ...
}
```

拦截器按列表顺序执行：先 `LoggingInterceptor`，后 `CooldownInterceptor`。

## 实战示例

### 日志拦截器

记录每条消息的基本信息，便于调试：

```rust
pub struct LoggingInterceptor;

#[async_trait]
impl MessageEventInterceptor for LoggingInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = event.sender_id().unwrap_or("unknown");
        let chat = event.chat_id().unwrap_or("unknown");
        let text = event.plain_text().unwrap_or_default();
        let scope = if event.is_group() { "群聊" } else { "私聊" };

        tracing::info!(
            "[{bot_id}] {scope} | 发送者: {sender} | 会话: {chat} | 内容: {text}"
        );

        true // 始终放行
    }

    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        let sender = event.sender_id().unwrap_or("unknown");
        tracing::debug!("[{bot_id}] 消息处理完成: sender={sender}");
    }
}
```

### 冷却时间拦截器

限制每个用户的消息频率，防止刷屏：

```rust
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct CooldownInterceptor {
    last_message: Mutex<HashMap<String, Instant>>,
}

impl CooldownInterceptor {
    pub fn new() -> Self {
        Self {
            last_message: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl MessageEventInterceptor for CooldownInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = match event.sender_id() {
            Some(id) => id.to_string(),
            None => return true,
        };

        let cooldown = Duration::from_secs(3); // 3 秒冷却
        let now = Instant::now();

        let mut map = self.last_message.lock().unwrap();
        if let Some(last) = map.get(&sender) {
            if now.duration_since(*last) < cooldown {
                tracing::debug!("用户 {sender} 触发冷却限制");
                return false; // 拦截
            }
        }

        map.insert(sender, now);
        true // 放行
    }
}
```

### 黑名单拦截器

```rust
pub struct BlacklistInterceptor {
    blocked_users: Vec<i64>,
}

impl BlacklistInterceptor {
    pub fn new(blocked_users: Vec<i64>) -> Self {
        Self { blocked_users }
    }
}

#[async_trait]
impl MessageEventInterceptor for BlacklistInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        if let Some(sender_id) = event.sender_id_i64() {
            if self.blocked_users.contains(&sender_id) {
                tracing::info!("黑名单用户 {sender_id} 被拦截");
                return false;
            }
        }
        true
    }
}
```

### 关键词过滤拦截器

```rust
pub struct KeywordFilterInterceptor {
    forbidden_words: Vec<String>,
}

#[async_trait]
impl MessageEventInterceptor for KeywordFilterInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        let text = event.plain_text().unwrap_or_default().to_lowercase();
        for word in &self.forbidden_words {
            if text.contains(&word.to_lowercase()) {
                tracing::info!("消息包含违禁词: {word}");
                return false;
            }
        }
        true
    }
}
```

## NormalizedEvent 便捷方法

在拦截器中，你可以通过 `NormalizedEvent` 访问事件信息：

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `sender_id()` | `Option<&str>` | 发送者 ID |
| `sender_id_i64()` | `Option<i64>` | 发送者 ID（数字） |
| `sender_nickname()` | `Option<&str>` | 发送者昵称 |
| `chat_id()` | `Option<&str>` | 聊天 ID |
| `group_id()` | `Option<&str>` | 群号 |
| `is_group()` | `bool` | 是否群聊 |
| `is_private()` | `bool` | 是否私聊 |
| `plain_text()` | `Option<String>` | 纯文本内容 |
| `message_id()` | `Option<i64>` | 消息 ID |
| `is_at_self()` | `bool` | 是否 @了 Bot |
| `sender_role()` | `Option<&str>` | 群角色 |

## 注意事项

::: warning 拦截器是全局的
拦截器对**所有消息事件**生效，不仅仅是命令消息。即使用户发的不是命令，拦截器的 `pre_handle` 和 `after_completion` 也会被调用。

在拦截器中要注意性能——每条消息都会经过所有拦截器。避免在拦截器中做耗时操作（如网络请求、数据库查询）。
:::

::: tip 状态管理
拦截器实例在整个运行期间保持存在，你可以使用 `Mutex<HashMap<...>>` 等方式在拦截器中维护状态。但要注意线程安全——拦截器可能被并发调用。
:::
