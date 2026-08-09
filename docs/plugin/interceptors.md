# 拦截器

拦截器用于处理进入命令分发器之前和正常完成之后的消息。日志、冷却、黑名单、维护模式和轻量统计适合放在这里；通知、申请和生命周期事件不属于消息拦截器，应使用事件处理器。

静态插件实现 `MessageEventInterceptor`，动态插件使用 `#[pre_handle]` 和 `#[after_completion]`。两者接入同一条运行时链，OneBot 11 与官方 QQ Bot 共用这套流程。

## 执行位置

一条消息实际经过的顺序如下：

```text
协议解码为 NormalizedEvent
  -> 消息去重
  -> 群事件过滤
  -> 空文本检查
  -> Bot 级限流
  -> pre_handle（正序）
  -> 权限解析与命令分发
  -> 发送被动回复
  -> after_completion（逆序）
```

前五步发生在拦截器之前。重复消息、被群过滤器排除的消息、纯文本为空的消息和被 Bot 级限流丢弃的消息，不会调用任何拦截器。

当前运行时以 `message.plain_text().trim()` 判断空消息，因此只有图片、语音、视频或文件而没有文本的消息也不会进入拦截器。这是现有消息流水线边界，不能用媒体类型判断在拦截器内绕过。

```text
[静态 A] pre_handle -> true
[静态 B] pre_handle -> true
[动态 C] pre_handle -> true
  -> 命令分发正常完成
[动态 C] after_completion
[静态 B] after_completion
[静态 A] after_completion
```

如果任意 `pre_handle` 返回阻断结果，运行时立即结束这条消息：

```text
[静态 A] pre_handle -> true
[静态 B] pre_handle -> false
  -> 不执行后续 pre_handle
  -> 不执行命令插件
  -> 不执行任何 after_completion
```

::: warning `after_completion` 不是 finally
`after_completion` 只在消息正常走完命令分发和回复阶段后执行。`pre_handle` 阻断、回复发送失败或流水线提前返回时都不会调用它。必须释放的资源应使用局部值和 RAII 管理，不要依赖后置钩子兜底。
:::

没有匹配到命令不算错误。只要消息正常通过前置链，`after_completion` 仍会执行。

## 协议支持

拦截器处理的是统一的 `EventKind::Message`，不是 OneBot 专用接口。

| 平台 | 进入拦截器的消息 |
|---|---|
| OneBot 11 | 私聊消息、群消息，以及适配器解码为普通消息的频道扩展 |
| 官方 QQ 群 | `GROUP_AT_MESSAGE_CREATE`、`GROUP_MESSAGE_CREATE` |
| 官方 QQ C2C | `C2C_MESSAGE_CREATE` |
| 官方 QQ 频道 | `AT_MESSAGE_CREATE`、`MESSAGE_CREATE` |
| 官方 QQ DMS | `DIRECT_MESSAGE_CREATE` |

官方 QQ 开放平台必须先把对应事件投递给 Gateway。没有订阅 Intent、没有全量群消息权限或平台只投递 @ 事件时，宿主收不到的消息自然不会进入拦截器。

以下事件不经过消息拦截器：

- `Notice`、`Request`、`Meta`；
- Bot 自身发送事件 `MessageSent`；
- Webhook 请求；
- 动态插件后台线程主动发送的消息。

静态插件使用 `#[notice]`、`#[request]`、`#[meta]` 处理系统事件；动态插件使用 `#[route]`。

## 静态插件

### 实现与注册

```rust
use qimen_plugin_api::prelude::*;

pub struct AuditInterceptor;

#[async_trait]
impl MessageEventInterceptor for AuditInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = event.sender_id().unwrap_or("unknown");
        let chat = event.chat_id().unwrap_or("unknown");

        tracing::debug!(
            bot_id,
            protocol = ?event.protocol,
            sender,
            chat,
            text = %event.plain_text(),
            "收到消息"
        );
        true
    }

    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        tracing::debug!(
            bot_id,
            message_id = ?event.message_id_str(),
            "消息处理完成"
        );
    }
}

#[module(
    id = "my-plugin",
    interceptors = [AuditInterceptor]
)]
#[commands]
impl MyPlugin {
    // 命令方法
}
```

同一模块内按 `interceptors` 数组顺序执行 `pre_handle`。宿主先收集静态拦截器，再把当前已加载的动态拦截器追加到链尾；`after_completion` 对完整链逆序执行。

### 有状态拦截器

`#[module(interceptors = [Type])]` 会直接构造 `Type`，因此适合单元结构体。需要冷却表等状态时，可以把共享状态放在静态容器中：

```rust
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

static LAST_MESSAGE: LazyLock<Mutex<HashMap<String, Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct CooldownInterceptor;

#[async_trait]
impl MessageEventInterceptor for CooldownInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        let Some(sender) = event.sender_id() else {
            return true;
        };
        // 这是进程内短期冷却键，使用部署实例名即可；持久化数据应改用 account_id。
        let key = format!(
            "{}:{}:{}",
            event.bot_instance,
            event.chat_id().unwrap_or("private"),
            sender
        );
        let now = Instant::now();
        let mut entries = LAST_MESSAGE.lock().unwrap();

        if entries
            .get(&key)
            .is_some_and(|last| now.duration_since(*last) < Duration::from_secs(3))
        {
            return false;
        }

        entries.insert(key, now);
        true
    }
}
```

需要由构造函数注入配置或服务时，手工实现 `Module::interceptors()` 并返回 `Arc<dyn MessageEventInterceptor>`。不要给带字段的结构体直接套用上面的宏注册写法。

`std::sync::Mutex` 的锁守卫不能跨 `.await` 持有。先在同步代码块中读写状态并释放锁，再执行异步操作。

### 可读取的信息

| 方法或字段 | 用途 |
|---|---|
| `event.protocol` | 判断 `OneBot11`、`QqOfficial` 等协议 |
| `event.bot_instance` | 当前 `[[bots]].id` 部署实例名 |
| `sender_id()` | 发送者字符串 ID；官方 QQ 下通常是 OpenID |
| `chat_id()` | 当前群、C2C、频道或 DMS 会话 ID |
| `group_id()` | 仅群聊返回群 ID |
| `sender_nickname()` | 协议提供的昵称或用户名 |
| `plain_text()` | 规范化后的纯文本 |
| `message_id_str()` | 跨协议字符串消息 ID |
| `is_group()` / `is_private()` | 判断群聊或私聊类场景 |
| `is_at_self()` | 是否提及当前 Bot |
| `event.message` | 完整通用消息段 |
| `event.raw_json` | 规范化原始事件和宿主 `qimen_context` |

跨协议插件不要用 `sender_id_i64()`、`group_id_i64()` 或数字消息 ID 作为通用逻辑。官方 QQ 的 OpenID、群 ID、频道 ID 和消息 ID 都应按字符串保存。

静态拦截器没有 `RuntimeBotContext`，返回值也只有放行或阻断，不能直接构造一条阻断回复。需要向用户返回说明时，优先在命令插件中返回 `CommandPluginSignal::Block(message)`；不要把需要回复的业务命令改成静默拦截器。

## 动态插件

### 最小实现

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
    fn record_completion(req: &InterceptorRequest) {
        eprintln!(
            "message completed: bot={}, message={}",
            req.bot_id.as_str(),
            req.message_id.as_str()
        );
    }
}
```

每个动态插件最多声明一个 `#[pre_handle]` 和一个 `#[after_completion]`。两个回调都是同步 FFI，受宿主 `dynamic_plugin_timeout_secs` 限制，不能使用 `async fn`，也不应执行长时间网络请求。

### InterceptorRequest

| 字段 | 说明 |
|---|---|
| `bot_id` | 当前 `[[bots]].id` 部署实例名 |
| `sender_id` | 协议提供的字符串发送者 ID |
| `group_id` | 仅群聊场景的群 ID；C2C、频道和 DMS 为空 |
| `message_text` | 规范化后的纯文本 |
| `raw_event_json` | 完整规范化事件 JSON，包含宿主写入的 `qimen_context` |
| `sender_nickname` | 昵称或用户名，缺失时为空 |
| `message_id` | 字符串消息 ID，缺失时为空 |
| `timestamp` | Unix 秒时间戳，缺失时为 `0` |

动态 ABI 暂时没有独立的 `chat_id` 字段。频道、DMS 和其他平台专属目标从 `raw_event_json` 读取；需要稳定 Bot 身份时读取宿主保留字段：

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

`bot_instance` 是可调整的部署别名。持久化跨重启状态和后台主动发送应优先使用可选的 `account_id`；缺失时不要用 `unknown` 把多个 Bot 的数据混在一起。宿主不会把 Secret、access token 等凭据放进该对象。

### 阻断和发送

`InterceptorResponse::block()` 只表达阻断，不携带回复内容。动态拦截器需要发送提示时，可以在回调中调用 `BotApi` 或 `SendBuilder`：

- `SendBuilder::...send()` 写入当前 FFI 回调的兼容队列，回调结束后由宿主处理；即使 `pre_handle` 随后阻断，队列也会被提取。
- `SendBuilder::...bot(...).try_send()` 或 `.bot_account(...).try_send()` 立即提交实时宿主队列，并返回 `SendEnqueueStatus`。
- 必须根据协议和会话类型选择正确目标。不要把官方 QQ OpenID 当数字 QQ 号，也不要拿 `group_id` 代替频道或 DMS 目标。

动态 `pre_handle` 加载失败、panic 或超时时，宿主采用 fail-open 策略，记录错误后继续处理消息，避免一个坏插件阻断全部机器人。`after_completion` 失败只记录日志。连续超时仍会计入动态插件熔断状态。

## 常见用途

### 字符串黑名单

跨协议黑名单保存字符串 ID，并把 Bot 身份纳入键：

```rust
let Some(account_id) = event
    .raw_json
    .pointer("/qimen_context/account_id")
    .and_then(serde_json::Value::as_str)
else {
    // 持久化规则无法可靠区分 Bot 时，要求管理员补 account_id。
    return true;
};

let key = format!(
    "{:?}:{}:{}",
    event.protocol,
    account_id,
    event.sender_id().unwrap_or("unknown")
);

if blocked_users.contains(&key) {
    return false;
}
```

不要把官方 QQ OpenID 解析为 `i64`。不同机器人应用下的 OpenID 也不能假定互通。

### 轻量审计

只记录定位问题所需的字段。消息正文、OpenID、媒体 URL 和 `raw_event_json` 都可能包含隐私信息；Secret 和完整 Base64 媒体不得写入日志。

### 耗时检查

数据库或外部风控服务可能拖慢每一条消息。优先使用本地缓存、短超时和有界并发；需要稳定异步处理时，把判定前移到插件自身的缓存刷新任务，不要在每次拦截时发起无上限网络请求。

## 排错

拦截器没有日志时，按以下顺序检查：

1. 插件是否已加载并启用，静态模块 ID 是否在 `plugin_modules` 中。
2. 官方 QQ 是否订阅了对应 Intent，机器人是否具备群全量消息或频道消息权限。
3. 消息是否在拦截器之前被去重、群过滤、空文本检查或 Bot 级限流丢弃。
4. 动态插件是否导出了拦截器描述符，Web 插件页重新扫描后是否重建成功。
5. 日志中是否存在动态回调加载失败、panic、超时或熔断信息。

开发期可以临时启用原始消息日志：

```toml
[observability]
level = "info,qimen_raw_message=debug"
```

它会记录协议侧收发 JSON，可能包含用户消息和平台 ID，只应在受控环境短期开启。

## 测试清单

- 放行时命令正常执行，`after_completion` 执行一次。
- 阻断时后续拦截器、命令插件和全部 `after_completion` 都不执行。
- 没有命令匹配时，后置钩子仍按逆序执行。
- OneBot 数字 ID 与官方 QQ 字符串 ID 都不会导致解析失败或 panic。
- 官方 QQ 群、C2C、频道和 DMS 的目标字段按各自场景读取。
- 动态插件重新扫描后不会重复注册拦截器。
- 超时、panic 和发送队列失败不会让整个消息循环退出。
