# 消息拦截器开发

## 目录

- [先确认适用范围](#先确认适用范围)
- [执行顺序](#执行顺序)
- [静态插件拦截器](#静态插件拦截器)
- [动态插件拦截器](#动态插件拦截器)
- [官方 QQ Bot](#官方-qq-bot)
- [发送、状态与性能](#发送状态与性能)
- [测试与排错](#测试与排错)

## 先确认适用范围

只把横切的消息逻辑做成拦截器，例如审计、冷却、黑名单、维护开关和本地缓存判定。

拦截器只处理统一的 `EventKind::Message`：

- OneBot 11 私聊、群聊及适配器归一化为普通消息的扩展场景；
- 官方 QQ `GROUP_AT_MESSAGE_CREATE`、`GROUP_MESSAGE_CREATE`；
- 官方 QQ `C2C_MESSAGE_CREATE`；
- 官方 QQ `AT_MESSAGE_CREATE`、`MESSAGE_CREATE`；
- 官方 QQ `DIRECT_MESSAGE_CREATE`。

不要用消息拦截器处理 `Notice`、`Request`、`Meta`、`MessageSent` 或 Webhook。静态插件改用 `#[notice]`、`#[request]`、`#[meta]`；动态插件改用 `#[route]` 或 `#[webhook]`。

需要根据命令返回说明并阻断其他插件时，优先返回 `CommandPluginSignal::Block(message)`。静态 `MessageEventInterceptor` 没有运行时发送上下文，`false` 只会静默阻断。

## 执行顺序

按真实运行时顺序推理：

```text
协议解码
  -> 去重
  -> 群事件过滤
  -> 空文本检查
  -> Bot 级限流
  -> pre_handle 正序
  -> 权限与命令分发
  -> 被动回复
  -> after_completion 逆序
```

遵守以下语义：

1. 静态拦截器先加入链，动态拦截器在扫描后追加。
2. 同一静态模块按 `interceptors = [...]` 数组顺序执行。
3. 任意 `pre_handle` 阻断后，立即结束消息；不执行后续前置钩子、命令和任何后置钩子。
4. 没有命令匹配但流水线正常完成时，执行 `after_completion`。
5. 回复发送失败或其他提前错误返回时，不执行 `after_completion`。
6. 把 `after_completion` 当作正常完成通知，不要当作 `finally` 或资源释放保证。

重复消息、群过滤、空文本和 Bot 级限流发生在拦截器之前。排查“完全没有进入拦截器”时先检查这些步骤。

当前空文本检查使用规范化 `plain_text()`。纯图片、语音、视频或文件消息没有文本时不会进入拦截器；不要承诺用拦截器处理所有媒体消息。需要这类能力时先确认宿主版本是否已改变流水线，再决定是否修改框架。

## 静态插件拦截器

完整读取 `static-plugins.md` 后再实现。最小形式：

```rust
use qimen_plugin_api::prelude::*;

pub struct AuditInterceptor;

#[async_trait]
impl MessageEventInterceptor for AuditInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        tracing::debug!(
            bot_id,
            protocol = ?event.protocol,
            sender = ?event.sender_id(),
            chat = ?event.chat_id(),
            message_id = ?event.message_id_str(),
            "message received"
        );
        true
    }

    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        tracing::debug!(
            bot_id,
            message_id = ?event.message_id_str(),
            "message completed"
        );
    }
}

#[module(id = "my-plugin", interceptors = [AuditInterceptor])]
#[commands]
impl MyPlugin {}
```

`#[module(interceptors = [Type])]` 展开为直接构造 `Type`，因此只对单元结构体直接适用。需要构造参数时选择以下方式之一：

- 使用 `LazyLock<Mutex<_>>`、`OnceLock` 或其他进程级共享状态，并保持拦截器为单元结构体；
- 手工实现 `Module::interceptors()`，显式构造 `Arc<dyn MessageEventInterceptor>`；
- 如果状态只属于一条命令，把它放入命令插件，不要为了共享状态强行使用拦截器。

不要在持有 `std::sync::Mutex` 或 `RwLock` 守卫时 `.await`。先完成同步读写并释放锁。

优先使用以下跨协议字段：

- `event.protocol`、`event.bot_instance`；
- `sender_id()`、`chat_id()`、`group_id()`；
- `message_id_str()`、`plain_text()`、`event.message`；
- `is_group()`、`is_private()`、`is_at_self()`；
- `event.raw_json.qimen_context`。

只在明确限定 OneBot 11 时使用 `sender_id_i64()`、`group_id_i64()`、数字消息 ID 和 `OneBotActionClient`。

## 动态插件拦截器

完整读取 `dynamic-plugins.md` 后再实现。新插件明确声明 API 版本：

```rust
use abi_stable_host_api::{InterceptorRequest, InterceptorResponse};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0", api = "0.6")]
mod plugin {
    use super::*;

    #[pre_handle]
    fn filter(req: &InterceptorRequest) -> InterceptorResponse {
        if req.message_text.as_str().contains("blocked-word") {
            InterceptorResponse::block()
        } else {
            InterceptorResponse::allow()
        }
    }

    #[after_completion]
    fn completed(req: &InterceptorRequest) {
        eprintln!("completed message {}", req.message_id.as_str());
    }
}
```

每个动态插件最多各声明一个 `#[pre_handle]` 和 `#[after_completion]`。保持同步 FFI 回调短小，不使用 `async fn`，不在回调内执行无超时网络请求。

`InterceptorRequest` 字段：

| 字段 | 约束 |
|---|---|
| `bot_id` | `[[bots]].id` 部署别名，不是稳定账号 |
| `sender_id` | 字符串发送者 ID；官方 QQ 为 OpenID |
| `group_id` | 仅群聊的群 ID；C2C、频道和 DMS 为空 |
| `message_text` | 规范化纯文本 |
| `raw_event_json` | 完整事件 JSON，包含可信 `qimen_context` |
| `sender_nickname` | 缺失时为空字符串 |
| `message_id` | 字符串消息 ID，缺失时为空 |
| `timestamp` | Unix 秒，缺失时为 `0` |

ABI 当前没有独立 `chat_id`。从 `raw_event_json` 读取频道、DMS 和平台专属目标，不要把空 `group_id` 当作解码失败。

解析并使用宿主账号上下文：

```json
{
  "qimen_context": {
    "version": 1,
    "protocol": "qq-official",
    "bot_instance": "qq-main",
    "account_id": "102012345"
  }
}
```

将 `account_id` 用作持久化和后台主动发送的稳定 Bot 选择器。它是可选字段；缺失且无法从协议原始事件可靠推导时，拒绝跨 Bot 共享有状态数据。不要使用 `bot_instance` 或 `unknown` 代替账号主键。宿主不会注入 Secret、Token 或其他 Bot 凭据。

动态 `pre_handle` 加载失败、panic 或超时时采用 fail-open：宿主记录错误并继续处理消息。不要把动态拦截器当成唯一安全边界；真正的鉴权仍应在命令、Webhook 或外部服务端复核。

## 官方 QQ Bot

先确认 Gateway 实际投递事件：

- 群内 @ 需要 `GROUP_AT_MESSAGE_CREATE`；
- 群普通消息需要 `GROUP_MESSAGE_CREATE` 及对应全量消息权限；
- C2C、频道和 DMS 需要各自 Intent；
- 没有平台权限时，修改拦截器代码不能让消息凭空进入宿主。

官方 QQ 适配器先把自身 @ 标签转换为通用消息段并生成规范化纯文本。使用 `plain_text()` 或 `message_text` 做普通匹配；需要核对平台原文、mentions、channel_id、guild_id 或 group_openid 时解析 `raw_event_json`。

始终把 OpenID、group_openid、频道 ID、guild ID 和官方消息 ID 当作字符串。不同 Bot 应用下的 OpenID 不保证相同。

## 发送、状态与性能

静态拦截器没有 `RuntimeBotContext`，不能直接发送阻断提示。动态拦截器可以使用 Host API：

- `SendBuilder::...send()` 把消息加入当前回调兼容队列；运行时会在前置或后置链调用后提取，前置拦截不会丢弃已经排队的发送。
- `SendBuilder::...bot(...).try_send()` 或 `.bot_account(...).try_send()` 立即提交实时队列并返回 `SendEnqueueStatus`。
- 根据 `qimen_context.protocol` 和原始会话字段选择 private、group、channel 或 channel_private，不猜测目标类型。

静态拦截器没有宿主超时保护，耗时会直接拖慢当前 Bot 消息处理。动态拦截器受 `dynamic_plugin_timeout_secs` 限制，但超时仍消耗线程和熔断预算。两者都应优先使用内存缓存、短临界区、有界任务和短超时。

日志不要输出 Secret、Token、完整 Base64、Webhook 签名密钥或全部用户配置。`raw_event_json`、消息正文、OpenID 和媒体 URL 也属于敏感运行数据，只在受控调试环境记录。

## 测试与排错

至少验证：

1. 放行后命令执行，后置钩子执行一次。
2. 阻断后后续前置钩子、命令和全部后置钩子都不执行。
3. 没有命令匹配时后置钩子仍执行。
4. OneBot 数字 ID 和官方 QQ 字符串 ID 都不会 panic。
5. 官方 QQ 群、C2C、频道和 DMS 分别读取正确目标。
6. 动态回调超时、panic 或加载失败时消息保持 fail-open，错误可在动态诊断中查看。
7. 动态重新扫描后只存在一套拦截器，不重复执行。

完全没有进入拦截器时依次检查：插件启用状态、静态 `plugin_modules`、动态描述符、官方 Intent、去重、群过滤、空文本和 Bot 级限流。

开发期可临时配置：

```toml
[observability]
level = "info,qimen_raw_message=debug"
```

仅在受控环境短期开启原始消息日志，并在定位完成后恢复普通级别。
