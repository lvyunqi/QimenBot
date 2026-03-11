# QimenBot 动态插件示例

本项目演示如何开发 QimenBot 动态插件——编译为 `.so` / `.dll` / `.dylib` 的独立库，运行时通过 `dlopen` 加载。

## 快速开始

### 1. 编译

```bash
cd plugins/qimen-dynamic-plugin-example
cargo build --release
```

### 2. 部署

将生成的动态库复制到 `plugins/bin/` 目录：

```bash
# Linux
cp target/release/libqimen_dynamic_plugin_example.so ../../plugins/bin/

# Windows
cp target/release/qimen_dynamic_plugin_example.dll ../../plugins/bin/

# macOS
cp target/release/libqimen_dynamic_plugin_example.dylib ../../plugins/bin/
```

### 3. 热重载

在 Bot 中发送 `/plugins reload`，无需重启即可加载新插件。

## 本示例包含

| 命令 | 别名 | 说明 |
|------|------|------|
| `/greet [消息]` | `/hi`, `/hello` | 向发送者打招呼，支持富媒体（文本+表情） |
| `/time` | - | 显示当前 Unix 时间戳 |

| 事件路由 | 说明 |
|----------|------|
| GroupPoke, PrivatePoke | 戳一戳回复 |

## FFI 接口 (v0.2)

### 必须导出的符号

每个动态插件必须导出 `qimen_plugin_descriptor` 函数：

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("plugin-id", "0.1.0")
        .add_command("cmd", "描述", "callback_symbol")
        .add_route("notice", "GroupPoke", "on_poke_symbol")
}
```

### 命令回调签名

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn callback_symbol(req: &CommandRequest) -> CommandResponse {
    // req.args        — 命令参数
    // req.sender_id   — 发送者 ID
    // req.group_id    — 群 ID（私聊为空）
    // req.command_name — 匹配的命令名
    // req.raw_event_json — 原始 OneBot 事件 JSON
    CommandResponse {
        action: DynamicActionResponse::text_reply("回复内容"),
    }
}
```

### 事件回调签名

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn on_poke_symbol(req: &NoticeRequest) -> NoticeResponse {
    // req.route         — 路由名（如 "GroupPoke"）
    // req.raw_event_json — 原始 OneBot 事件 JSON
    NoticeResponse {
        action: DynamicActionResponse::text_reply("被戳了！"),
    }
}
```

### 响应类型

| 方法 | 用途 |
|------|------|
| `DynamicActionResponse::text_reply(text)` | 纯文本回复 |
| `DynamicActionResponse::rich_reply(json)` | 富媒体回复（OneBot 段 JSON） |
| `DynamicActionResponse::ignore()` | 忽略事件 |
| `DynamicActionResponse::approve(remark)` | 同意请求 |
| `DynamicActionResponse::reject(reason)` | 拒绝请求 |

### 富媒体段格式

```rust
let segments = serde_json::json!([
    { "type": "text", "data": { "text": "Hello " } },
    { "type": "face", "data": { "id": "1" } },
    { "type": "at", "data": { "qq": "123456" } },
    { "type": "image", "data": { "file": "https://example.com/img.png" } }
]);
DynamicActionResponse::rich_reply(&segments.to_string())
```

## Cargo.toml 模板

```toml
[package]
name = "my-dynamic-plugin"
edition = "2024"
version = "0.1.0"

[lib]
crate-type = ["cdylib"]  # 编译为动态库

[workspace]  # 独立于主工作空间

[dependencies]
abi-stable-host-api = { path = "../../crates/abi-stable-host-api" }
abi_stable = "0.11"
serde_json = "1"  # 可选，用于构建富媒体和解析事件
```

## 注意事项

- 动态插件使用 C ABI (`extern "C"`)，所有导出函数必须标记 `#[unsafe(no_mangle)]`
- `abi_stable` crate 提供跨动态库安全传递的类型（`RString`、`RVec`）
- 动态插件不支持 async——所有回调都是同步执行的
- 熔断器保护：连续 3 次失败会自动隔离插件 60 秒
- 向后兼容：v0.1 的 `qimen_demo_plugin_descriptor` 符号名仍然支持
