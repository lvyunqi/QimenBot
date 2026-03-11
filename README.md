<div align="center">

<img src="logo.jpg" width="200" alt="QimenBot Logo">

# QimenBot

_✨ 基于 Rust 的高性能多协议 Bot 框架 ✨_

[![License](https://img.shields.io/github/license/lvyunqi/QimenBot?style=flat-square)](https://github.com/lvyunqi/QimenBot/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![OneBot 11](https://img.shields.io/badge/OneBot-11-black?style=flat-square)](https://github.com/botuniverse/onebot-11)

**简体中文** | [English](README_EN.md) | [日本語](README_JA.md)

</div>

---

QimenBot 是一个用 Rust 编写的模块化、可扩展的聊天机器人框架。它将**可复用的框架层**与**参考 Host 实现**分离，既可以直接部署官方 Host，也可以基于框架层构建自己的 Bot 平台。

## 特性

- **多协议支持** — OneBot 11（生产就绪）、OneBot 12 / Satori（预留扩展点）
- **多传输模式** — 正向 WebSocket、反向 WebSocket、HTTP API、HTTP POST
- **声明式插件开发** — 通过 `#[module]` / `#[commands]` / `#[notice]` 宏，最少 ~7 行即可完成一个插件
- **拦截器链** — `pre_handle` / `after_completion`，支持黑名单、权限校验、快捷指令改写等
- **灵活的命令系统** — 别名、示例、分类、权限等级、消息过滤器，自动生成 `/help`
- **系统事件路由** — 群通知、好友请求、Meta 事件，全部通过注解路由分发
- **运行时保护** — 令牌桶限流、消息去重、群事件过滤、插件 ACL
- **动态插件** — 支持通过 `dlopen` 加载 ABI 稳定的动态库插件
- **请求自动化** — 好友/群邀请的自动审批，基于白名单、黑名单、关键词过滤
- **完善的 OneBot 11 API** — 消息、群管理、文件、频道、表情回应等 40+ 操作封装

## 架构

```
┌─────────────────────────────────────────────────────┐
│                    应用层 (apps/)                      │
│         qimenbotd (守护进程)    qimenctl (CLI)         │
├─────────────────────────────────────────────────────┤
│                Official Host 层                       │
│   qimen-official-host · qimen-config · observability  │
├─────────────────────────────────────────────────────┤
│                  Framework 层 (可复用)                  │
│  runtime · plugin-api · plugin-host · message         │
│  protocol-core · transport-core · command-registry    │
├─────────────────────────────────────────────────────┤
│                   适配器 & 传输                        │
│  adapter-onebot11 · transport-ws · transport-http     │
├─────────────────────────────────────────────────────┤
│                   内置模块                             │
│  mod-command · mod-admin · mod-scheduler · mod-bridge  │
└─────────────────────────────────────────────────────┘
```

## 快速开始

### 环境要求

- Rust 1.89+（2024 Edition）
- 一个 OneBot 11 实现（如 [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core)、[NapCat](https://github.com/NapNeko/NapCatQQ) 等）

### 构建 & 运行

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot

# 编辑配置（修改 endpoint、owners 等）
vim config/base.toml

# 运行
cargo run
```

## 配置详解

框架启动时只读取一个配置文件：**`config/base.toml`**。所有全局设置和 Bot 实例都在这一个文件中定义。

> `config/bots/` 目录下的 `.toml` 文件**不会被框架自动加载**，它们仅作为多 Bot 场景下的管理参考/备份模板。

### 配置文件结构总览

```
config/base.toml           ← 框架唯一读取的配置文件
│
├── [runtime]               全局运行时设置
├── [observability]         日志与监控
├── [official_host]         模块加载（全局共享）
│
├── [[bots]]                Bot 实例 1（每个 Bot 独立配置）
├── [[bots]]                Bot 实例 2
└── [[bots]]                Bot 实例 3 ...
```

关键概念：**`[official_host]` 是全局的，`[[bots]]` 是每个 Bot 实例独立的**。

```
┌────────────────────────────────────────────────────┐
│  [official_host]（全局）                              │
│  决定框架加载哪些模块和插件                              │
│  所有 Bot 共享同一套模块代码                             │
├────────────────────────────────────────────────────┤
│  [[bots]] qq-main        │  [[bots]] qq-backup     │
│  ├─ 连接地址 endpoint     │  ├─ 监听地址 bind        │
│  ├─ enabled_modules      │  ├─ enabled_modules     │
│  ├─ owners / admins      │  ├─ owners / admins     │
│  ├─ 好友请求策略           │  └─ ...                 │
│  ├─ 群邀请策略             │                         │
│  ├─ 戳一戳回复             │                         │
│  └─ 限流器 limiter        │                         │
└────────────────────────────────────────────────────┘
```

- `[official_host]` 控制"加载哪些模块到内存"
- `[[bots]].enabled_modules` 控制"这个 Bot 实际启用哪些模块"
- 所以你可以全局加载 10 个模块，但某个 Bot 只启用其中 3 个

---

### `[runtime]` — 运行时

```toml
[runtime]
env = "dev"                    # 运行环境："dev" 或 "prod"
shutdown_timeout_secs = 15     # 关闭信号后等待任务完成的超时（秒），超时强制退出
task_grace_secs = 5            # 后台任务（定时器、重连等）的优雅退出等待（秒）
```

### `[observability]` — 日志与监控

```toml
[observability]
level = "info"                 # 日志级别：trace / debug / info / warn / error
json_logs = false              # true = JSON 格式输出（适合 ELK/Loki 采集）
metrics_bind = "127.0.0.1:9090"  # Metrics 暴露地址（预留）
```

### `[official_host]` — 全局模块加载

这个区块决定框架启动时**加载哪些模块到内存**。它是全局的，所有 Bot 共享。

```toml
[official_host]
# 内置模块（框架自带的核心功能）
# 可选值：
#   "command"   — 命令系统（/ping、/echo、/help 等）
#   "admin"     — 管理模块（权限管理、插件管理）
#   "scheduler" — 定时任务调度器
#   "bridge"    — 消息桥接（跨群/跨bot转发）
builtin_modules = ["command", "admin", "scheduler", "bridge"]

# 第三方插件模块（填写 #[module(id = "xxx")] 中的 id）
# 示例插件可用 id：
#   "example-plugin"  — 基础命令（向后兼容别名）
#   "example-basic"   — 基础命令（ping、echo、whoami、ban、stop）
#   "example-message" — 消息构建（rich、parse、card、keyboard）
#   "example-events"  — 事件处理（戳一戳、入群欢迎、好友请求）
plugin_modules = ["example-plugin"]

# 插件启用/禁用状态的持久化文件
# 用 /plugins 命令修改的状态会保存在这里，重启后恢复
plugin_state_path = "config/plugin-state.toml"

# 动态插件（.so/.dll/.dylib）的扫描目录
plugin_bin_dir = "plugins/bin"
```

### `[[bots]]` — Bot 实例配置

每个 `[[bots]]` 块定义一个独立的 Bot 实例。可以配置多个。每个 Bot 有自己的连接地址、权限、审批策略等——**互不影响**。

#### 连接与身份

```toml
[[bots]]
id = "qq-main"                 # Bot 唯一标识（不可重复）
protocol = "onebot11"          # 通信协议：onebot11 / onebot12 / satori
transport = "ws-forward"       # 传输方式（见下表）
endpoint = "ws://127.0.0.1:3001"  # ws-forward 时填连接地址
# bind = "0.0.0.0:6701"        # ws-reverse 时填监听地址
# path = "/onebot/reverse"     # ws-reverse 时填路径
# access_token = "${QQ_TOKEN}" # 连接鉴权 Token（支持环境变量）
enabled = true                 # 是否启用（false 则跳过）
```

**传输方式说明：**

| transport | 方向 | 必填字段 | 说明 |
|-----------|------|---------|------|
| `ws-forward` | 框架 → OneBot | `endpoint` | 框架主动连接 OneBot 实现端的 WebSocket |
| `ws-reverse` | OneBot → 框架 | `bind` + `path` | 框架监听，OneBot 实现端主动连接过来 |
| `http` | 双向 HTTP | `endpoint` | HTTP API + HTTP POST |

#### 模块与权限

```toml
# 此 Bot 启用的模块（从 official_host 已加载的模块中选择）
# 留空 = 使用 builtin_modules 全部
enabled_modules = ["command", "admin", "scheduler"]

# 所有者 ID 列表（最高权限：重启、插件管理、所有命令）
owners = ["123456"]

# 管理员 ID 列表（管理权限：禁言、踢人等 role = "admin" 的命令）
admins = ["789012"]
```

> `[official_host].builtin_modules` 和 `[[bots]].enabled_modules` 的关系：
> - `builtin_modules` 决定"框架加载哪些模块代码"（全局）
> - `enabled_modules` 决定"这个 Bot 实际使用哪些模块"（每 Bot 独立）
> - `enabled_modules` 里的模块必须在 `builtin_modules` 或 `plugin_modules` 中已声明

#### 好友请求自动审批

```toml
# 总开关：是否自动同意所有好友请求
auto_approve_friend_requests = false

# 用户白名单：这些用户的请求始终自动同意（不受总开关影响）
auto_approve_friend_request_user_whitelist = ["111111", "222222"]

# 用户黑名单：这些用户的请求始终自动拒绝（优先级高于白名单）
auto_approve_friend_request_user_blacklist = []

# 验证消息关键词白名单：验证消息中包含这些关键词则自动同意
auto_approve_friend_request_comment_keywords = ["来自群"]

# 验证消息关键词黑名单：验证消息中包含这些关键词则自动拒绝
auto_reject_friend_request_comment_keywords = ["广告"]

# 自动同意时设置的好友备注
auto_approve_friend_request_remark = ""
```

#### 群邀请自动审批

```toml
# 总开关：是否自动同意所有群邀请
auto_approve_group_invites = false

# 邀请者用户白名单
auto_approve_group_invite_user_whitelist = []

# 邀请者用户黑名单
auto_approve_group_invite_user_blacklist = []

# 群号白名单：被邀请加入这些群时自动同意
auto_approve_group_invite_group_whitelist = ["12345678"]

# 群号黑名单：被邀请加入这些群时自动拒绝
auto_approve_group_invite_group_blacklist = []

# 邀请验证消息关键词白名单
auto_approve_group_invite_comment_keywords = []

# 邀请验证消息关键词黑名单
auto_reject_group_invite_comment_keywords = []

# 自动拒绝时的拒绝理由
auto_reject_group_invite_reason = ""
```

#### 戳一戳自动回复

```toml
# 是否启用戳一戳自动回复（被戳时自动回复一条消息）
auto_reply_poke_enabled = true

# 回复内容
auto_reply_poke_message = "别戳了，我在忙。"
```

#### 令牌桶限流器

```toml
# 针对此 Bot 的消息限流（防止刷屏）
[bots.limiter]
enable = false           # 是否启用限流
rate = 5.0               # 每秒恢复的令牌数（默认 5.0）
capacity = 10            # 令牌桶容量（默认 10，即最多突发处理 10 条）
timeout_secs = 0         # 等待令牌的超时（0 = 不等待，直接丢弃）
```

---

### 环境变量

配置值支持 `${ENV_VAR}` 格式的环境变量占位符，框架启动时自动替换：

```toml
access_token = "${QQ_TOKEN}"        # 从环境变量 QQ_TOKEN 读取
endpoint = "${ONEBOT_WS_ENDPOINT}"  # 从环境变量读取连接地址
```

如果环境变量不存在，会被替换为空字符串。

### 环境覆盖文件

`config/dev.toml` 和 `config/prod.toml` 是预设的环境差异化配置参考。当前框架只读取 `config/base.toml`，这两个文件用于手动切换不同环境时参考或复制。

### 完整配置示例

```toml
[runtime]
env = "dev"
shutdown_timeout_secs = 15
task_grace_secs = 5

[observability]
level = "info"
json_logs = false
metrics_bind = "127.0.0.1:9090"

[official_host]
builtin_modules = ["command", "admin", "scheduler"]
plugin_modules  = ["example-plugin"]

[[bots]]
id        = "qq-main"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"
enabled   = true
owners    = ["123456"]
auto_reply_poke_enabled = true
auto_reply_poke_message = "别戳了，我在忙。"
```

## 插件开发

QimenBot 通过过程宏将插件开发降至最简。完整示例见 [`plugins/qimen-plugin-example/`](plugins/qimen-plugin-example/)。

### 最小示例

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("Say hello")]
    async fn hello(&self) -> &str {
        "Hello from QimenBot!"
    }
}
```

只需要这么多代码，就完成了一个可用的插件。下面解释每个宏的作用。

### `#[module]` — 声明模块

标记在 `impl` 块上面，告诉框架"这是一个插件模块"。宏会自动帮你创建 `struct MyPlugin;` 结构体（不需要手动写），并生成 `Module` trait 实现。

```rust
#[module(
    id = "my-plugin",             // 必填，模块唯一标识
    version = "0.1.0",            // 可选，默认 "0.1.0"
    name = "My Plugin",           // 可选，默认取结构体名
    description = "...",           // 可选
    interceptors = [MyInterceptor] // 可选，拦截器列表
)]
```

### `#[commands]` — 扫描命令和事件

紧跟在 `#[module]` 下面。扫描 `impl` 块里所有带 `#[command]`/`#[notice]`/`#[request]`/`#[meta]` 的方法，自动生成 `CommandPlugin` 和 `SystemPlugin` 实现。

### `#[command]` — 定义聊天命令

```rust
#[command(
    "Echo back the given text",    // 必填，命令描述
    aliases = ["e"],               // 可选，别名列表
    examples = ["/echo hello"],    // 可选，使用示例
    category = "examples",         // 可选，默认 "general"
    role = "admin",                // 可选，"admin" 或 "owner"
    hidden,                        // 可选，隐藏命令
)]
async fn echo(&self, args: Vec<String>) -> Message { ... }
```

**命令名自动推导**：如果你没写 `name = "xxx"`，宏会拿**函数名**当命令名，并把下划线 `_` 替换成连字符 `-`：

| 函数名 | 推导出的命令名 | 用户输入 |
|--------|---------------|---------|
| `ping` | `"ping"` | `/ping` |
| `echo` | `"echo"` | `/echo hello` |
| `group_info` | `"group-info"` | `/group-info` |

**方法签名灵活组合**：宏自动检测你的参数，决定注入什么：

```rust
// 无参数 — 最简单
async fn ping(&self) -> Message { ... }

// 只要参数 — 框架自动按空格拆分命令后的文字
async fn echo(&self, args: Vec<String>) -> Message { ... }

// 只要上下文 — 获取发送者、群号等信息
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal { ... }

// 上下文 + 参数（ctx 必须在前）
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal { ... }
```

### `#[notice]` / `#[request]` / `#[meta]` — 系统事件路由

```rust
// 通知事件（可同时监听多个类型）
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self) -> Message { ... }

// 请求事件
#[request(Friend)]
async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

// 元事件
#[meta(Heartbeat)]
async fn on_heartbeat(&self) -> SystemPluginSignal { ... }
```

### 返回值自动包装

方法可以返回以下任意类型，框架自动转换为信号：

| 返回类型 | 行为 |
|---------|------|
| `Message` | 回复该消息 |
| `String` / `&str` | 回复文本消息 |
| `CommandPluginSignal` | 完全控制（Reply / Continue / Block / Ignore） |
| `Result<T, E>` | Ok → 正常处理，Err → 回复 `"Error: {e}"` |

### 拦截器

在事件到达插件之前/之后进行预处理：

```rust
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        // 返回 false 拦截事件，true 放行
        true
    }

    async fn after_completion(&self, _bot_id: &str, _event: &NormalizedEvent) {
        // 所有插件处理完毕后执行（逆序）
    }
}

// 在 #[module] 中注册拦截器
#[module(id = "my-plugin", interceptors = [MyInterceptor])]
#[commands]
impl MyPlugin { /* ... */ }
```

### 宏的完整文档

宏系统的详细原理说明（包括宏展开后的完整代码对比）请参阅[示例插件文档](plugins/qimen-plugin-example/README.md#宏系统详解)。

### 事件处理流程

```
收到事件
  → 系统事件分发（notice / request / meta）
  → 消息去重
  → 群事件过滤
  → 令牌桶限流
  → 拦截器链 pre_handle
  → 权限解析
  → 命令匹配 & 插件分发
  → 拦截器链 after_completion
```

## 动态插件开发

除了与框架一同编译的**静态插件**（`#[module]` 宏），QimenBot 还支持**动态插件**——编译为 `.so`（Linux）/ `.dll`（Windows）/ `.dylib`（macOS）的独立库，运行时通过 `dlopen` 加载。

### 两种插件模式对比

| 特性 | 静态插件 | 动态插件 |
|------|---------|---------|
| 编译方式 | 与框架一同编译 | 独立编译为动态库 |
| API 访问 | 完整（async、OneBotActionClient 等） | FFI 接口（同步、C ABI） |
| 消息类型 | 完整 Message（富媒体） | 纯文本 + JSON 段（v0.2 支持富媒体） |
| 热重载 | 需要重启进程 | `/plugins reload` 即可 |
| 适用场景 | 核心功能、需要异步 API | 第三方扩展、快速迭代 |

### 最小动态插件

```rust
use abi_stable::std_types::RString;
use abi_stable_host_api::*;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("my-dynamic-plugin", "0.1.0")
        .add_command("hello", "Say hello", "my_plugin_hello")
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_plugin_hello(req: &CommandRequest) -> CommandResponse {
    CommandResponse {
        action: DynamicActionResponse::text_reply(&format!(
            "Hello, {}!", req.sender_id
        )),
    }
}
```

### 构建 & 部署

```bash
# 1. 在 plugins/ 下创建独立 crate（不加入 workspace）
cargo new --lib plugins/my-plugin
# Cargo.toml 中设置 crate-type = ["cdylib"]，加 [workspace] 空表

# 2. 编译
cd plugins/my-plugin
cargo build --release

# 3. 部署：复制动态库到 plugin_bin_dir
cp target/release/libmy_plugin.so ../../plugins/bin/
# Windows: cp target/release/my_plugin.dll ../../plugins/bin/
# macOS:   cp target/release/libmy_plugin.dylib ../../plugins/bin/

# 4. 在 Bot 中执行 /plugins reload 热重载
```

### FFI 接口详解

#### `PluginDescriptor` — 插件描述符

宿主通过 `qimen_plugin_descriptor()` 符号获取插件元数据：

```rust
// v0.2：支持多命令 + 多事件路由
PluginDescriptor::new("plugin-id", "0.1.0")
    .add_command("cmd1", "描述", "callback_symbol_1")
    .add_command("cmd2", "描述", "callback_symbol_2")
    .add_route("notice", "GroupPoke,PrivatePoke", "on_poke_symbol")
    .add_route("request", "Friend", "on_friend_symbol")
    .add_route("meta", "Heartbeat", "on_heartbeat_symbol")
```

#### `CommandRequest` — 命令请求

```rust
pub struct CommandRequest {
    pub args: RString,           // 命令参数（空格分隔）
    pub command_name: RString,   // 匹配到的命令名
    pub sender_id: RString,      // 发送者用户 ID
    pub group_id: RString,       // 群 ID（私聊为空）
    pub raw_event_json: RString, // 原始 OneBot 事件 JSON
}
```

#### 响应类型

```rust
// 纯文本回复
DynamicActionResponse::text_reply("hello")

// 富媒体回复（JSON 段格式，与 OneBot11 相同）
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

### 运行时管理

| 命令 | 说明 |
|------|------|
| `/plugins reload` | 热重载：重新扫描 plugin_bin_dir，卸载旧库，加载新库 |
| `/plugins enable <id>` | 启用插件（动态/静态均可） |
| `/plugins disable <id>` | 禁用插件（持久化到 plugin-state.toml） |
| `/dynamic-errors` | 查看动态插件健康状态（熔断器、错误历史） |
| `/dynamic-errors clear` | 清除错误计数，解除隔离 |

### 熔断器机制

动态插件内置熔断器保护：

- 连续 3 次失败 → 插件自动隔离 60 秒
- 隔离期间所有请求直接返回错误
- 成功执行后自动重置失败计数
- `/dynamic-errors clear` 手动重置

完整示例见 [`plugins/qimen-dynamic-plugin-example/`](plugins/qimen-dynamic-plugin-example/)。

## 内置命令

| 命令 | 说明 |
|------|------|
| `ping` / `/ping` | 返回 pong |
| `echo <text>` / `/echo <text>` | 回显文本 |
| `status` / `/status` | 运行时状态 |
| `help` / `/help` | 自动生成的帮助信息 |
| `plugins` / `/plugins` | 已加载插件列表 |
| `plugins reload` | 热重载动态插件 |
| `dynamic-errors` | 动态插件健康状态 |

命令触发方式：私聊直发、`/前缀`、`@bot 提及`、回复触发。

## 项目结构

```
QimenBot/
├── apps/
│   ├── qimenbotd/           # Bot 守护进程
│   └── qimenctl/            # CLI 管理工具
├── crates/
│   ├── qimen-plugin-api/    # 插件 API（CommandPlugin, SystemPlugin, Module）
│   ├── qimen-plugin-derive/ # 过程宏（#[module], #[commands], #[command]...）
│   ├── qimen-runtime/       # 事件分发、插件编排、拦截器
│   ├── qimen-message/       # 消息模型（Segment, MessageBuilder）
│   ├── qimen-adapter-onebot11/ # OneBot 11 适配器
│   ├── qimen-transport-ws/  # WebSocket 传输（TLS、自动重连）
│   ├── qimen-transport-http/# HTTP 传输
│   ├── qimen-mod-command/   # 命令检测与匹配
│   ├── qimen-mod-admin/     # 权限管理
│   ├── qimen-mod-scheduler/ # Cron 定时任务
│   └── ...                  # 更多核心 crate
├── plugins/
│   ├── qimen-plugin-example/        # 静态插件示例（含详细文档）
│   └── qimen-dynamic-plugin-example/# 动态插件示例（独立编译）
└── config/
    ├── base.toml            # 主配置（框架唯一读取的文件）
    ├── dev.toml             # 开发环境参考配置
    ├── prod.toml            # 生产环境参考配置
    ├── plugin-state.toml    # 插件启用/禁用状态（自动管理）
    └── bots/                # Bot 独立配置参考（不会被自动加载）
        ├── qq-main.toml
        └── qq-backup.toml
```

## 协议支持

| 协议 | 状态 | 传输模式 |
|------|------|---------|
| OneBot 11 | ✅ 生产就绪 | WS 正向、WS 反向、HTTP API、HTTP POST |
| OneBot 12 | 🔲 计划中 | — |
| Satori | 🔲 计划中 | — |

## 致谢

QimenBot 的设计参考了以下优秀项目：

- [Shiro](https://github.com/MisakaTAT/Shiro) — 基于 Java 的 OneBot 框架，拦截器与插件模型的灵感来源
- [Kovi](https://github.com/ThriceCola/Kovi) — Rust OneBot 框架，简洁 API 设计的参考

## 许可证

[MIT](LICENSE)
