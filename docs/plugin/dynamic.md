# 动态插件开发

除了与框架一同编译的**静态插件**，QimenBot 还支持**动态插件**——编译为独立的动态库（`.so` / `.dll` / `.dylib`），运行时通过 `dlopen` 加载。

## 两种插件对比

| 特性 | 静态插件 | 动态插件 |
|------|---------|---------|
| 编译方式 | 与框架一同编译 | 独立编译为动态库 |
| API 访问 | 完整（async、OneBotActionClient 等） | FFI 接口（同步、C ABI） |
| 消息类型 | 完整 Message（富媒体） | 纯文本 + JSON 段 |
| 热重载 | 需要重启进程 | `/plugins reload` 即可 |
| 适用场景 | 核心功能、需要异步 API | 第三方扩展、快速迭代 |

::: tip 如何选择？
- 如果你是框架开发者，或者需要使用异步 API → 选择**静态插件**
- 如果你是第三方开发者，需要快速迭代和热重载 → 选择**动态插件**
:::

## 快速开始

### 第 1 步：创建项目

```bash
# 在 plugins/ 目录下创建新项目
cargo new --lib plugins/my-plugin
cd plugins/my-plugin
```

### 第 2 步：配置 Cargo.toml

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

::: warning 重要配置
- `crate-type = ["cdylib"]` — 必须设为 cdylib 才能编译为动态库
- `[workspace]` — 空的 workspace 表，使这个 crate 不属于主工作空间
:::

### 第 3 步：编写插件

```rust
use abi_stable::std_types::RString;
use abi_stable_host_api::*;

/// 插件描述符 — 唯一必须导出的函数
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("my-plugin", "0.1.0")
        .add_command("hello", "向发送者打招呼", "my_plugin_hello")
}

/// 命令回调
#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_hello(req: &CommandRequest) -> CommandResponse {
    CommandResponse {
        action: DynamicActionResponse::text_reply(
            &format!("Hello, {}!", req.sender_id)
        ),
    }
}
```

### 第 4 步：编译

```bash
cd plugins/my-plugin
cargo build --release
```

编译产物位于 `target/release/` 目录：

| 平台 | 文件名 |
|------|--------|
| Linux | `libmy_dynamic_plugin.so` |
| macOS | `libmy_dynamic_plugin.dylib` |
| Windows | `my_dynamic_plugin.dll` |

### 第 5 步：部署

将动态库复制到 `plugins/bin/` 目录：

```bash
# Linux
cp target/release/libmy_dynamic_plugin.so ../../plugins/bin/

# macOS
cp target/release/libmy_dynamic_plugin.dylib ../../plugins/bin/

# Windows
cp target/release/my_dynamic_plugin.dll ../../plugins/bin/
```

### 第 6 步：加载

在 Bot 中发送 `/plugins reload`，无需重启即可加载新插件。

## FFI 接口详解

### PluginDescriptor — 插件描述符

每个动态插件必须导出 `qimen_plugin_descriptor` 函数，返回 `PluginDescriptor`：

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("plugin-id", "0.1.0")
        // 注册命令
        .add_command("cmd1", "命令描述", "callback_symbol_1")
        .add_command("cmd2", "命令描述", "callback_symbol_2")
        // 注册事件路由
        .add_route("notice", "GroupPoke,PrivatePoke", "on_poke_symbol")
        .add_route("request", "Friend", "on_friend_symbol")
        .add_route("meta", "Heartbeat", "on_heartbeat_symbol")
}
```

#### add_command — 注册命令

```rust
// 简单方式
.add_command("name", "description", "callback_symbol")

// 完整方式（支持别名、分类、权限）
.add_command_full(CommandDescriptorEntry {
    name: RString::from("greet"),
    description: RString::from("向发送者打招呼"),
    callback_symbol: RString::from("my_greet"),
    aliases: RString::from("hi,hello"),       // 逗号分隔的别名
    category: RString::from("general"),       // 分类
    required_role: RString::new(),            // 空 = 任何人可用
})
```

| `required_role` 值 | 说明 |
|--------------------|------|
| `""` (空) | 任何人可用 |
| `"admin"` | 仅管理员 |
| `"owner"` | 仅所有者 |

#### add_route — 注册事件路由

```rust
.add_route("notice", "GroupPoke,PrivatePoke", "on_poke_symbol")
//         ↑ 类型     ↑ 路由名（逗号分隔）       ↑ 回调符号名
```

| 类型 | 可用路由名 |
|------|----------|
| `"notice"` | 参见 [通知路由列表](/plugin/events#通知路由notice) |
| `"request"` | `Friend`, `GroupAdd`, `GroupInvite` |
| `"meta"` | `Heartbeat`, `LifecycleConnect` 等 |

### CommandRequest — 命令请求

```rust
pub struct CommandRequest {
    pub args: RString,            // 命令参数（空格分隔）
    pub command_name: RString,    // 匹配到的命令名
    pub sender_id: RString,       // 发送者 QQ 号
    pub group_id: RString,        // 群号（私聊为空）
    pub raw_event_json: RString,  // 原始 OneBot 事件 JSON
}
```

### NoticeRequest — 事件请求

```rust
pub struct NoticeRequest {
    pub route: RString,           // 路由名（如 "GroupPoke"）
    pub raw_event_json: RString,  // 原始 OneBot 事件 JSON
}
```

### 响应类型

```rust
// 纯文本回复
DynamicActionResponse::text_reply("hello")

// 富媒体回复（OneBot 消息段 JSON 格式）
DynamicActionResponse::rich_reply(r#"[
    {"type":"text","data":{"text":"hello "}},
    {"type":"face","data":{"id":"1"}}
]"#)

// 忽略事件
DynamicActionResponse::ignore()

// 同意好友/群请求
DynamicActionResponse::approve("备注")

// 拒绝好友/群请求
DynamicActionResponse::reject("理由")
```

## 富媒体消息

使用 `rich_reply` 发送富媒体消息，格式与 OneBot 11 消息段一致：

```rust
let segments = serde_json::json!([
    { "type": "text",  "data": { "text": "Hello " } },
    { "type": "at",    "data": { "qq": "123456" } },
    { "type": "face",  "data": { "id": "1" } },
    { "type": "image", "data": { "file": "https://example.com/img.png" } }
]);

CommandResponse {
    action: DynamicActionResponse::rich_reply(&segments.to_string()),
}
```

### 常用消息段类型

| type | data 字段 | 说明 |
|------|----------|------|
| `text` | `text` | 纯文本 |
| `at` | `qq` | @某人 |
| `face` | `id` | QQ 表情 |
| `image` | `file` | 图片（URL 或路径） |
| `record` | `file` | 语音 |
| `video` | `file` | 视频 |
| `share` | `url`, `title` | 链接分享 |
| `reply` | `id` | 引用回复 |

## 完整示例

```rust
use abi_stable::std_types::RString;
use abi_stable_host_api::*;

// ── 插件描述符 ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("dynamic-example", "0.1.0")
        .add_command_full(CommandDescriptorEntry {
            name: RString::from("greet"),
            description: RString::from("向发送者打招呼"),
            callback_symbol: RString::from("dynamic_example_greet"),
            aliases: RString::from("hi,hello"),
            category: RString::from("general"),
            required_role: RString::new(),
        })
        .add_command("time", "显示当前 Unix 时间戳", "dynamic_example_time")
        .add_route("notice", "GroupPoke,PrivatePoke", "dynamic_example_on_poke")
}

// ── 命令回调 ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_greet(req: &CommandRequest) -> CommandResponse {
    let sender = req.sender_id.as_str();
    let args = req.args.as_str().trim();

    let greeting = if args.is_empty() {
        format!("Hello, {sender}! 欢迎使用 QimenBot！")
    } else {
        format!("{sender}: {args}")
    };

    let segments = serde_json::json!([
        { "type": "text", "data": { "text": greeting } },
        { "type": "face", "data": { "id": "1" } }
    ]);

    CommandResponse {
        action: DynamicActionResponse::rich_reply(&segments.to_string()),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_time(_req: &CommandRequest) -> CommandResponse {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    CommandResponse {
        action: DynamicActionResponse::text_reply(
            &format!("当前 Unix 时间戳: {now}")
        ),
    }
}

// ── 事件回调 ──

#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_on_poke(req: &NoticeRequest) -> NoticeResponse {
    let target_id = serde_json::from_str::<serde_json::Value>(req.raw_event_json.as_str())
        .ok()
        .and_then(|v| v.get("target_id")?.as_i64())
        .unwrap_or(0);

    let route = req.route.as_str();
    let text = format!("检测到戳一戳事件 [{route}]！目标: {target_id}");

    NoticeResponse {
        action: DynamicActionResponse::text_reply(&text),
    }
}
```

## 运行时管理

| 命令 | 说明 |
|------|------|
| `/plugins reload` | 热重载：重新扫描 `plugin_bin_dir`，卸载旧库，加载新库 |
| `/plugins enable <id>` | 启用插件 |
| `/plugins disable <id>` | 禁用插件（持久化到 `plugin-state.toml`） |
| `/dynamic-errors` | 查看动态插件健康状态 |
| `/dynamic-errors clear` | 清除错误计数，解除隔离 |

## 熔断器机制

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
2. **C ABI** — 所有导出函数必须标记 `#[unsafe(no_mangle)]`，Rust 2024 Edition 要求写 `#[unsafe(no_mangle)]` 而不是 `#[no_mangle]`
3. **ABI 稳定** — 使用 `abi_stable` crate 提供的类型（`RString`、`RVec`），不能直接传递 Rust 标准库类型跨 FFI 边界
4. **独立编译** — 动态插件不属于主工作空间，Cargo.toml 中必须有空的 `[workspace]` 表
:::
