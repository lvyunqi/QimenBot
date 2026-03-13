# 插件 API 参考

本页列出 `qimen-plugin-api` crate 提供的所有公共类型。

## 导入

```rust
use qimen_plugin_api::prelude::*;
```

这一行导入了开发插件所需的全部类型。

## 核心 Trait

### Module — 插件模块

每个 `#[module]` 宏自动实现此 trait，你通常不需要手动实现。

```rust
#[async_trait]
pub trait Module: Send + Sync + 'static {
    fn id(&self) -> &'static str;
    async fn on_load(&self) -> Result<()>;
    async fn on_unload(&self) -> Result<()>;
    fn supports_hot_reload(&self) -> bool;
    fn command_plugins(&self) -> Vec<Arc<dyn CommandPlugin>>;
    fn system_plugins(&self) -> Vec<Arc<dyn SystemPlugin>>;
    fn interceptors(&self) -> Vec<Arc<dyn MessageEventInterceptor>>;
    fn register_plugins(&self, registrar: &mut dyn PluginRegistrar);
}
```

| 方法 | 说明 |
|------|------|
| `id()` | 模块唯一标识 |
| `on_load()` | 模块加载时调用 |
| `on_unload()` | 模块卸载时调用（默认空实现） |
| `supports_hot_reload()` | 是否支持热重载（默认 `false`） |
| `command_plugins()` | 返回命令插件列表 |
| `system_plugins()` | 返回系统事件插件列表 |
| `interceptors()` | 返回拦截器列表 |

### CommandPlugin — 命令插件

处理用户发送的命令（如 `/ping`、`/echo`）。

```rust
#[async_trait]
pub trait CommandPlugin: Send + Sync + 'static {
    fn metadata(&self) -> PluginMetadata;
    fn commands(&self) -> Vec<CommandDefinition>;
    fn priority(&self) -> i32;       // 默认 100
    fn is_dynamic(&self) -> bool;    // 默认 false
    async fn on_command(
        &self,
        ctx: &CommandPluginContext<'_>,
        invocation: &CommandInvocation,
    ) -> Option<CommandPluginSignal>;
}
```

::: tip 优先级
`priority()` 值越小，优先级越高。内置命令为 0，静态插件默认 100，动态插件默认 200。
:::

### SystemPlugin — 系统事件插件

处理通知、请求和元事件。

```rust
#[async_trait]
pub trait SystemPlugin: Send + Sync + 'static {
    fn metadata(&self) -> PluginMetadata;
    fn priority(&self) -> i32;
    fn is_dynamic(&self) -> bool;

    async fn on_notice(
        &self, ctx: &SystemPluginContext<'_>, route: &SystemNoticeRoute,
    ) -> Option<SystemPluginSignal>;

    async fn on_request(
        &self, ctx: &SystemPluginContext<'_>, route: &SystemRequestRoute,
    ) -> Option<SystemPluginSignal>;

    async fn on_meta(
        &self, ctx: &SystemPluginContext<'_>, route: &SystemMetaRoute,
    ) -> Option<SystemPluginSignal>;
}
```

### MessageEventInterceptor — 消息拦截器

在消息到达命令插件之前/之后执行。

```rust
#[async_trait]
pub trait MessageEventInterceptor: Send + Sync + 'static {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool;
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent);
}
```

| 方法 | 返回值 | 说明 |
|------|-------|------|
| `pre_handle` | `bool` | 返回 `false` 拦截消息，`true` 放行 |
| `after_completion` | — | 所有插件处理完后调用（逆序），默认空实现 |

### RuntimeBotContext — 运行时上下文

提供发送消息和执行操作的能力。

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

## 上下文类型

### CommandPluginContext

命令插件的执行上下文。

```rust
pub struct CommandPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a NormalizedEvent,
    pub runtime: &'a dyn RuntimeBotContext,
}
```

#### 便捷方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `onebot_actions()` | `OneBotActionClient<'_>` | 获取 OneBot API 客户端 |
| `sender_id()` | `&str` | 发送者 QQ 号 |
| `sender_id_i64()` | `Option<i64>` | 发送者 QQ 号（数字） |
| `chat_id()` | `&str` | 聊天 ID（群号或用户 ID） |
| `group_id()` | `&str` | 群号（私聊返回空字符串） |
| `group_id_i64()` | `Option<i64>` | 群号（私聊返回 `None`） |
| `is_group()` | `bool` | 是否群聊 |
| `is_private()` | `bool` | 是否私聊 |
| `plain_text()` | `String` | 消息纯文本 |
| `message()` | `&Message` | 完整消息对象 |

#### 通过 event 访问更多信息

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `event.sender_nickname()` | `Option<&str>` | 发送者昵称 |
| `event.sender_role()` | `Option<&str>` | 群角色：`"owner"` / `"admin"` / `"member"` |
| `event.sender_card()` | `Option<&str>` | 群名片 |
| `event.sender_sex()` | `Option<&str>` | 性别 |
| `event.sender_age()` | `Option<i64>` | 年龄 |
| `event.sender_title()` | `Option<&str>` | 专属头衔 |
| `event.message_id()` | `Option<i64>` | 消息 ID |
| `event.self_id()` | `Option<i64>` | Bot QQ 号 |
| `event.is_at_self()` | `bool` | 是否 @了 Bot |
| `event.is_group_admin_or_owner()` | `bool` | 发送者是否管理员/群主 |

### SystemPluginContext

系统事件插件的执行上下文。

```rust
pub struct SystemPluginContext<'a> {
    pub bot_id: &'a str,
    pub event: &'a Value,       // serde_json::Value，原始事件 JSON
    pub runtime: &'a dyn RuntimeBotContext,
}
```

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `onebot_actions()` | `OneBotActionClient<'_>` | 获取 OneBot API 客户端 |

::: tip 访问事件字段
SystemPluginContext 的 `event` 是原始 JSON。访问字段示例：
```rust
let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
let flag = ctx.event["flag"].as_str().unwrap_or_default();
let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
```
:::

## 信号枚举

### CommandPluginSignal

命令处理器返回此枚举告诉框架如何处理：

| 变体 | 说明 |
|------|------|
| `Reply(Message)` | 发送回复消息，**继续**处理后续插件 |
| `Continue` | 不做任何操作，继续后续插件 |
| `Block(Message)` | 发送回复消息，**终止**后续所有插件 |
| `Ignore` | 静默**终止**后续所有插件 |

### SystemPluginSignal

系统事件处理器返回此枚举：

| 变体 | 说明 |
|------|------|
| `Continue` | 不做特殊处理，继续下一个插件 |
| `Reply(Message)` | 回复消息并继续 |
| `ApproveFriend { flag, remark }` | 同意好友请求 |
| `RejectFriend { flag, reason }` | 拒绝好友请求 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 |
| `Block(Message)` | 回复消息并终止后续插件 |
| `Ignore` | 静默终止后续插件 |

## 事件路由枚举

### SystemNoticeRoute — 通知路由

| 路由 | 说明 |
|------|------|
| `GroupPoke` | 群聊戳一戳 |
| `PrivatePoke` | 私聊戳一戳 |
| `GroupIncreaseApprove` | 新成员通过审批入群 |
| `GroupIncreaseInvite` | 新成员被邀请入群 |
| `GroupDecreaseLeave` | 成员主动退群 |
| `GroupDecreaseKick` | 成员被踢出群 |
| `GroupDecreaseKickMe` | Bot 被踢出群 |
| `GroupRecall` | 群消息被撤回 |
| `FriendRecall` | 好友消息被撤回 |
| `GroupBanBan` | 成员被禁言 |
| `GroupBanLiftBan` | 成员被解除禁言 |
| `FriendAdd` | 新好友已添加 |
| `GroupUpload` | 群文件上传 |
| `GroupAdminSet` | 成员被设为管理员 |
| `GroupAdminUnset` | 成员被取消管理员 |
| `GroupCard` | 群名片变更 |
| `EssenceAdd` | 消息被设为精华 |
| `EssenceDelete` | 精华消息被移除 |
| `NotifyLuckyKing` | 运气王 |
| `NotifyHonor` | 荣誉变更 |
| `OfflineFile` | 离线文件 |
| `ClientStatus` | 客户端状态变更 |
| `GroupReaction` | 群消息表情回应 |
| `MessageEmojiLike` | 消息表情点赞 |
| `ChannelCreated` | 频道创建 |
| `ChannelDestroyed` | 频道销毁 |
| `ChannelUpdated` | 频道更新 |
| `GuildMessageReactionsUpdated` | 频道消息表情更新 |
| `Unknown(String)` | 未知通知类型 |

### SystemRequestRoute — 请求路由

| 路由 | 说明 |
|------|------|
| `Friend` | 好友申请 |
| `GroupAdd` | 用户申请加群 |
| `GroupInvite` | 被邀请加入某群 |
| `Unknown { request_type, sub_type }` | 未知请求类型 |

### SystemMetaRoute — 元事件路由

| 路由 | 说明 |
|------|------|
| `LifecycleEnable` | OneBot 启用 |
| `LifecycleDisable` | OneBot 禁用 |
| `LifecycleConnect` | 连接建立 |
| `LifecycleOther(String)` | 其他生命周期事件 |
| `Heartbeat` | 心跳包 |
| `Unknown(String)` | 未知元事件 |

## 命令定义

### CommandDefinition

```rust
pub struct CommandDefinition {
    pub name: &'static str,            // 命令名
    pub description: &'static str,     // 命令描述
    pub aliases: &'static [&'static str], // 别名列表
    pub examples: &'static [&'static str], // 使用示例
    pub category: &'static str,        // 分类（默认 "general"）
    pub hidden: bool,                  // 是否隐藏
    pub required_role: CommandRole,     // 权限要求
    pub scope: CommandScope,           // 作用域（默认 All）
    pub filter: Option<MessageFilter>, // 消息过滤器
}
```

### CommandScope

声明命令的生效范围，分发层自动过滤不匹配的环境。

```rust
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum CommandScope {
    #[default]
    All,      // 群聊和私聊均可触发
    Group,    // 仅在群聊中触发
    Private,  // 仅在私聊中触发
}
```

| 变体 | 说明 |
|------|------|
| `All` (默认) | 群聊和私聊均可触发 |
| `Group` | 仅在群聊中触发，私聊中静默忽略 |
| `Private` | 仅在私聊中触发，群聊中静默忽略 |

### CommandRole

| 变体 | 说明 |
|------|------|
| `Anyone` | 任何人都能使用 |
| `Admin` | 仅管理员和所有者 |
| `Owner` | 仅所有者 |

### CommandInvocation

```rust
pub struct CommandInvocation {
    pub definition: CommandDefinition,  // 匹配到的命令定义
    pub args: Vec<String>,             // 命令参数
    pub source_text: String,           // 原始命令文本
}
```

## 消息过滤器

### MessageFilter

高级消息匹配规则，可通过 `CommandDefinition` 设置：

| 字段 | 类型 | 说明 |
|------|------|------|
| `cmd` | `Option<String>` | 正则匹配命令文本 |
| `starts_with` | `Option<String>` | 前缀匹配 |
| `ends_with` | `Option<String>` | 后缀匹配 |
| `contains` | `Option<String>` | 包含匹配 |
| `groups` | `Vec<i64>` | 群白名单 |
| `senders` | `Vec<i64>` | 发送者白名单 |
| `at_mode` | `AtMode` | @检测：`Need` / `NotNeed` / `Both` |
| `reply_filter` | `ReplyFilter` | 回复检测 |
| `media_types` | `Vec<MediaType>` | 媒体类型要求 |
| `invert` | `bool` | 取反 |

## 自动转换 Trait

### IntoCommandSignal

以下类型可以作为命令处理器的返回值，框架自动转换：

| 返回类型 | 转换为 |
|---------|--------|
| `CommandPluginSignal` | 直接使用 |
| `Message` | `Reply(message)` |
| `String` | `Reply(Message::text(string))` |
| `&str` | `Reply(Message::text(str))` |
| `Result<T, E>` | Ok → 转换 T，Err → `Reply(Message::text("Error: ..."))` |

### IntoSystemSignal

同上，用于系统事件处理器的返回值。

## 插件元数据

### PluginMetadata

```rust
pub struct PluginMetadata {
    pub id: &'static str,          // 插件唯一标识
    pub name: &'static str,        // 显示名称
    pub version: &'static str,     // 版本号
    pub description: &'static str, // 描述
    pub api_version: &'static str, // API 版本
    pub compatibility: PluginCompatibility,
}
```

### PluginCompatibility

```rust
pub struct PluginCompatibility {
    pub host_api: &'static str,       // 宿主 API 版本
    pub framework_min: &'static str,  // 最低框架版本
    pub framework_max: &'static str,  // 最高框架版本
}
```
