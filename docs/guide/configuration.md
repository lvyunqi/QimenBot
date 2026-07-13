# 配置详解

框架启动时只读取一个配置文件：**`config/base.toml`**。所有全局设置和 Bot 实例都在这一个文件中定义。

::: info 关于其他配置文件
`config/bots/` 目录下的 `.toml` 文件**不会被框架自动加载**，它们仅作为多 Bot 场景下的管理参考或备份模板。`config/dev.toml` 和 `config/prod.toml` 也仅供手动切换环境时参考。
:::

## 配置文件结构总览

```
config/base.toml
│
├── [runtime]               全局运行时设置
├── [observability]         日志与监控
├── [official_host]         模块加载（全局共享）
│
├── [[bots]]                Bot 实例 1
├── [[bots]]                Bot 实例 2
└── [[bots]]                Bot 实例 3 ...
```

关键设计：**`[official_host]` 是全局的，`[[bots]]` 是每个 Bot 实例独立的**。

```
┌─────────────────────────────────────────────────────┐
│  [official_host]（全局）                               │
│  决定框架加载哪些模块和插件到内存                          │
│  所有 Bot 共享同一套模块代码                              │
├──────────────────────────┬──────────────────────────┤
│  [[bots]] qq-main        │  [[bots]] qq-backup      │
│  ├─ 连接地址 endpoint     │  ├─ 监听地址 bind         │
│  ├─ enabled_modules      │  ├─ enabled_modules      │
│  ├─ owners / admins      │  ├─ owners / admins      │
│  └─ 各种策略配置           │  └─ ...                  │
└──────────────────────────┴──────────────────────────┘
```

**简单来说：**
- `[official_host]` 控制"加载哪些模块到内存"
- `[[bots]].enabled_modules` 控制"这个 Bot 实际启用哪些模块"
- 你可以全局加载 10 个模块，但某个 Bot 只启用其中 3 个

## `[runtime]` — 运行时

```toml
[runtime]
env = "dev"                    # 运行环境："dev" 或 "prod"
shutdown_timeout_secs = 15     # 关闭信号后等待任务完成的超时（秒）
task_grace_secs = 5            # 后台任务的优雅退出等待（秒）
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|-------|------|
| `env` | `String` | `"dev"` | 运行环境标识。`"dev"` 模式下日志更详细 |
| `shutdown_timeout_secs` | `u64` | `15` | 收到 Ctrl+C 后等待进行中的任务完成的最大秒数 |
| `task_grace_secs` | `u64` | `5` | 后台任务（定时器、重连循环等）收到停止信号后的等待秒数 |

## `[observability]` — 日志与监控

```toml
[observability]
level = "info"                    # 日志级别
json_logs = false                 # 是否输出 JSON 格式日志
metrics_bind = "127.0.0.1:9090"   # Metrics 暴露地址（预留）
```

| 字段 | 类型 | 默认值 | 说明 |
|------|------|-------|------|
| `level` | `String` | `"info"` | 日志级别：`trace` / `debug` / `info` / `warn` / `error` |
| `json_logs` | `bool` | `false` | `true` 时输出 JSON 格式（适合 ELK / Loki 等日志采集系统） |
| `metrics_bind` | `String` | `"127.0.0.1:9090"` | Metrics HTTP 端点地址（预留功能） |

::: tip 日志级别选择
- **开发调试** → `debug` 或 `trace`（信息量大，包含事件原始数据）
- **日常运行** → `info`（推荐，记录关键操作）
- **生产环境** → `warn`（只记录警告和错误）
:::

## `[official_host]` — 全局模块加载

这个区块决定框架启动时**加载哪些模块到内存**，是全局共享的。

```toml
[official_host]
# 内置模块
builtin_modules = ["command", "admin", "scheduler", "bridge"]

# 第三方插件模块
plugin_modules = ["example-plugin"]

# 插件状态持久化文件
plugin_state_path = "config/plugin-state.toml"

# 动态插件扫描目录
plugin_bin_dir = "plugins/bin"
```

### 内置模块列表

| 模块 ID | 说明 |
|---------|------|
| `command` | 命令系统 — `/ping`、`/echo`、`/help`、`/status` 等基础命令 |
| `admin` | 管理模块 — 权限管理、插件管理（`/plugins`） |
| `scheduler` | 定时任务 — 基于 Cron 表达式的定时任务调度器 |
| `bridge` | 消息桥接 — 跨群 / 跨 Bot 消息转发（预留） |

### 插件模块

`plugin_modules` 中填写的是插件的 `#[module(id = "xxx")]` 中声明的 `id`：

```toml
# 框架自带的示例插件
plugin_modules = ["example-plugin"]

# 如果你开发了自己的插件
plugin_modules = ["example-plugin", "my-plugin"]
```

### 动态插件目录

`plugin_bin_dir` 指定动态库文件的扫描目录。框架启动时会自动扫描该目录下的 `.so` / `.dll` / `.dylib` 文件：

```toml
plugin_bin_dir = "plugins/bin"
```

## `[[bots]]` — Bot 实例配置

每个 `[[bots]]` 块定义一个独立的 Bot 实例，各实例互不影响。

### 连接与身份

```toml
[[bots]]
id        = "qq-main"                  # Bot 唯一标识（不可重复）
protocol  = "onebot11"                 # 通信协议：onebot11 / qq-official
transport = "ws-forward"               # 传输方式
endpoint  = "ws://127.0.0.1:3001"      # 连接地址
enabled   = true                       # 是否启用
```

### 传输方式

| transport | 方向 | 必填字段 | 说明 |
|-----------|------|---------|------|
| `ws-forward` | 框架 → OneBot | `endpoint` | 框架主动连接 OneBot 实现端的 WebSocket |
| `ws-reverse` | OneBot → 框架 | `bind` + `path` | 框架监听端口，等待 OneBot 实现端连接 |
| `http` | 双向 HTTP | `endpoint` | HTTP API + HTTP POST |
| `gateway` | 框架 → 官方 Bot Gateway | `appid` + `secret` | 连接官方 QQ Bot Gateway，事件走 WebSocket，动作走 OpenAPI |

**正向 WebSocket 示例**（框架主动连接）：

```toml
[[bots]]
id        = "qq-main"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"   # OneBot 实现的 WS 地址
```

**官方 QQ Bot Gateway 示例**：

```toml
[[bots]]
id        = "qq-official"
protocol  = "qq-official"
transport = "gateway"
enabled   = true

appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
sandbox = false

# public_messages: QQ 群 @ 消息和 QQ 单聊 C2C 消息
# public_guild_messages: 频道 @ 消息
# direct_message: 频道私信消息
intents = ["public_messages", "public_guild_messages", "direct_message"]

enabled_modules = ["command", "admin"]
owners = []
admins = []
```

::: tip 官方 Bot 配置要点
- `qq-official` 必须配合 `transport = "gateway"` 使用。
- `appid` 和 `secret` 可通过 `.env`、系统环境变量或部署平台环境变量注入。
- `owners` 和 `admins` 对官方 Bot 使用字符串 ID，可填 `openid`、`member_openid` 或频道用户 ID。
- `config/bots/qq-official.toml` 只是参考模板；运行时仍以 `config/base.toml` 和环境覆盖配置为准。
- 完整接入流程见 [官方 QQ Bot 接入](/guide/qq-official-quickstart)。
:::

**反向 WebSocket 示例**（框架等待连接）：

```toml
[[bots]]
id        = "qq-backup"
transport = "ws-reverse"
bind      = "0.0.0.0:6701"          # 框架监听地址
path      = "/onebot/reverse"       # WS 路径
```

### 模块与权限

```toml
# 此 Bot 启用的模块
enabled_modules = ["command", "admin", "scheduler"]

# 所有者（最高权限）
owners = ["123456"]

# 管理员
admins = ["789012"]
```

::: info 权限层级
| 角色 | 能力 |
|------|------|
| **Owner** | 所有命令 + 插件管理 + 重启等危险操作 |
| **Admin** | 标记为 `role = "admin"` 的命令（如 `/ban`） |
| **普通用户** | 无权限限制的命令（如 `/ping`、`/echo`） |
:::

### 好友请求自动审批

```toml
# 总开关
auto_approve_friend_requests = false

# 用户白名单（始终同意）
auto_approve_friend_request_user_whitelist = ["111111"]

# 用户黑名单（始终拒绝，优先级高于白名单）
auto_approve_friend_request_user_blacklist = []

# 验证消息关键词白名单（包含关键词则同意）
auto_approve_friend_request_comment_keywords = ["来自群"]

# 验证消息关键词黑名单（包含关键词则拒绝）
auto_reject_friend_request_comment_keywords = ["广告"]

# 同意时设置的备注
auto_approve_friend_request_remark = ""
```

**审批优先级：** 黑名单 > 白名单 > 关键词拒绝 > 关键词同意 > 总开关

### 群邀请自动审批

```toml
auto_approve_group_invites = false
auto_approve_group_invite_user_whitelist = []
auto_approve_group_invite_user_blacklist = []
auto_approve_group_invite_group_whitelist = ["12345678"]
auto_approve_group_invite_group_blacklist = []
auto_approve_group_invite_comment_keywords = []
auto_reject_group_invite_comment_keywords = []
auto_reject_group_invite_reason = ""
```

### 戳一戳自动回复

```toml
auto_reply_poke_enabled = true
auto_reply_poke_message = "别戳了，我在忙。"
```

### 令牌桶限流器

```toml
[bots.limiter]
enable = false       # 是否启用
rate = 5.0           # 每秒恢复的令牌数
capacity = 10        # 令牌桶容量（最大突发处理量）
timeout_secs = 0     # 等待令牌超时（0 = 直接丢弃）
```

::: tip 限流器工作原理
令牌桶限流器像一个容量有限的桶，桶里装着"令牌"：
- 每处理一条消息消耗一个令牌
- 令牌以 `rate` 的速度自动恢复
- 桶最多装 `capacity` 个令牌
- 如果桶空了，新消息会被丢弃（或等待 `timeout_secs` 秒）

**推荐设置：** `rate = 5.0, capacity = 10` 表示稳态下每秒处理 5 条消息，允许突发 10 条。
:::

## 环境变量

配置值支持 `${ENV_VAR}` 格式的环境变量占位符：

```toml
access_token = "${QQ_TOKEN}"
endpoint = "${ONEBOT_WS_ENDPOINT}"
appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
```

框架启动时自动替换。如果环境变量不存在，会被替换为空字符串。

## 完整配置示例

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
plugin_state_path = "config/plugin-state.toml"
plugin_bin_dir = "plugins/bin"

[official_host.proactive_send]
queue_capacity = 256
offline_ttl_secs = 60

[official_host.webhook]
enabled = false
bind = "127.0.0.1:8088"
base_path = "/webhooks"
max_body_bytes = 1048576
request_timeout_ms = 5000
max_in_flight = 64
access_token = ""

[[bots]]
id        = "qq-main"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"
enabled   = true
owners    = ["123456"]
admins    = ["789012"]

enabled_modules = ["command", "admin", "scheduler"]

auto_approve_friend_requests = false
auto_approve_friend_request_user_whitelist = []
auto_approve_friend_request_comment_keywords = ["来自群"]

auto_reply_poke_enabled = true
auto_reply_poke_message = "别戳了，我在忙。"

[bots.limiter]
enable = false
rate = 5.0
capacity = 10
```

## 动态插件实时主动发送队列

~~~toml
[official_host.proactive_send]
queue_capacity = 256
offline_ttl_secs = 60
~~~

queue_capacity 是每个启用 Bot 的独立队列容量，必须大于 0。offline_ttl_secs 是离线请求等待对应 Bot 上线的时间；设置为 0 会在离线时立即丢弃。详见 [API 0.4 实时主动推送](/advanced/dynamic-proactive-send-v04)。

## 动态插件 Webhook Gateway

```toml
[official_host.webhook]
enabled = false
bind = "127.0.0.1:8088"
base_path = "/webhooks"
max_body_bytes = 1048576
request_timeout_ms = 5000
max_in_flight = 64
access_token = ""
```

网关默认关闭并监听本机。启用后，API 0.5 插件声明的局部路由会暴露为 `{base_path}/{plugin_id}{path}`。例如插件 `build-events` 的 `POST /events` 对应 `POST /webhooks/build-events/events`。

`max_body_bytes`、`request_timeout_ms` 和 `max_in_flight` 必须大于 0。`access_token` 非空时，所有请求都必须携带完全匹配的 `Authorization: Bearer <token>`。第三方服务自己的 HMAC 和重放保护由插件验证。生产环境建议保持回环监听并由反向代理提供 TLS。详见 [API 0.5 Webhook Gateway](/advanced/dynamic-webhook-v05)。
