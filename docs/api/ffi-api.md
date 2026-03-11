# FFI 接口参考

本页详细列出 `abi-stable-host-api` crate 提供的动态插件 FFI 类型和函数。

## API 版本

当前 API 版本为 **0.2**，兼容 0.1 版本。

```rust
/// 获取当前 API 版本
pub fn expected_api_version() -> RString  // "0.2"

/// 检查版本兼容性
pub fn is_compatible_api_version(version: &str) -> bool
// "0.1" → true
// "0.2" → true
// "0.3" → false
```

## PluginDescriptor

插件描述符，宿主通过 `qimen_plugin_descriptor()` 符号获取。

```rust
#[repr(C)]
pub struct PluginDescriptor {
    pub api_version: RString,
    pub plugin_id: RString,
    pub plugin_version: RString,
    // v0.1 遗留字段（已弃用，使用 commands/routes 代替）
    pub command_name: RString,
    pub command_description: RString,
    pub command_callback_symbol: RString,
    pub notice_route: RString,
    pub notice_callback_symbol: RString,
    // v0.2 字段
    pub commands: RVec<CommandDescriptorEntry>,
    pub routes: RVec<RouteDescriptorEntry>,
}
```

### 构造方法

```rust
/// 创建新的描述符
PluginDescriptor::new(id: &str, version: &str) -> Self

/// 添加命令（简单方式）
.add_command(name: &str, description: &str, callback_symbol: &str) -> Self

/// 添加命令（完整方式，支持别名/分类/权限）
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
}
```

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
}
```

### 使用示例

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_callback(req: &CommandRequest) -> CommandResponse {
    let sender = req.sender_id.as_str();
    let args = req.args.as_str().trim();
    let group = req.group_id.as_str();
    let cmd = req.command_name.as_str();

    // 解析原始事件获取更多信息
    if let Ok(event) = serde_json::from_str::<serde_json::Value>(req.raw_event_json.as_str()) {
        let nickname = event["sender"]["nickname"].as_str().unwrap_or("unknown");
        // ...
    }

    CommandResponse {
        action: DynamicActionResponse::text_reply(&format!("收到命令 /{cmd}")),
    }
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

### rich_reply 格式

`segments_json` 应为 OneBot 11 消息段 JSON 数组：

```json
[
    {"type": "text", "data": {"text": "Hello "}},
    {"type": "at", "data": {"qq": "123456"}},
    {"type": "face", "data": {"id": "1"}},
    {"type": "image", "data": {"file": "https://example.com/img.png"}}
]
```

::: tip 优先级
框架处理响应时，**`segments_json` 优先于 `message`**。如果 `segments_json` 非空，框架会将其解析为 OneBot 消息段；否则使用 `message` 作为纯文本回复。
:::

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

v0.2 FFI 接口向后兼容 v0.1：

- v0.1 的 `qimen_demo_plugin_descriptor` 符号名仍然支持
- v0.1 的单命令/单路由字段仍然可用
- 框架会优先尝试 v0.2 符号 `qimen_plugin_descriptor`，找不到再尝试 v0.1
