# 插件 API 参考

本页列出 `qimen-plugin-api` crate 提供的所有公共类型、trait 和方法。

## 导入

```rust
use qimen_plugin_api::prelude::*;
```

prelude 模块导出了开发插件所需的全部类型。

## Trait

### Module

模块是插件的最顶层抽象，每个 `#[module]` 宏自动实现此 trait。

```rust
#[async_trait]
pub trait Module: Send + Sync + 'static {
    /// 模块唯一标识
    fn id(&self) -> &'static str;

    /// 模块加载时调用
    async fn on_load(&self) -> Result<()>;

    /// 模块卸载时调用（默认空实现）
    async fn on_unload(&self) -> Result<()> { Ok(()) }

    /// 是否支持热重载（默认 false）
    fn supports_hot_reload(&self) -> bool { false }

    /// 返回此模块的命令插件列表
    fn command_plugins(&self) -> Vec<Arc<dyn CommandPlugin>> { vec![] }

    /// 返回此模块的系统事件插件列表
    fn system_plugins(&self) -> Vec<Arc<dyn SystemPlugin>> { vec![] }

    /// 返回此模块的拦截器列表
    fn interceptors(&self) -> Vec<Arc<dyn MessageEventInterceptor>> { vec![] }

    /// 注册插件到注册器
    fn register_plugins(&self, registrar: &mut dyn PluginRegistrar);
}
```

### CommandPlugin

命令插件处理用户发送的命令消息。

```rust
#[async_trait]
pub trait CommandPlugin: Send + Sync + 'static {
    /// 插件元数据
    fn metadata(&self) -> PluginMetadata;

    /// 支持的命令列表
    fn commands(&self) -> Vec<CommandDefinition>;

    /// 插件优先级（默认 100，数值越小优先级越高）
    fn priority(&self) -> i32 { 100 }

    /// 是否为动态插件（默认 false）
    fn is_dynamic(&self) -> bool { false }

    /// 命令处理入口
    async fn on_command(
        &self,
        ctx: &CommandPluginContext<'_>,
        invocation: &CommandInvocation,
    ) -> Option<CommandPluginSignal>;
}
```

### SystemPlugin

系统事件插件处理通知、请求和元事件。

```rust
#[async_trait]
pub trait SystemPlugin: Send + Sync + 'static {
    fn metadata(&self) -> PluginMetadata;
    fn priority(&self) -> i32 { 100 }
    fn is_dynamic(&self) -> bool { false }

    /// 处理通知事件
    async fn on_notice(
        &self,
        ctx: &SystemPluginContext<'_>,
        route: &SystemNoticeRoute,
    ) -> Option<SystemPluginSignal> { None }

    /// 处理请求事件
    async fn on_request(
        &self,
        ctx: &SystemPluginContext<'_>,
        route: &SystemRequestRoute,
    ) -> Option<SystemPluginSignal> { None }

    /// 处理元事件
    async fn on_meta(
        &self,
        ctx: &SystemPluginContext<'_>,
        route: &SystemMetaRoute,
    ) -> Option<SystemPluginSignal> { None }
}
```

### MessageEventInterceptor

消息事件拦截器。

```rust
#[async_trait]
pub trait MessageEventInterceptor: Send + Sync + 'static {
    /// 事件到达插件前调用，返回 false 拦截
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool;

    /// 所有插件处理完毕后调用（逆序执行）
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {}
}
```

### RuntimeBotContext

运行时上下文，提供发送消息和执行操作的能力。

```rust
#[async_trait]
pub trait RuntimeBotContext: Send + Sync {
    fn bot_instance(&self) -> &str;
    fn protocol(&self) -> ProtocolId;
    fn capabilities(&self) -> &CapabilitySet;
    async fn send_action(&self, req: NormalizedActionRequest) -> Result<NormalizedActionResponse>;
    async fn reply(&self, event: &NormalizedEvent, message: Message) -> Result<NormalizedActionResponse>;
    fn spawn_owned(&self, name: String, fut: OwnedTaskFuture) -> TaskHandle;
}
```

## 上下文

### CommandPluginContext

命令插件的执行上下文。

```rust
pub struct CommandPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a NormalizedEvent,
    pub runtime: &'a dyn RuntimeBotContext,
}
```

#### 方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `onebot_actions()` | `OneBotActionClient<'_>` | 获取 OneBot API 客户端 |
| `sender_id()` | `&str` | 发送者 ID |
| `sender_id_i64()` | `Option<i64>` | 发送者 ID（数字） |
| `chat_id()` | `&str` | 聊天 ID（群号或用户 ID） |
| `group_id()` | `&str` | 群号（私聊为空） |
| `group_id_i64()` | `Option<i64>` | 群号（数字） |
| `is_group()` | `bool` | 是否群聊 |
| `is_private()` | `bool` | 是否私聊 |
| `plain_text()` | `String` | 消息纯文本 |
| `message()` | `&Message` | 完整消息对象 |

### SystemPluginContext

系统事件插件的执行上下文。

```rust
pub struct SystemPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a Value,       // serde_json::Value
    pub runtime: &'a dyn RuntimeBotContext,
}
```

#### 方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `onebot_actions()` | `OneBotActionClient<'_>` | 获取 OneBot API 客户端 |

## 信号枚举

### CommandPluginSignal

```rust
pub enum CommandPluginSignal {
    /// 回复消息，继续处理后续插件
    Reply(Message),
    /// 不做任何操作，继续后续插件
    Continue,
    /// 回复消息，终止后续所有插件
    Block(Message),
    /// 静默终止后续所有插件
    Ignore,
}
```

### SystemPluginSignal

```rust
pub enum SystemPluginSignal {
    Continue,
    Reply(Message),
    ApproveFriend { flag: String, remark: Option<String> },
    RejectFriend { flag: String, reason: Option<String> },
    ApproveGroupInvite { flag: String, sub_type: String },
    RejectGroupInvite { flag: String, sub_type: String, reason: Option<String> },
    Block(Message),
    Ignore,
}
```

## 系统事件路由

### SystemNoticeRoute

```rust
pub enum SystemNoticeRoute {
    GroupUpload,
    GroupAdminSet,
    GroupAdminUnset,
    GroupDecreaseLeave,
    GroupDecreaseKick,
    GroupDecreaseKickMe,
    GroupIncreaseApprove,
    GroupIncreaseInvite,
    GroupBanBan,
    GroupBanLiftBan,
    FriendAdd,
    GroupRecall,
    FriendRecall,
    GroupPoke,
    PrivatePoke,
    NotifyLuckyKing,
    NotifyHonor,
    GroupCard,
    OfflineFile,
    ClientStatus,
    EssenceAdd,
    EssenceDelete,
    GroupReaction,
    MessageEmojiLike,
    ChannelCreated,
    ChannelDestroyed,
    ChannelUpdated,
    GuildMessageReactionsUpdated,
    Unknown(String),
}
```

### SystemRequestRoute

```rust
pub enum SystemRequestRoute {
    Friend,
    GroupAdd,
    GroupInvite,
    Unknown { request_type: String, sub_type: Option<String> },
}
```

### SystemMetaRoute

```rust
pub enum SystemMetaRoute {
    LifecycleEnable,
    LifecycleDisable,
    LifecycleConnect,
    LifecycleOther(String),
    Heartbeat,
    Unknown(String),
}
```

## 命令定义

### CommandDefinition

```rust
pub struct CommandDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub aliases: &'static [&'static str],
    pub examples: &'static [&'static str],
    pub category: &'static str,
    pub hidden: bool,
    pub required_role: CommandRole,
    pub filter: Option<MessageFilter>,
}
```

### CommandRole

```rust
pub enum CommandRole {
    Anyone,  // 任何人
    Admin,   // 管理员
    Owner,   // 所有者
}
```

### CommandInvocation

```rust
pub struct CommandInvocation {
    pub definition: CommandDefinition,
    pub args: Vec<String>,
    pub source_text: String,
}
```

## 插件元数据

### PluginMetadata

```rust
pub struct PluginMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    pub api_version: &'static str,
    pub compatibility: PluginCompatibility,
}
```

### PluginCompatibility

```rust
pub struct PluginCompatibility {
    pub host_api: &'static str,
    pub framework_min: &'static str,
    pub framework_max: &'static str,
}
```

## 自动转换 Trait

### IntoCommandSignal

以下类型可以作为命令处理器的返回值，自动转换为 `CommandPluginSignal`：

| 类型 | 转换为 |
|------|--------|
| `CommandPluginSignal` | 直接使用 |
| `Message` | `Reply(message)` |
| `String` | `Reply(Message::text(string))` |
| `&str` | `Reply(Message::text(str))` |
| `Result<T, E>` | Ok → 转换 T，Err → `Reply(Message::text("Error: ..."))` |

### IntoSystemSignal

与上类似，用于系统事件处理器的返回值。
