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
| **0.3** | `CommandRequest` 新增 `sender_nickname` / `message_id` / `timestamp`；`ReplyBuilder` 流式构建；`PluginInitConfig` / `PluginInitResult` 生命周期钩子；`CommandDescriptorEntry` 新增 `scope` 字段 |

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

::: warning Rust 2024 Edition
Rust 2024 Edition 要求写 `#[unsafe(no_mangle)]` 而不是旧版的 `#[no_mangle]`。如果你使用 `edition = "2024"`，必须使用新语法。
:::

## 向后兼容

v0.3 FFI 接口向后兼容 v0.1 和 v0.2：

- v0.1 的 `qimen_demo_plugin_descriptor` 符号名仍然支持
- v0.1 的单命令/单路由字段仍然可用
- 框架会优先尝试 v0.2+ 符号 `qimen_plugin_descriptor`，找不到再尝试 v0.1
- v0.2 的 `CommandDescriptorEntry`（无 `scope` 字段）会自动使用 `scope = "all"` 默认值
