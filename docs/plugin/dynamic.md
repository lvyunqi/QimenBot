# 动态插件开发

除了与框架一同编译的**静态插件**，QimenBot 还支持**动态插件**——编译为独立的动态库（`.so` / `.dll` / `.dylib`），运行时通过 `dlopen` 加载，支持 `/plugins reload` 热重载。

## 两种插件对比

| 特性 | 静态插件 | 动态插件 |
|------|---------|---------|
| 编译方式 | 与框架一同编译 | 独立编译为动态库 |
| API 访问 | 完整（async、OneBotActionClient 等） | FFI 接口（同步、C ABI） |
| 消息构建 | `Message` + `MessageBuilder` | `ReplyBuilder`（流式构建）/ JSON 段 |
| 拦截器 | `MessageEventInterceptor` trait（async） | `#[pre_handle]` / `#[after_completion]`（同步 FFI） |
| 热重载 | 需要重启进程 | `/plugins reload` 即可 |
| 生命周期 | `on_load` / `on_unload` | `#[init]` / `#[shutdown]` |
| 适用场景 | 核心功能、需要异步 API | 第三方扩展、快速迭代 |

::: tip 如何选择？
- 如果你是框架开发者，或者需要使用异步 API → 选择**静态插件**
- 如果你是第三方开发者，需要快速迭代和热重载 → 选择**动态插件**
:::

## 快速开始 {#quickstart}

动态插件推荐使用 `#[dynamic_plugin]` **过程宏**来编写。宏会自动生成所有 FFI 导出函数，你只需关注业务逻辑。

### 第 1 步：创建项目

```bash
cargo new --lib plugins/qimen-dynamic-plugin-myplugin
cd plugins/qimen-dynamic-plugin-myplugin
```

### 第 2 步：配置 Cargo.toml

```toml
[package]
name = "qimen-dynamic-plugin-myplugin"
edition = "2024"
version = "0.1.0"

[lib]
crate-type = ["cdylib"]  # 编译为动态库

[workspace]  # 独立于主工作空间

[dependencies]
abi-stable-host-api = { path = "../../crates/abi-stable-host-api" }
qimen-dynamic-plugin-derive = { path = "../../crates/qimen-dynamic-plugin-derive" }
abi_stable = "0.11"
serde_json = "1"  # 可选，用于解析事件 JSON
```

::: warning 重要配置
- `crate-type = ["cdylib"]` — 必须设为 cdylib 才能编译为动态库
- `[workspace]` — 空的 workspace 表，使这个 crate 不属于主工作空间
:::

### 第 3 步：编写插件

```rust
use abi_stable_host_api::{CommandRequest, CommandResponse, NoticeRequest, NoticeResponse};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0")]
mod my_plugin {
    use super::*;

    #[command(name = "hello", description = "向发送者打招呼")]
    fn hello(req: &CommandRequest) -> CommandResponse {
        let name = req.sender_nickname.as_str();
        let display = if name.is_empty() { req.sender_id.as_str() } else { name };
        CommandResponse::text(&format!("你好，{display}！"))
    }
}
```

就这么简单！宏自动帮你生成了 `qimen_plugin_descriptor()` 和 `extern "C" fn hello(...)` 导出。

### 第 4 步：编译

```bash
cd plugins/qimen-dynamic-plugin-myplugin
cargo build --release
```

编译产物位于 `target/release/` 目录：

| 平台 | 文件名 |
|------|--------|
| Linux | `libqimen_dynamic_plugin_myplugin.so` |
| macOS | `libqimen_dynamic_plugin_myplugin.dylib` |
| Windows | `qimen_dynamic_plugin_myplugin.dll` |

### 第 5 步：部署

将动态库复制到配置文件中 `plugin_bin_dir` 指定的目录（默认 `plugins/bin/`）：

```bash
# Linux
cp target/release/libqimen_dynamic_plugin_myplugin.so ../../plugins/bin/

# macOS
cp target/release/libqimen_dynamic_plugin_myplugin.dylib ../../plugins/bin/

# Windows
cp target/release/qimen_dynamic_plugin_myplugin.dll ../../plugins/bin/
```

### 第 6 步：加载

在 Bot 中发送 `/plugins reload`，无需重启即可加载新插件。

## `#[dynamic_plugin]` 宏详解 {#macro}

### 宏属性

```rust
#[dynamic_plugin(id = "插件ID", version = "版本号")]
mod 模块名 {
    // ...
}
```

| 属性 | 必填 | 说明 |
|------|:----:|------|
| `id` | ✅ | 插件唯一标识 |
| `version` | ✅ | 插件版本号 |

### `#[command]` — 注册命令

```rust
#[command(
    name = "命令名",
    description = "命令描述",
    aliases = "别名1,别名2",        // 逗号分隔
    category = "分类",              // 默认 "dynamic"
    role = "admin",                 // 权限要求
    scope = "group",                // 作用域
)]
fn my_command(req: &CommandRequest) -> CommandResponse {
    // ...
}
```

| 属性 | 必填 | 默认值 | 说明 |
|------|:----:|-------|------|
| `name` | ✅ | — | 命令名 |
| `description` | ✅ | — | 命令描述 |
| `aliases` | ❌ | `""` | 逗号分隔的别名列表 |
| `category` | ❌ | `""` | 命令分类 |
| `role` | ❌ | `""` | 权限：`""` = 任何人、`"admin"` = 管理员、`"owner"` = 所有者 |
| `scope` | ❌ | `""` | 作用域：`""` / `"all"` = 全部、`"group"` = 仅群聊、`"private"` = 仅私聊 |

### `#[route]` — 注册事件路由

```rust
#[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
fn on_poke(req: &NoticeRequest) -> NoticeResponse {
    // ...
}
```

| 属性 | 必填 | 说明 |
|------|:----:|------|
| `kind` | ✅ | 事件类型：`"notice"` / `"request"` / `"meta"` |
| `events` | ✅ | 逗号分隔的路由名 |

| kind | 可用 events |
|------|------------|
| `"notice"` | `GroupPoke`, `PrivatePoke`, `GroupIncreaseApprove`, `GroupDecreaseKick`, `GroupRecall`, `FriendRecall` 等 |
| `"request"` | `Friend`, `GroupAdd`, `GroupInvite` |
| `"meta"` | `Heartbeat`, `LifecycleConnect` 等 |

完整路由列表参见 [事件路由](/plugin/events)。

### `#[init]` — 初始化钩子

插件加载后由框架自动调用。配置从 `config/plugins/<plugin_id>.toml` 加载并以 JSON 传入。

```rust
#[init]
fn on_init(config: PluginInitConfig) -> PluginInitResult {
    let plugin_id = config.plugin_id.as_str();
    let config_json = config.config_json.as_str();  // 插件配置 JSON
    let plugin_dir = config.plugin_dir.as_str();     // 插件所在目录
    let data_dir = config.data_dir.as_str();         // 数据目录

    // 初始化数据库连接、加载配置等...
    PluginInitResult::ok()
}
```

#### PluginInitConfig

| 字段 | 类型 | 说明 |
|------|------|------|
| `plugin_id` | `RString` | 插件 ID |
| `config_json` | `RString` | 插件配置（JSON 字符串），从 `config/plugins/<id>.toml` 加载，空字符串表示无配置文件 |
| `plugin_dir` | `RString` | 插件二进制所在目录 |
| `data_dir` | `RString` | Bot 数据目录根路径 |

#### PluginInitResult

```rust
// 初始化成功
PluginInitResult::ok()

// 初始化失败（框架会记录错误并跳过此插件）
PluginInitResult::err("数据库连接失败")
```

### `#[shutdown]` — 关闭钩子

插件卸载前由框架调用，用于清理资源。

```rust
#[shutdown]
fn on_shutdown() {
    // 关闭数据库连接、保存状态等...
}
```

::: warning 限制
每个插件模块内最多一个 `#[init]` 和一个 `#[shutdown]` 函数。
:::

### `#[pre_handle]` — 消息预处理拦截器 {#pre-handle}

在消息到达命令插件**之前**执行。返回 `InterceptorResponse::allow()` 放行，`InterceptorResponse::block()` 拦截。

```rust
use abi_stable_host_api::{InterceptorRequest, InterceptorResponse};

#[pre_handle]
fn my_filter(req: &InterceptorRequest) -> InterceptorResponse {
    let sender = req.sender_id.as_str();
    let text = req.message_text.as_str();

    // 示例：拦截包含特定关键词的消息
    if text.contains("spam") {
        return InterceptorResponse::block();
    }

    InterceptorResponse::allow()
}
```

### `#[after_completion]` — 消息后置处理 {#after-completion}

所有插件处理完毕后执行，适合做日志记录、统计等。

```rust
use abi_stable_host_api::InterceptorRequest;

#[after_completion]
fn my_logger(req: &InterceptorRequest) {
    let sender = req.sender_id.as_str();
    let group = req.group_id.as_str();
    let text = req.message_text.as_str();
    eprintln!("[log] sender={sender}, group={group}, text={text:?}");
}
```

### InterceptorRequest

拦截器回调接收的请求上下文：

```rust
#[repr(C)]
pub struct InterceptorRequest {
    pub bot_id: RString,           // Bot 实例 ID
    pub sender_id: RString,        // 发送者 QQ 号
    pub group_id: RString,         // 群号（私聊为空字符串）
    pub message_text: RString,     // 消息纯文本
    pub raw_event_json: RString,   // 原始事件 JSON
    pub sender_nickname: RString,  // 发送者昵称
    pub message_id: RString,       // 消息 ID
    pub timestamp: i64,            // 事件 Unix 时间戳
}
```

::: warning 限制
每个插件模块内最多一个 `#[pre_handle]` 和一个 `#[after_completion]` 函数。
:::

## CommandRequest — 命令请求 {#command-request}

每个命令回调接收一个 `&CommandRequest`，包含完整的请求上下文：

```rust
#[repr(C)]
pub struct CommandRequest {
    pub args: RString,             // 命令参数（空格分隔后的文本）
    pub command_name: RString,     // 匹配到的命令名
    pub sender_id: RString,        // 发送者 QQ 号
    pub group_id: RString,         // 群号（私聊为空字符串）
    pub raw_event_json: RString,   // 原始 OneBot 事件 JSON

    // ── v0.3 新增 ──
    pub sender_nickname: RString,  // 发送者昵称
    pub message_id: RString,       // 消息 ID
    pub timestamp: i64,            // 事件 Unix 时间戳（秒）
}
```

| 字段 | 说明 |
|------|------|
| `args` | 命令参数，如 `/echo hello world` → `"hello world"` |
| `command_name` | 匹配到的命令名（包括别名匹配后的原始名） |
| `sender_id` | 发送者 QQ 号 |
| `group_id` | 群号，私聊时为空字符串 |
| `raw_event_json` | 原始 OneBot 事件 JSON，用于获取更多高级字段 |
| `sender_nickname` | 发送者昵称（v0.3 新增） |
| `message_id` | 消息 ID，可用于引用回复（v0.3 新增） |
| `timestamp` | 事件时间戳，0 表示不可用（v0.3 新增） |

## CommandResponse — 命令响应 {#command-response}

### 快捷方法

```rust
// 纯文本回复
CommandResponse::text("Hello!")

// 忽略事件
CommandResponse::ignore()

// 流式构建富媒体
CommandResponse::builder()
    .text("Hello, ")
    .at("123456")
    .face(1)
    .build()
```

### ReplyBuilder — 流式消息构建

`CommandResponse::builder()` 返回 `ReplyBuilder`，支持链式调用构建富媒体消息：

```rust
let response = CommandResponse::builder()
    .reply("12345")              // 引用回复某条消息
    .at("67890")                 // @某人
    .text(" 你好！这是一条")     // 文本
    .face(1)                     // QQ 表情
    .image_url("https://...")    // 图片（URL）
    .image_base64("iVBOR...")    // 图片（Base64）
    .record("https://...")       // 语音
    .at_all()                    // @全体成员
    .build();
```

| 方法 | 参数 | 说明 |
|------|------|------|
| `.text(text)` | `&str` | 文本段 |
| `.at(user_id)` | `&str` | @某人 |
| `.at_all()` | — | @全体成员 |
| `.face(id)` | `i32` | QQ 表情 |
| `.image_url(url)` | `&str` | 图片（URL） |
| `.image_base64(base64)` | `&str` | 图片（Base64 编码） |
| `.record(file)` | `&str` | 语音（URL 或路径） |
| `.reply(message_id)` | `&str` | 引用回复 |
| `.build()` | — | 构建为 `CommandResponse` |

::: tip 引用回复
结合 `req.message_id` 可以实现引用回复：
```rust
let mut builder = CommandResponse::builder();
let msg_id = req.message_id.as_str();
if !msg_id.is_empty() {
    builder = builder.reply(msg_id);
}
builder.text("收到！").build()
```
:::

### 底层响应（DynamicActionResponse）

如果不使用 `ReplyBuilder`，你也可以直接构造 `DynamicActionResponse`：

```rust
// 纯文本
DynamicActionResponse::text_reply("hello")

// 富媒体（OneBot 消息段 JSON）
DynamicActionResponse::rich_reply(r#"[{"type":"text","data":{"text":"hello"}}]"#)

// 忽略
DynamicActionResponse::ignore()

// 同意请求
DynamicActionResponse::approve("备注")

// 拒绝请求
DynamicActionResponse::reject("理由")
```

## NoticeRequest / NoticeResponse {#notice}

事件回调使用 `NoticeRequest` 和 `NoticeResponse`：

```rust
#[repr(C)]
pub struct NoticeRequest {
    pub route: RString,            // 路由名（如 "GroupPoke"）
    pub raw_event_json: RString,   // 原始 OneBot 事件 JSON
}
```

解析事件详情需要通过 `raw_event_json`：

```rust
#[route(kind = "notice", events = "GroupPoke")]
fn on_poke(req: &NoticeRequest) -> NoticeResponse {
    let raw: serde_json::Value = serde_json::from_str(req.raw_event_json.as_str())
        .unwrap_or_default();

    let target = raw["target_id"].as_i64().unwrap_or(0);
    let sender = raw["user_id"].as_i64().unwrap_or(0);

    NoticeResponse {
        action: DynamicActionResponse::text_reply(
            &format!("{sender} 戳了 {target}！")
        ),
    }
}
```

## 插件配置 {#config}

动态插件可以拥有独立的配置文件。在 `config/plugins/` 目录下创建以插件 ID 命名的 TOML 文件：

```toml
# config/plugins/my-plugin.toml
[database]
url = "sqlite://data.db"
max_connections = 5

[feature]
enable_greeting = true
welcome_message = "欢迎！"
```

配置通过 `#[init]` 钩子的 `config.config_json` 以 JSON 字符串传入：

```rust
#[init]
fn on_init(config: PluginInitConfig) -> PluginInitResult {
    let config_json = config.config_json.as_str();
    if !config_json.is_empty() {
        let cfg: serde_json::Value = serde_json::from_str(config_json).unwrap();
        let db_url = cfg["database"]["url"].as_str().unwrap_or("sqlite://default.db");
        // 初始化数据库...
    }
    PluginInitResult::ok()
}
```

## 完整示例 {#full-example}

```rust
use std::sync::atomic::{AtomicBool, Ordering};

use abi_stable_host_api::{
    CommandRequest, CommandResponse, DynamicActionResponse,
    InterceptorRequest, InterceptorResponse,
    NoticeRequest, NoticeResponse,
    PluginInitConfig, PluginInitResult,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

static INITIALIZED: AtomicBool = AtomicBool::new(false);

#[dynamic_plugin(id = "dynamic-example", version = "0.1.0")]
mod example {
    use super::*;

    // ── 生命周期 ──

    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult {
        eprintln!("[example] init: id={}", config.plugin_id.as_str());
        INITIALIZED.store(true, Ordering::Relaxed);
        PluginInitResult::ok()
    }

    #[shutdown]
    fn on_shutdown() {
        eprintln!("[example] shutdown");
        INITIALIZED.store(false, Ordering::Relaxed);
    }

    // ── 命令 ──

    /// 打招呼 — 演示 ReplyBuilder + 引用回复
    #[command(name = "greet", description = "打招呼", aliases = "hi,hello,你好", category = "示例")]
    fn greet(req: &CommandRequest) -> CommandResponse {
        let name = req.sender_nickname.as_str();
        let display = if name.is_empty() { req.sender_id.as_str() } else { name };

        let mut builder = CommandResponse::builder();

        // 引用回复原消息
        let msg_id = req.message_id.as_str();
        if !msg_id.is_empty() {
            builder = builder.reply(msg_id);
        }

        builder
            .at(req.sender_id.as_str())
            .text(&format!(" 你好 {display}！欢迎使用动态插件~"))
            .face(1)
            .build()
    }

    /// 显示时间 — 演示 timestamp 字段
    #[command(name = "time", description = "显示时间", aliases = "时间", category = "示例")]
    fn time(req: &CommandRequest) -> CommandResponse {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let msg = if req.timestamp > 0 {
            let latency = now.saturating_sub(req.timestamp as u64);
            format!("⏰ 服务器: {now} | 事件: {} | 延迟: {latency}s", req.timestamp)
        } else {
            format!("⏰ 服务器时间: {now}")
        };

        CommandResponse::text(&msg)
    }

    /// 仅群聊命令 — 演示 scope
    #[command(name = "group-hello", description = "仅群聊打招呼", scope = "group")]
    fn group_hello(req: &CommandRequest) -> CommandResponse {
        CommandResponse::builder()
            .at(req.sender_id.as_str())
            .text(" 这条命令只在群聊中可用！")
            .build()
    }

    /// 仅私聊命令 — 演示 scope
    #[command(name = "secret", description = "仅私聊悄悄话", scope = "private")]
    fn secret(_req: &CommandRequest) -> CommandResponse {
        CommandResponse::text("🤫 这是一条仅私聊可见的秘密消息！")
    }

    /// 管理员命令 — 演示 role
    #[command(name = "info", description = "请求详情", role = "admin")]
    fn info(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(&format!(
            "📋 Request Info\n\
             ├ command: {}\n\
             ├ sender: {} ({})\n\
             ├ group: {}\n\
             ├ message_id: {}\n\
             ├ timestamp: {}\n\
             └ initialized: {}",
            req.command_name.as_str(),
            req.sender_id.as_str(),
            req.sender_nickname.as_str(),
            if req.group_id.is_empty() { "<private>" } else { req.group_id.as_str() },
            if req.message_id.is_empty() { "<none>" } else { req.message_id.as_str() },
            req.timestamp,
            INITIALIZED.load(Ordering::Relaxed),
        ))
    }

    // ── 拦截器 ──

    /// 消息预处理 — 记录日志，始终放行
    #[pre_handle]
    fn on_pre_handle(req: &InterceptorRequest) -> InterceptorResponse {
        let sender = req.sender_id.as_str();
        let text = req.message_text.as_str();
        eprintln!("[example] pre_handle: sender={sender}, text={text:?}");
        InterceptorResponse::allow()
    }

    // ── 事件路由 ──

    #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
    fn on_poke(req: &NoticeRequest) -> NoticeResponse {
        let raw: serde_json::Value = serde_json::from_str(req.raw_event_json.as_str())
            .unwrap_or_default();
        let target = raw["target_id"].as_i64().unwrap_or(0);
        let sender = raw["user_id"].as_i64().unwrap_or(0);

        NoticeResponse {
            action: DynamicActionResponse::text_reply(
                &format!("👆 {sender} 戳了 {target}！[{}]", req.route.as_str())
            ),
        }
    }
}
```

## 手动 FFI 写法（不使用宏） {#manual-ffi}

如果你不想使用过程宏，也可以手动编写所有 FFI 导出函数。这种方式更底层，但能完全控制导出行为。

<details>
<summary>展开手动 FFI 示例</summary>

```rust
use abi_stable::std_types::RString;
use abi_stable_host_api::*;

/// 插件描述符 — 唯一必须导出的函数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("my-plugin", "0.1.0")
        .add_command("hello", "向发送者打招呼", "my_plugin_hello")
        .add_command_full(CommandDescriptorEntry {
            name: RString::from("greet"),
            description: RString::from("打招呼（完整版）"),
            callback_symbol: RString::from("my_plugin_greet"),
            aliases: RString::from("hi,你好"),
            category: RString::from("general"),
            required_role: RString::new(),
            scope: RString::from("group"),   // 仅群聊
        })
        .add_route("notice", "GroupPoke", "my_plugin_on_poke")
        .add_interceptor("my_plugin_pre_handle", "")  // 注册拦截器
}

/// 初始化钩子（可选）
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_init(config: PluginInitConfig) -> PluginInitResult {
    eprintln!("plugin init: {}", config.plugin_id.as_str());
    PluginInitResult::ok()
}

/// 关闭钩子（可选）
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_shutdown() {
    eprintln!("plugin shutdown");
}

/// 命令回调
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_hello(req: &CommandRequest) -> CommandResponse {
    CommandResponse::text(&format!("Hello, {}!", req.sender_id))
}

/// 带 ReplyBuilder 的命令回调
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_greet(req: &CommandRequest) -> CommandResponse {
    CommandResponse::builder()
        .at(req.sender_id.as_str())
        .text(" 你好！")
        .face(1)
        .build()
}

/// 事件回调
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_on_poke(req: &NoticeRequest) -> NoticeResponse {
    NoticeResponse {
        action: DynamicActionResponse::text_reply("别戳了！"),
    }
}

/// 拦截器 pre_handle 回调（可选）
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_pre_handle(req: &InterceptorRequest) -> InterceptorResponse {
    eprintln!("pre_handle: sender={}", req.sender_id.as_str());
    InterceptorResponse::allow()
}
```

</details>

## 运行时管理 {#runtime}

| 命令 | 说明 |
|------|------|
| `/plugins reload` | 热重载：重新扫描 `plugin_bin_dir`，卸载旧库，加载新库 |
| `/plugins enable <id>` | 启用插件 |
| `/plugins disable <id>` | 禁用插件（持久化到 `plugin-state.toml`） |
| `/dynamic-errors` | 查看动态插件健康状态 |
| `/dynamic-errors clear` | 清除错误计数，解除隔离 |

## 熔断器机制 {#circuit-breaker}

动态插件内置熔断器保护，防止有问题的插件影响整体稳定性：

```
执行成功 → 重置失败计数
执行失败 → 失败计数 +1
            ↓
      连续 3 次失败
            ↓
    自动隔离 60 秒
            ↓
    隔离期间所有请求直接返回错误
            ↓
    60 秒后自动恢复
```

使用 `/dynamic-errors` 查看各插件的健康状态，`/dynamic-errors clear` 可以手动重置错误计数。

## 注意事项

::: warning 重要限制
1. **同步执行** — 动态插件回调是同步的 `extern "C"` 函数，不支持 `async/await`
2. **C ABI** — 所有导出函数必须标记 `#[unsafe(no_mangle)]`（Rust 2024 Edition 语法）
3. **ABI 稳定** — 使用 `abi_stable` crate 提供的类型（`RString`、`RVec`），不能直接传递 Rust 标准库类型跨 FFI 边界
4. **独立编译** — 动态插件不属于主工作空间，Cargo.toml 中必须有空的 `[workspace]` 表
5. **每模块限制** — 最多一个 `#[init]`、一个 `#[shutdown]`、一个 `#[pre_handle]`、一个 `#[after_completion]` 函数
:::
