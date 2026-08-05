<div align="center">

<img src="logo.jpg" width="200" alt="QimenBot Logo">

# QimenBot

_基于 Rust 的多协议 Bot 框架_

[![License](https://img.shields.io/github/license/lvyunqi/QimenBot?style=flat-square)](https://github.com/lvyunqi/QimenBot/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![OneBot 11](https://img.shields.io/badge/OneBot-11-black?style=flat-square)](https://github.com/botuniverse/onebot-11)
[![QQ Official](https://img.shields.io/badge/QQ_Official-Bot-blue?style=flat-square)](docs/guide/qq-official-quickstart.md)

**QQ 交流群：835684778** · [点击加入群聊【QimenBot】](https://qun.qq.com/universal-share/share?ac=1&authKey=0sFE1a6DbXGo70vp3VpylxRQ8AmXY%2BgpIAbrB4Cgf9qjT634oSVcrHDWptDNP3%2Fq&busi_data=eyJncm91cENvZGUiOiI4MzU2ODQ3NzgiLCJ0b2tlbiI6IitmMTBOWS96UXQ2Tk9nakgrOWZFMElPL0VXcFJnNmp0c0NSS0tpK25wY24xNEpGV2MvdjY1c2VBL3ArM09TQngiLCJ1aW4iOiI0MzQ2NTgxOTgifQ%3D%3D&data=EJZhsrc7rxEPVPxGeDybFi7TfocR3lNIFijyePfdpsQTTzNNnqoiMvuahA0t8HoN8DVZR9aKBCKcTxDKmOb8IQ&svctype=4&tempid=h5_group_info)

**简体中文** | [English](README_EN.md) | [日本語](README_JA.md)

</div>

---

QimenBot 是一个用 Rust 编写的模块化、可扩展的聊天机器人框架。它将**可复用的框架层**与**参考 Host 实现**分离，既可以直接部署官方 Host，也可以基于框架层构建自己的 Bot 平台。

## 特性

- **多协议支持** — OneBot 11、官方 QQ Bot，OneBot 12 / Satori 预留扩展点
- **多传输模式** — 正向 WebSocket、反向 WebSocket、HTTP API、HTTP POST、官方 Gateway + OpenAPI
- **声明式插件开发** — `#[module]` / `#[commands]` / `#[notice]` 宏生成注册与路由代码
- **拦截器链** — `pre_handle` / `after_completion`，支持黑名单、权限校验、快捷指令改写等
- **命令系统** — 别名、示例、分类、权限等级、消息过滤器，自动生成 `/help`
- **系统事件路由** — 群通知、好友请求和 Meta 事件通过注解路由分发
- **运行时保护** — 令牌桶限流、消息去重、群事件过滤、插件 ACL
- **动态插件** — `#[dynamic_plugin]` 宏声明式开发，`dlopen` 热重载，ABI 稳定，API 0.6 支持 Schema 在线配置
- **GitHub 插件商城** — 静态目录、SemVer 兼容选择、SHA256 安装锁、热更新失败回滚
- **请求自动化** — 好友/群邀请的自动审批，基于白名单、黑名单、关键词过滤
- **完善的 OneBot 11 API** — 消息、群管理、文件、频道、表情回应等 40+ 操作封装
- **官方 QQ Bot 接入** — 支持群 @ 与全量群消息、C2C、频道和 DMS，包含 Markdown / Keyboard、公网 URL 与 Base64 本地媒体上传、撤回、互动 ACK、心跳与断线恢复

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
│  adapter-onebot11 · adapter-qqbot                     │
│  transport-ws · transport-http · transport-qqbot      │
├─────────────────────────────────────────────────────┤
│                   内置模块                             │
│  mod-command · mod-admin · mod-scheduler · mod-bridge  │
└─────────────────────────────────────────────────────┘
```

## 快速开始

先按运行环境选择安装方式：

| 方式 | 适合场景 | 额外要求 |
| --- | --- | --- |
| Docker Compose | Linux 服务器、NAS、Portainer | Docker Engine、Compose v2 |
| Release 二进制 | Windows、macOS、普通 Linux 主机 | 无需 Rust 和 Node.js |
| 源码构建 | 修改框架、开发静态插件 | Rust 1.89+、Node.js 22 |

接入官方 QQ Bot 不需要 OneBot 实现端。凭据、事件权限和群内 @ 测试见[官方 QQ Bot 接入](docs/guide/qq-official-quickstart.md)。接入 OneBot 11 时，需要先准备 [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core)、[NapCat](https://github.com/NapNeko/NapCatQQ) 等实现。

### Docker Compose

Linux 服务器接入 QQ 官方机器人，可以直接运行一键安装：

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/lvyunqi/QimenBot/main/deploy/docker/install.sh)
```

脚本会询问 AppID 和 Secret、生成管理 Token，并把数据保存到 `~/qimenbot/data/`（root 用户为 `/opt/qimenbot/data/`）。配置、动态插件、日志和商城缓存分别映射到容器的 `/data/config`、`/data/plugins`、`/data/logs`、`/data/cache/marketplace`。

NAS、Portainer、OneBot 或自定义数据盘请使用手动 Compose：

```bash
git clone --depth 1 https://github.com/lvyunqi/QimenBot.git
cd QimenBot
cp deploy/docker/.env.example deploy/docker/.env
# 编辑 .env 中的管理 Token、QQ 凭据和三个宿主机目录
docker compose --env-file deploy/docker/.env up -d
docker compose --env-file deploy/docker/.env ps
docker compose --env-file deploy/docker/.env logs -f qimenbot
```

镜像默认为 [`mryunqi/qimenbot`](https://hub.docker.com/r/mryunqi/qimenbot)，支持 `linux/amd64` 和 `linux/arm64`。启动后打开 `http://服务器IP:3210/`，其余机器人、模块和插件配置在 Web 面板完成。

### Release 二进制

从 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases) 下载系统对应的 `QimenBot-*` 完整压缩包。根目录的 `qimenbot` 是唯一启动入口，`runtime/` 是内部目录。复制示例配置并修改 Bot 信息：

```bash
cp config/base.toml.example config/base.toml
cp config/qimenbot.toml.example config/qimenbot.toml
chmod +x qimenbot
./qimenbot run
```

Windows PowerShell 使用 `.\qimenbot.exe run`。生产环境只注册 `qimenbot`，不要单独启动 `runtime/qimenbotd`。

> 需要动态插件时，Linux 必须下载与 CPU 对应的 `*-unknown-linux-gnu` 包，或使用 Docker。静态链接的 `x86_64-unknown-linux-musl` 包不能加载 `.so` 动态插件。

### 源码构建

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot
npm --prefix web/admin ci
npm --prefix web/admin run build
cargo run --package qimenbotd
```

管理面板会嵌入 Rust 二进制，所以前端必须先构建。Docker 一键安装、目录映射、systemd、Windows Service、备份恢复和故障排查见[完整部署指南](docs/guide/deployment.md)。

第三方动态插件可以在管理面板的“插件商城”中安装和更新。商城会按 QimenBot、ABI、OS、CPU、GNU/MSVC 和 glibc 过滤版本，并按版本展示 OneBot 11、官方 QQ Bot 的场景、事件和发送能力；替换失败时会恢复旧二进制。管理员查看[商城使用教程](docs/plugin/marketplace.md)，插件作者从[商城投稿规范](docs/marketplace/index.md)开始。

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
- 全局可加载多个模块，各 Bot 通过 `enabled_modules` 选择实际启用的模块

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
#   "command"   — 命令解析与插件路由（不附带业务命令）
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

# 插件启用状态和命令优先级的持久化文件
# 用 Web 管理面板或管理 API 修改后会保存在这里
plugin_state_path = "config/plugin-state.toml"

# 动态插件（.so/.dll/.dylib）的扫描目录
plugin_bin_dir = "plugins/bin"

# 动态插件独立 TOML 配置目录
plugin_config_dir = "config/plugins"

[official_host.commands]
# 插件未注册 help/h 时提供分页目录；关闭后 help 完全由插件接管
help_enabled = true
help_page_size = 6            # /help 2 查看第 2 页，范围 1-20
# 三个宿主管理命令可分别关闭；关闭后插件仍可使用同名命令
plugins_enabled = true        # /plugins：查看、启停和重扫插件
registry_enabled = true       # /registry：查看命令冲突和优先顺序
dynamic_errors_enabled = true # /dynamic-errors：查看或清理动态插件错误
prefixes = ["/"]              # 可配置多个；空数组表示关闭前缀入口
private_bare_enabled = true   # 私聊可直接输入命令
mention_enabled = true        # 支持 @机器人 命令
reply_enabled = true          # 支持回复机器人后输入命令
```

多个插件注册同名命令时，可以在 Web 管理面板的“插件”页面设置 `0-1000` 的命令优先级。数值越大越先匹配，默认顺序是静态插件 `30`、动态插件 `20`；数值相同才使用插件声明顺序和插件 ID 决定结果。修改会写入 `plugin-state.toml`，并让已启用的 Bot 重连以刷新命令路由。

Runtime 不注册 `ping`、`echo`、`status` 等业务命令。宿主只保留需要管理员或所有者权限的 `/plugins`、`/registry` 和 `/dynamic-errors`，三项都能在 Web“配置 → 命令入口”中单独关闭。它们以优先级 `10` 进入普通命令注册表；静态插件默认 `30`、动态插件默认 `20`，因此插件声明同名命令时默认由插件接管。宿主的 `help` 是可关闭、可被插件接管的分页兜底。

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

以上代码构成一个可用插件。各宏的作用如下。

### `#[module]` — 声明模块

标记在 `impl` 块上方，用于声明插件模块。宏自动创建 `struct MyPlugin;` 结构体并生成 `Module` trait 实现。

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

**命令名自动推导**：未指定 `name = "xxx"` 时，宏使用**函数名**作为命令名，并将下划线 `_` 替换为连字符 `-`：

| 函数名 | 推导出的命令名 | 用户输入 |
|--------|---------------|---------|
| `ping` | `"ping"` | `/ping` |
| `echo` | `"echo"` | `/echo hello` |
| `group_info` | `"group-info"` | `/group-info` |

**可用的方法签名**：宏根据参数类型注入相应数据：

```rust
// 无参数 — 最简单
async fn ping(&self) -> Message { ... }

// 仅参数 — 框架自动按空格拆分命令后的文字
async fn echo(&self, args: Vec<String>) -> Message { ... }

// 仅上下文 — 获取发送者、群号等信息
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
| 声明方式 | `#[module]` + `#[commands]` 宏 | `#[dynamic_plugin]` 宏 |
| API 访问 | 完整（async、OneBotActionClient 等） | 同步 FFI（宏自动生成导出代码） |
| 消息构建 | `MessageBuilder` 链式 | `CommandResponse::builder()` / `SendBuilder` |
| 拦截器 | `MessageEventInterceptor` trait | `#[pre_handle]` / `#[after_completion]` |
| HTTP Webhook | 由应用自行挂载 HTTP 服务 | API 0.5+ `#[webhook]`，由框架统一提供网关 |
| 在线配置 | 自行开发配置入口 | API 0.6 JSON Schema，由 Web 面板生成表单 |
| 生命周期 | 随框架启停 | `#[init]` / `#[shutdown]` 钩子 |
| 热重载 | 需要重启进程 | Web 插件页点击“重新扫描” |
| 适用场景 | 核心功能、需要异步 API | 第三方扩展、快速迭代 |

### 在主仓库外独立开发

动态插件不需要加入 QimenBot 主 workspace。API 0.6 配套 crate 已发布到 crates.io：

```toml
[package]
name = "qimen-dynamic-plugin-myplugin"
version = "0.1.0"
edition = "2024"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "0.1.13"
qimen-dynamic-plugin-derive = "0.1.13"
abi_stable = "0.11"
```

[`abi-stable-host-api`](https://crates.io/crates/abi-stable-host-api) 和 [`qimen-dynamic-plugin-derive`](https://crates.io/crates/qimen-dynamic-plugin-derive) `0.1.13` 支持动态插件 API `0.1` 至 `0.6`，包含实时主动发送、Webhook、Schema 在线配置和完整媒体 builder。crate 发布版本与插件描述符中的 ABI API 相互独立；新插件应显式声明 `api = "0.6"`，旧版 API 0.1 至 0.5 插件仍可由新版宿主加载。

> crates.io `0.1.12` 只支持到动态 API 0.5，不能与 `api = "0.6"` 混用。API 0.6 插件应同时使用两个 `0.1.13` 配套 crate。
>
> `0.1.13` 解决的是插件编译依赖，不会升级已经安装的宿主。QimenBot `v0.1.17` 及更早版本只接受到动态 API 0.5；API 0.6 插件需要 `v0.1.18` 或更高版本宿主。

仓库外的插件不需要 `[workspace]`。只有把独立插件放在 QimenBot 仓库目录内、但不加入主 workspace 时，才需要在插件 `Cargo.toml` 中添加空的 `[workspace]` 表。

### 最小动态插件

使用 `#[dynamic_plugin]` 过程宏，无需手写 FFI 导出代码：

```rust
use abi_stable_host_api::*;
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0", api = "0.6")]
mod my_plugin {
    use super::*;

    #[command(name = "hello", description = "Say hello")]
    fn hello(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(&format!("Hello, {}!", req.sender_id))
    }
}
```

宏自动生成 `qimen_plugin_descriptor()` 和所有 `extern "C" fn` 导出，插件代码负责实现业务逻辑。

### API 0.4+ 实时主动推送

实时主动发送从 API `0.4` 开始提供，API `0.5` 完整包含该能力。新建插件应显式声明 API `0.5`，并为每次发送指定稳定的 Bot 账号或运行时实例别名。OneBot 部署应在 `[[bots]]` 中把 QQ / `self_id` 配置为 `account_id`：

```toml
[[bots]]
id = "qq-reverse"
account_id = "2733944636"
protocol = "onebot11"
transport = "ws-reverse"
```

```rust
use abi_stable_host_api::{BotApi, SendEnqueueStatus};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "push-example", version = "0.1.0", api = "0.5")]
mod push_example {
    use super::*;

    fn push_now() {
        match BotApi::for_account("2733944636")
            .send_group_msg("123456", "后台实时通知")
        {
            SendEnqueueStatus::Accepted => {}
            status => eprintln!("主动发送未被宿主接受: {status:?}"),
        }
    }
}
```

`BotApi::for_bot("qq-main")` 和 `.bot("qq-main")` 仍然可用，适合必须精确选择某个部署实例的场景；一般业务插件优先使用稳定账号，这样部署侧修改 `id` 后无需重新编译插件。

`Accepted` 仅表示宿主已复制请求并接受入队，不表示网络发送已经成功。`try_send()` 还可能返回 `HostUnavailable`、`InvalidRequest`、`BotNotFound`、`BotDisabled`、`QueueFull` 或 `HostShuttingDown`。实时接口支持私聊、群聊、频道和频道私信；OneBot 频道目标通过 `SendBuilder::guild_id(...)` 补充 `guild_id`。

宿主默认给每个启用 Bot 建立容量为 `256` 的独立队列，离线请求最多等待 `60` 秒：

```toml
[official_host.proactive_send]
queue_capacity = 256
offline_ttl_secs = 60
```

API 0.4/0.5 的 Host API 都会在插件 `init` 前绑定，因此后台线程不需要等待命令、事件或 Heartbeat。插件必须在 `shutdown` 中停止并 `join` 自己创建的线程，然后宿主才会解绑 Host API 和卸载动态库。完整目标映射、状态码和线程示例见 [API 0.4+ 实时主动推送](docs/advanced/dynamic-proactive-send-v04.md)。

### API 0.5 Webhook Gateway

API `0.5` 动态插件可以声明同步 HTTP Webhook，由框架统一监听、鉴权、限制请求大小和并发量，并把请求精确路由到插件：

```rust
use abi_stable_host_api::{WebhookRequest, WebhookResponse};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "webhook-example", version = "0.1.0", api = "0.5")]
mod webhook_example {
    use super::*;

    #[webhook(method = "POST", path = "/events")]
    fn receive_event(request: &WebhookRequest) -> WebhookResponse {
        WebhookResponse::text(200, format!("received {} bytes", request.body.len()))
    }
}
```

启用网关后，该处理器的完整地址是 `/webhooks/webhook-example/events`：

```toml
[official_host.webhook]
enabled = true
bind = "127.0.0.1:8088"
base_path = "/webhooks"
max_body_bytes = 1048576
request_timeout_ms = 5000
max_in_flight = 64
access_token = ""
```

网关默认关闭且只监听回环地址。生产部署建议配置 Bearer token、在反向代理处启用 TLS，并由插件按第三方协议验证 HMAC 签名和时间戳。Webhook 回调中如需主动发送消息，必须通过 `BotApi::for_account(...)` / `BotApi::for_bot(...)` 或 `.bot_account(...)` / `.bot(...).try_send()` 明确选择 Bot。完整配置、状态码、热重载和安全边界见 [API 0.5 动态插件 Webhook Gateway](docs/advanced/dynamic-webhook-v05.md)。

### API 0.6 在线配置

API 0.6 插件可以把 JSON Schema 和可选 UI Schema 编进动态库。管理面板按 Schema 生成表单，统一处理密钥脱敏、校验、revision 冲突、配置备份和失败回滚：

```rust
#[dynamic_plugin(
    id = "config-example",
    version = "0.1.0",
    api = "0.6",
    config_schema = "../config.schema.json",
    config_ui = "../config.ui.json",
    config_version = 1,
    config_apply = "reload"
)]
mod config_example {
    // 命令、init 和可选 #[validate_config]
}
```

`config_apply` 支持即时应用、动态重载和等待宿主重启。密钥不会通过 GET 接口返回浏览器，对象数组调整顺序时也会保留密钥与原业务项的对应关系。完整字段映射、回调和发布规则见 [动态插件 API 0.6 在线配置](docs/advanced/dynamic-config-v06.md)。

### 构建 & 部署

```bash
# 1. 在任意目录创建独立 crate
cargo new --lib qimen-dynamic-plugin-myplugin
# Cargo.toml 中设置 crate-type = ["cdylib"] 并添加上述 crates.io 依赖

# 2. 编译
cd qimen-dynamic-plugin-myplugin
cargo build --release

# 3. 部署：复制或上传动态库到 QimenBot 的 plugin_bin_dir
scp target/release/libqimen_dynamic_plugin_myplugin.so user@bot-host:/opt/qimenbot/plugins/bin/
# Windows: Copy-Item target/release/qimen_dynamic_plugin_myplugin.dll C:\qimenbot\plugins\bin\

# 4. 在 Web 管理面板的“插件”页点击“重新扫描”
```

动态库必须与 QimenBot 的操作系统、CPU 架构和 C 运行时一致。Docker `linux/amd64` 使用 `x86_64-unknown-linux-gnu` 插件，Docker `linux/arm64` 使用 `aarch64-unknown-linux-gnu` 插件；musl 发行包不支持动态插件。完整 target 对照和排错流程见[动态插件开发文档](docs/plugin/dynamic.md)。

### 宏属性一览

```rust
#[dynamic_plugin(id = "my-plugin", version = "0.1.0")]
mod my_plugin {
    // 生命周期
    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult { ... }

    #[shutdown]
    fn on_shutdown() { ... }

    // 命令（支持 name, description, aliases, category, role, scope）
    #[command(name = "greet", description = "打招呼", aliases = "hi,hello", role = "admin", scope = "group")]
    fn greet(req: &CommandRequest) -> CommandResponse { ... }

    // 拦截器
    #[pre_handle]
    fn on_pre_handle(req: &InterceptorRequest) -> InterceptorResponse { ... }

    // 系统事件路由
    #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
    fn on_poke(req: &NoticeRequest) -> NoticeResponse { ... }
}
```

### 响应构建

```rust
// 纯文本快捷回复
CommandResponse::text("hello")

// 链式构建富媒体回复（引用原消息 + @发送者 + 文本 + 表情）
CommandResponse::builder()
    .reply(msg_id)
    .at(sender_id)
    .text(" 你好！")
    .face(1)
    .build()

// 兼容 API 0.1-0.3：在当前 FFI 回调返回后由宿主 flush
BotApi::send_group_msg(group_id, "通知内容");
SendBuilder::private(user_id).text("私聊消息").send();

// API 0.4：按稳定账号选择 Bot，立即提交到对应实例的实时队列
let status = BotApi::for_account("2733944636")
    .send_group_msg(group_id, "实时通知");
let status = SendBuilder::channel(channel_id)
    .guild_id(guild_id)
    .bot_account("2733944636")
    .text("频道通知")
    .try_send();
```

### 运行时管理

插件启用、停用、重新扫描、在线配置和健康状态都可以在 Web 管理面板的“插件”页操作，管理动作走带 Token 鉴权的 `/api/v1/plugins` 接口。聊天内也默认提供仅管理员可用的 `/plugins`、`/registry` 和 `/dynamic-errors`；不需要时可逐项关闭，插件也能通过更高的命令优先级接管同名命令。静态插件开关需要重启；动态插件可在面板或聊天命令中重新扫描并热加载。

### 熔断器机制

动态插件内置熔断器保护：

- 连续 3 次失败 → 插件自动隔离 60 秒
- 隔离期间所有请求直接返回错误
- 成功执行后自动重置失败计数
- 在 Web 插件页重新扫描后重建动态运行时状态

完整示例见 [`plugins/qimen-dynamic-plugin-example/`](plugins/qimen-dynamic-plugin-example/)。

## 命令入口

框架本身不附带业务命令，`ping`、`echo`、`status` 等名称都由插件决定。默认提供分页帮助兜底，以及仅管理员可用的 `/plugins`、`/registry`、`/dynamic-errors` 三个宿主管理命令；四项都可在 Web 面板关闭，插件注册同名命令且优先级更高时会直接接管。

命令入口支持前缀、私聊直发、`@bot` 提及和回复触发。前缀可配置多个，四类入口都能在“配置 → 命令入口”中独立调整，保存后 Bot 自动重连生效。

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
│   ├── qimen-adapter-qqbot/ # 官方 QQ Bot 协议适配器
│   ├── qimen-transport-ws/  # WebSocket 传输（TLS、自动重连）
│   ├── qimen-transport-http/# HTTP 传输
│   ├── qimen-transport-qqbot/# 官方 QQ Bot Gateway/OpenAPI
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
        ├── qq-backup.toml
        └── qq-official.toml
```

## 协议支持

| 协议 | 状态 | 传输模式 |
|------|------|---------|
| OneBot 11 | ✅ 生产就绪 | WS 正向、WS 反向、HTTP API、HTTP POST |
| 官方 QQ Bot | ✅ 已支持 | Gateway + OpenAPI |
| OneBot 12 | 🔲 计划中 | — |
| Satori | 🔲 计划中 | — |

## 致谢

QimenBot 的设计参考了以下优秀项目：

- [Shiro](https://github.com/MisakaTAT/Shiro) — 基于 Java 的 OneBot 框架，拦截器与插件模型的灵感来源
- [Kovi](https://github.com/ThriceCola/Kovi) — Rust OneBot 框架，简洁 API 设计的参考

## 许可证

[MIT](LICENSE)
