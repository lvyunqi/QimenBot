# FFI 接口参考

本页详细列出 `abi-stable-host-api` crate 提供的动态插件 FFI 类型和函数。

## API 版本

当前 API 版本为 **0.3**，兼容 0.1 和 0.2 版本。

```rust
/// 获取当前 API 版本
pub fn expected_api_version() -> RString  // "0.3"

/// 检查版本兼容性
pub fn is_compatible_api_version(version: &str) -> bool
// "0.1" → true
// "0.2" → true
// "0.3" → true
// "0.4" → false
```

### 版本历史

| 版本 | 新增 |
|------|------|
| **0.1** | 初始版本：单命令、纯文本响应 |
| **0.2** | 多命令/多路由 `RVec<CommandDescriptorEntry>`，富媒体 JSON 响应 |
| **0.3** | `CommandRequest` 新增 `sender_nickname` / `message_id` / `timestamp`；`ReplyBuilder` 流式构建；`PluginInitConfig` / `PluginInitResult` 生命周期钩子；`CommandDescriptorEntry` 新增 `scope` 字段；`InterceptorRequest` / `InterceptorResponse` / `InterceptorDescriptorEntry` 拦截器支持 |

## PluginDescriptor

插件描述符，宿主通过 `qimen_plugin_descriptor()` 符号获取。

```rust
#[repr(C)]
pub struct PluginDescriptor {
    pub plugin_id: RString,
    pub plugin_version: RString,
    pub api_version: RString,

    // v0.1 遗留字段（已弃用，使用 commands/routes 代替）
    pub command_name: RString,
    pub command_description: RString,
    pub notice_route: RString,
    pub request_route: RString,
    pub meta_route: RString,

    // v0.2+ 字段
    pub commands: RVec<CommandDescriptorEntry>,
    pub routes: RVec<RouteDescriptorEntry>,
    pub interceptors: RVec<InterceptorDescriptorEntry>,
}
```

### 构造方法

```rust
/// 创建新的描述符（api_version 自动设为 "0.3"）
PluginDescriptor::new(id: &str, version: &str) -> Self

/// 添加命令（简单方式）
.add_command(name: &str, description: &str, callback_symbol: &str) -> Self

/// 添加命令（完整方式，支持别名/分类/权限/作用域）
.add_command_full(entry: CommandDescriptorEntry) -> Self

/// 添加事件路由
.add_route(kind: &str, route: &str, callback_symbol: &str) -> Self

/// 添加拦截器（空字符串表示不注册对应回调）
.add_interceptor(pre_handle_symbol: &str, after_completion_symbol: &str) -> Self
```

## CommandDescriptorEntry

命令描述条目。

```rust
#[repr(C)]
pub struct CommandDescriptorEntry {
    pub name: RString,             // 命令名
    pub description: RString,      // 描述
    pub callback_symbol: RString,  // 回调函数符号名
    pub aliases: RString,          // 别名（逗号分隔，如 "hi,hello"）
    pub category: RString,         // 分类（如 "general"）
    pub required_role: RString,    // 权限要求（""=任何人, "admin", "owner"）
    pub scope: RString,            // 作用域（""/"all"=全部, "group"=仅群聊, "private"=仅私聊）
}
```

| `scope` 值 | 说明 |
|------------|------|
| `""` / `"all"` | 群聊和私聊均可触发（默认） |
| `"group"` | 仅在群聊中触发 |
| `"private"` | 仅在私聊中触发 |

## RouteDescriptorEntry

事件路由描述条目。

```rust
#[repr(C)]
pub struct RouteDescriptorEntry {
    pub kind: RString,             // 事件类型："notice", "request", "meta"
    pub route: RString,            // 路由名（逗号分隔多个）
    pub callback_symbol: RString,  // 回调函数符号名
}
```

## CommandRequest

传递给命令回调的请求数据。

```rust
#[repr(C)]
pub struct CommandRequest {
    pub args: RString,             // 命令参数（空格分隔后的文本）
    pub command_name: RString,     // 匹配到的命令名
    pub sender_id: RString,        // 发送者 QQ 号
    pub group_id: RString,         // 群号（私聊为空字符串）
    pub raw_event_json: RString,   // 原始 OneBot 事件完整 JSON

    // ── v0.3 新增 ──
    pub sender_nickname: RString,  // 发送者昵称
    pub message_id: RString,       // 消息 ID
    pub timestamp: i64,            // 事件 Unix 时间戳（秒），0 表示不可用
}
```

### 使用示例

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_callback(req: &CommandRequest) -> CommandResponse {
    let sender = req.sender_id.as_str();
    let nickname = req.sender_nickname.as_str();
    let args = req.args.as_str().trim();
    let msg_id = req.message_id.as_str();

    // 使用 ReplyBuilder 构建富媒体回复
    let mut builder = CommandResponse::builder();
    if !msg_id.is_empty() {
        builder = builder.reply(msg_id);  // 引用回复
    }
    builder
        .at(sender)
        .text(&format!(" 你好 {}！", nickname))
        .build()
}
```

## CommandResponse

命令回调返回的响应。

```rust
#[repr(C)]
pub struct CommandResponse {
    pub action: DynamicActionResponse,
}
```

### 快捷构造

```rust
/// 纯文本回复
CommandResponse::text("Hello!")

/// 忽略事件
CommandResponse::ignore()

/// 流式构建富媒体回复
CommandResponse::builder()  // → ReplyBuilder
```

## ReplyBuilder

流式构建富媒体命令响应，无需手动拼接 JSON。

```rust
let response = CommandResponse::builder()
    .reply("12345")              // 引用回复
    .at("67890")                 // @某人
    .at_all()                    // @全体
    .text("Hello!")              // 文本
    .face(1)                     // QQ 表情
    .image_url("https://...")    // 图片（URL）
    .image_base64("iVBOR...")    // 图片（Base64）
    .record("https://...")       // 语音
    .build();                    // → CommandResponse
```

| 方法 | 参数 | 说明 |
|------|------|------|
| `text(text)` | `&str` | 文本段 |
| `at(user_id)` | `&str` | @某人 |
| `at_all()` | — | @全体成员 |
| `face(id)` | `i32` | QQ 表情 |
| `image_url(url)` | `&str` | 图片（URL） |
| `image_base64(base64)` | `&str` | 图片（Base64） |
| `record(file)` | `&str` | 语音（URL 或路径） |
| `reply(message_id)` | `&str` | 引用回复 |
| `build()` | — | 构建为 `CommandResponse` |

## InterceptorDescriptorEntry

拦截器描述条目。

```rust
#[repr(C)]
pub struct InterceptorDescriptorEntry {
    pub pre_handle_symbol: RString,       // pre_handle 回调符号名（空 = 不注册）
    pub after_completion_symbol: RString, // after_completion 回调符号名（空 = 不注册）
}
```

## InterceptorRequest

传递给拦截器回调的请求数据。

```rust
#[repr(C)]
pub struct InterceptorRequest {
    pub bot_id: RString,           // Bot 实例 ID
    pub sender_id: RString,        // 发送者 QQ 号
    pub group_id: RString,         // 群号（私聊为空字符串）
    pub message_text: RString,     // 消息纯文本
    pub raw_event_json: RString,   // 完整事件 JSON
    pub sender_nickname: RString,  // 发送者昵称
    pub message_id: RString,       // 消息 ID
    pub timestamp: i64,            // 事件 Unix 时间戳（秒），0 表示不可用
}
```

## InterceptorResponse

`pre_handle` 拦截器回调的返回值。

```rust
#[repr(C)]
pub struct InterceptorResponse {
    pub allow: i32,   // 1 = 放行，0 = 拦截
}
```

```rust
/// 放行
InterceptorResponse::allow() -> Self

/// 拦截
InterceptorResponse::block() -> Self
```

## NoticeRequest

传递给事件回调的请求数据。

```rust
#[repr(C)]
pub struct NoticeRequest {
    pub route: RString,            // 路由名（如 "GroupPoke"）
    pub raw_event_json: RString,   // 原始 OneBot 事件 JSON
}
```

## NoticeResponse

事件回调返回的响应。

```rust
#[repr(C)]
pub struct NoticeResponse {
    pub action: DynamicActionResponse,
}
```

## DynamicActionResponse

所有回调统一使用的响应类型。

```rust
#[repr(C)]
pub struct DynamicActionResponse {
    pub action_kind: i32,       // 动作类型
    pub message: RString,       // 纯文本消息
    pub segments_json: RString, // 富媒体 JSON（优先于 message）
}
```

### 动作类型常量

| 常量 | 值 | 说明 |
|------|---|------|
| `ACTION_IGNORE` | 0 | 忽略事件 |
| `ACTION_REPLY` | 1 | 回复消息 |
| `ACTION_APPROVE` | 2 | 同意请求 |
| `ACTION_REJECT` | 3 | 拒绝请求 |

### 便捷构造方法

```rust
/// 纯文本回复
DynamicActionResponse::text_reply(text: &str) -> Self

/// 富媒体回复（OneBot 消息段 JSON）
DynamicActionResponse::rich_reply(segments_json: &str) -> Self

/// 忽略事件
DynamicActionResponse::ignore() -> Self

/// 同意请求（好友/群）
DynamicActionResponse::approve(remark: &str) -> Self

/// 拒绝请求
DynamicActionResponse::reject(reason: &str) -> Self
```

::: tip 优先级
框架处理响应时，**`segments_json` 优先于 `message`**。如果 `segments_json` 非空，框架会将其解析为 OneBot 消息段；否则使用 `message` 作为纯文本回复。
:::

## 生命周期钩子 (v0.3)

### PluginInitConfig

初始化钩子接收的配置。

```rust
#[repr(C)]
pub struct PluginInitConfig {
    pub plugin_id: RString,    // 插件 ID
    pub config_json: RString,  // 插件配置 JSON（从 config/plugins/<id>.toml 加载）
    pub plugin_dir: RString,   // 插件二进制所在目录
    pub data_dir: RString,     // 数据目录根路径
}
```

### PluginInitResult

初始化钩子的返回值。

```rust
#[repr(C)]
pub struct PluginInitResult {
    pub code: i32,                // 0 = 成功，非 0 = 失败
    pub error_message: RString,   // 失败时的错误信息
}
```

```rust
/// 初始化成功
PluginInitResult::ok() -> Self

/// 初始化失败
PluginInitResult::err(message: &str) -> Self
```

### 导出符号

```rust
/// 初始化（可选）— 插件加载后调用
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_init(config: PluginInitConfig) -> PluginInitResult

/// 关闭（可选）— 插件卸载前调用
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_shutdown()
```

## 导出函数签名

### 插件描述符（必须）

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor
```

### 命令回调

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn callback_name(req: &CommandRequest) -> CommandResponse
```

### 事件回调

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn callback_name(req: &NoticeRequest) -> NoticeResponse
```

### 拦截器回调

```rust
/// pre_handle — 返回 InterceptorResponse 控制放行/拦截
#[unsafe(no_mangle)]
pub unsafe extern "C" fn callback_name(req: &InterceptorRequest) -> InterceptorResponse

/// after_completion — 无返回值
#[unsafe(no_mangle)]
pub unsafe extern "C" fn callback_name(req: &InterceptorRequest)
```

::: warning Rust 2024 Edition
Rust 2024 Edition 要求写 `#[unsafe(no_mangle)]` 而不是旧版的 `#[no_mangle]`。如果你使用 `edition = "2024"`，必须使用新语法。
:::

## SendAction

插件通过 `BotApi` / `SendBuilder` 队列化的发送动作。回调返回后由宿主异步执行。

```rust
#[repr(C)]
pub struct SendAction {
    pub message_type: RString,    // "private" 或 "group"
    pub target_id: RString,       // user_id 或 group_id
    pub message: RString,         // 纯文本（segments_json 为空时使用）
    pub segments_json: RString,   // 富媒体 JSON（优先于 message）
}
```

| 字段 | 说明 |
|------|------|
| `message_type` | `"private"`（私聊）或 `"group"`（群聊） |
| `target_id` | 目标 QQ 号或群号（字符串） |
| `message` | 纯文本消息体 |
| `segments_json` | OneBot 消息段 JSON，非空时优先于 `message` |

::: tip
通常不需要手动构造 `SendAction`，使用 `BotApi` 或 `SendBuilder` 即可。
:::

## BotApi

静态方法集合，在 FFI 回调中向任意目标发送消息。内部 push 到进程内 `SEND_QUEUE`，回调返回后宿主 flush 并异步发送。

```rust
/// 向私聊发送纯文本
BotApi::send_private_msg(user_id: &str, text: &str)

/// 向群发送纯文本
BotApi::send_group_msg(group_id: &str, text: &str)

/// 向私聊发送富媒体
BotApi::send_private_rich(user_id: &str, segments_json: &str)

/// 向群发送富媒体
BotApi::send_group_rich(group_id: &str, segments_json: &str)
```

### 使用示例

```rust
use abi_stable_host_api::BotApi;

#[command(name = "broadcast", description = "广播消息")]
fn broadcast(req: &CommandRequest) -> CommandResponse {
    BotApi::send_group_msg("111111", "广播消息！");
    BotApi::send_group_msg("222222", "广播消息！");
    BotApi::send_private_msg("333333", "管理通知");
    CommandResponse::text("广播完成")
}
```

## SendBuilder

流式构建并入队发送到任意目标的富媒体消息，类似 `ReplyBuilder` 但目标自由指定。

```rust
/// 开始构建群消息
SendBuilder::group(group_id: &str) -> SendBuilder

/// 开始构建私聊消息
SendBuilder::private(user_id: &str) -> SendBuilder
```

### 链式方法

| 方法 | 参数 | 说明 |
|------|------|------|
| `.text(text)` | `&str` | 文本段 |
| `.at(user_id)` | `&str` | @某人 |
| `.at_all()` | — | @全体成员 |
| `.face(id)` | `i32` | QQ 表情 |
| `.image_url(url)` | `&str` | 图片（URL） |
| `.image_base64(base64)` | `&str` | 图片（Base64） |
| `.send()` | — | 入队发送（消耗 builder） |

### 使用示例

```rust
use abi_stable_host_api::SendBuilder;

SendBuilder::group("123456")
    .text("来自 ")
    .at("789")
    .text(" 的消息")
    .face(1)
    .send();

SendBuilder::private("789")
    .text("私信通知")
    .image_url("https://example.com/img.png")
    .send();
```

## 队列刷新符号

使用 `#[dynamic_plugin]` 宏时自动生成，无需手动编写：

```rust
/// 宏自动生成 — drain 插件的发送队列
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_flush_sends() -> RVec<SendAction>
```

::: tip 向后兼容
旧插件不导出此符号时，宿主会优雅降级（返回空列表），行为无变化。使用宏的旧插件重新编译后自动获得此符号。
:::

## 向后兼容

v0.3 FFI 接口向后兼容 v0.1 和 v0.2：

- v0.1 的 `qimen_demo_plugin_descriptor` 符号名仍然支持
- v0.1 的单命令/单路由字段仍然可用
- 框架会优先尝试 v0.2+ 符号 `qimen_plugin_descriptor`，找不到再尝试 v0.1
- v0.2 的 `CommandDescriptorEntry`（无 `scope` 字段）会自动使用 `scope = "all"` 默认值
- 旧插件无 `qimen_plugin_flush_sends` 符号时宿主返回空 Vec，无副作用
