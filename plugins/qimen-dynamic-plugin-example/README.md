# QimenBot 动态插件示例

本项目演示如何使用 `#[dynamic_plugin]` 过程宏开发 QimenBot 动态插件——编译为 `.so` / `.dll` / `.dylib` 的独立库，运行时通过 `dlopen` 加载。

**API 版本：v0.3**

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

# macOS
cp target/release/libqimen_dynamic_plugin_example.dylib ../../plugins/bin/

# Windows
cp target/release/qimen_dynamic_plugin_example.dll ../../plugins/bin/
```

### 3. 热重载

在 Bot 中发送 `/plugins reload`，无需重启即可加载新插件。

## 本示例包含

| 命令 | 别名 | 说明 |
|------|------|------|
| `greet [内容]` | `hi`, `hello`, `你好` | 打招呼，演示 ReplyBuilder + 昵称 + 引用回复 |
| `time` | `时间` | 显示时间，演示 `CommandResponse::text()` + 时间戳 |
| `echo <内容>` | `复读`, `say` | 复读消息，演示参数解析 + 空参检查 |
| `info` | `debug`, `调试` | 显示 CommandRequest 所有字段（仅管理员） |
| `example-help` | `示例帮助` | 帮助菜单 |

| 事件路由 | 说明 |
|----------|------|
| GroupPoke, PrivatePoke | 戳一戳回复 |

| 生命周期钩子 | 说明 |
|-------------|------|
| `#[init]` | 插件加载时初始化（打印配置信息） |
| `#[shutdown]` | 插件卸载时清理 |

## 过程宏写法（推荐）

本示例使用 `#[dynamic_plugin]` 过程宏，一个 `mod` 包含所有功能：

```rust
use qimen_dynamic_plugin_derive::dynamic_plugin;
use abi_stable_host_api::*;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0")]
mod my_plugin {
    use super::*;

    // 生命周期钩子（可选）
    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult {
        PluginInitResult::ok()
    }

    #[shutdown]
    fn on_shutdown() { }

    // 命令注册
    #[command(name = "greet", description = "打招呼", aliases = "hi,hello", category = "示例")]
    fn greet(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(&format!("Hello, {}!", req.sender_nickname.as_str()))
    }

    // 事件路由
    #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
    fn on_poke(req: &NoticeRequest) -> NoticeResponse {
        NoticeResponse {
            action: DynamicActionResponse::text_reply("被戳了！"),
        }
    }
}
// 宏自动生成：
// - qimen_plugin_descriptor()（含命令、路由注册）
// - qimen_plugin_init() / qimen_plugin_shutdown()
// - 所有 extern "C" fn 导出
```

### 可用属性

| 属性 | 用途 | 参数 |
|------|------|------|
| `#[command(...)]` | 注册命令回调 | `name`, `description`, `aliases`, `category`, `role` |
| `#[route(...)]` | 注册事件路由 | `kind`（notice/request/meta）, `events` |
| `#[init]` | 插件加载钩子 | 无（函数签名固定） |
| `#[shutdown]` | 插件卸载钩子 | 无（函数签名固定） |

## CommandRequest 字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `args` | RString | 命令参数（空格分隔） |
| `command_name` | RString | 匹配的命令名 |
| `sender_id` | RString | 发送者 ID |
| `sender_nickname` | RString | 发送者昵称 **(v0.3)** |
| `group_id` | RString | 群 ID（私聊为空） |
| `message_id` | RString | 消息 ID **(v0.3)** |
| `timestamp` | i64 | Unix 秒时间戳 **(v0.3)** |
| `raw_event_json` | RString | 完整 OneBot 事件 JSON |

## 三种回复方式

```rust
// 1. 纯文本回复（最简单）
CommandResponse::text("Hello!")

// 2. ReplyBuilder 链式构建富媒体（推荐）
CommandResponse::builder()
    .reply(msg_id)         // 引用原消息
    .at("12345")           // @某人
    .at_all()              // @全体
    .text("Hello!")        // 文本
    .face(1)               // QQ 表情
    .image_url("https://...") // 图片
    .record("https://...")  // 语音
    .build()

// 3. 忽略事件
CommandResponse::ignore()
```

## Cargo.toml 模板

```toml
[package]
name = "qimen-dynamic-plugin-myname"
edition = "2024"
version = "0.1.0"
rust-version = "1.89"

[workspace]                     # 独立于主工作空间
[lib]
crate-type = ["cdylib"]         # 编译为动态库

[dependencies]
abi-stable-host-api = "0.1"
abi_stable = "0.11"
serde_json = "1"
qimen-dynamic-plugin-derive = "0.1"
```

## 手动 FFI 写法

如果不想使用过程宏，也可以手动导出 FFI 符号：

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("plugin-id", "0.1.0")
        .add_command_full(CommandDescriptorEntry { ... })
        .add_route("notice", "GroupPoke", "on_poke_symbol")
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_callback(req: &CommandRequest) -> CommandResponse { ... }
```

## 注意事项

- 动态插件使用 C ABI (`extern "C"`)，过程宏自动处理 `#[unsafe(no_mangle)]` 标记
- `abi_stable` crate 提供跨动态库安全传递的类型（`RString`、`RVec`）
- 动态插件不支持 async——所有回调都是同步执行的
- 插件 panic 会被宿主 `catch_unwind` 捕获，不会崩溃宿主进程（v0.3）
- 熔断器保护：连续 3 次失败会自动隔离插件 60 秒
- 向后兼容：v0.1 / v0.2 的插件仍然支持
