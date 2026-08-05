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

两级配置的职责如下：
- `[official_host]` 控制"加载哪些模块到内存"
- `[[bots]].enabled_modules` 控制"这个 Bot 实际启用哪些模块"
- 全局可加载多个模块，各 Bot 通过 `enabled_modules` 选择实际启用的模块

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
| `level` | `String` | `"info"` | tracing 过滤表达式，可填写 `trace` / `debug` / `info` / `warn` / `error` 或按模块组合 |
| `json_logs` | `bool` | `false` | `true` 时输出 JSON 格式（适合 ELK / Loki 等日志采集系统） |
| `metrics_bind` | `String` | `"127.0.0.1:9090"` | Metrics HTTP 端点地址（预留功能） |

::: tip 日志级别选择
- **开发调试** → `debug` 或 `trace`（包含收到和发出的原始消息 JSON）
- **日常运行** → `info`（推荐，记录关键操作）
- **生产环境** → `warn`（只记录警告和错误）
:::

原始消息使用独立日志 target `qimen_raw_message`。只想临时查看收发内容、又不想打开其他模块的调试日志时，可以使用 EnvFilter 写法：

```toml
[observability]
level = "info,qimen_raw_message=debug"
```

这类日志的正文就是协议侧完整 JSON，附带 `direction`、`bot_id`、`protocol`、`transport`、`action` 等检索字段。`inbound` 表示收到的消息，`outbound` 表示机器人回复或主动发送。原始内容可能包含用户消息、OpenID、群号和媒体地址，只应在开发或临时排错时开启。

## `[admin_web]` — Web 管理面板

QimenBot 自带本地 Web 管理面板，默认访问地址是 `http://127.0.0.1:3210`。它可以查看 Bot 状态和实时日志，管理动态插件，并在校验通过后编辑配置。

```toml
[admin_web]
enabled = true
bind = "127.0.0.1:3210"
access_token = "${QIMEN_ADMIN_TOKEN}"
log_capacity = 2000
audit_path = "config/admin-audit.jsonl"
```

本机只监听回环地址时可以不配置 Token。只要 `bind` 使用非回环地址，就必须提供 `access_token`。面板中的管理 Token 和 Webhook Token 只写不读，页面只显示“已配置”或“未配置”徽标；留空保存会保留原值。

配置页分为运行时、面板安全、模块与插件、插件商城、Webhook、配置版本六个分区。保存前会先校验 TOML 并备份当前版本，涉及监听地址、鉴权或启动期模块的修改会标记为“需要重启”。完整操作说明见 [Web 管理面板](/guide/web-admin)。

## `[marketplace]` — 插件商城

```toml
[marketplace]
enabled = true
cache_dir = "cache/marketplace"
lock_path = "config/marketplace-lock.toml"
request_timeout_secs = 30
allow_prerelease = false
auto_update = false
```

商城目录源文件位于 [`lvyunqi/QimenBot`](https://github.com/lvyunqi/QimenBot) 主仓库。插件投稿 PR 合并到 `main` 后，GitHub Pages 流水线会重新校验目录、生成索引并自动发布，通常一到几分钟后可以读取。客户端固定使用这个官方索引，不能通过配置或管理面板改成其他来源。

`cache_dir` 保存目录缓存、下载资产、历史审核版本和文件替换事务；`lock_path` 保存当前安装的仓库数字 ID、版本、target、SHA256、固定和回滚状态。两个目录都应放在持久化磁盘并纳入备份。请求超时允许 1-300 秒。`allow_prerelease` 只控制 beta、rc 等版本是否参与选择，不会自动安装。当前版本要求 `auto_update = false`，第三方插件更新必须在 Web 面板人工确认。完整说明见[插件商城](/plugin/marketplace)。

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

# 动态插件独立配置目录
plugin_config_dir = "config/plugins"
```

### 内置模块列表

| 模块 ID | 说明 |
|---------|------|
| `command` | 命令解析、触发识别与插件路由；不注册业务命令 |
| `admin` | 权限解析和宿主管理基础能力；操作入口在 Web 面板 |
| `scheduler` | 定时任务 — 基于 Cron 表达式的定时任务调度器 |
| `bridge` | 消息桥接 — 跨群 / 跨 Bot 消息转发（预留） |

### 插件模块

`plugin_modules` 中填写的是插件的 `#[module(id = "xxx")]` 中声明的 `id`：

```toml
# 框架自带的示例插件
plugin_modules = ["example-plugin"]

# 自定义插件
plugin_modules = ["example-plugin", "my-plugin"]
```

### 命令冲突与插件优先级

多个插件可以声明同一个命令名或别名，但每次只会选中一个插件。管理员可以在 Web 管理面板的“插件”页面设置命令优先级，范围是 `0-1000`，数值越大越先匹配。

| 来源 | 默认值 | 能否在面板修改 |
| --- | ---: | --- |
| 宿主管理命令 | `10` | 不能修改数值，可逐项关闭 |
| 静态插件 | `30` | 可以 |
| 动态插件 | `20` | 可以 |

优先级相同时，先比较插件声明的次级顺序：静态插件使用 `CommandPlugin::priority()`，该值较小的排在前面；动态插件使用兼容默认值 `200`。仍然相同则按插件 ID 排序，保证每次启动的结果一致。

Runtime 不注册 `ping`、`echo`、`status` 等业务命令。宿主默认注册 `/plugins`、`/registry` 和 `/dynamic-errors` 三个管理命令，均要求管理员或所有者权限，并以优先级 `10` 参加普通注册表排序。静态、动态插件的默认优先级分别是 `30`、`20`，所以插件声明同名命令时默认由插件接管；关闭某个宿主命令后，其命令名和别名都不再保留。`help`/`h` 在插件匹配失败后兜底，插件注册同名命令时直接接管。

设置后会写入 `plugin_state_path` 指向的文件：

```toml
[modules]
"example-plugin" = true

[priorities]
"example-plugin" = 80
```

面板保存后会让已启用的 Bot 重连一次并重建命令路由，不需要重启宿主。该文件由宿主自动维护，运行期间不要同时手工编辑。完整排序过程见[运行时详解](/advanced/runtime#命令优先级)。

### 命令入口和帮助分页

前缀、私聊裸命令、@ 提及和回复是四种独立入口。群聊普通文本不会直接进入命令路由；要么带 `prefixes` 中的前缀，要么明确 @/回复机器人。`prefixes` 可以配置 `/`、`!` 等多个值，最长匹配优先。

宿主帮助按当前有效注册表生成，不维护另一份固定清单，因此停用插件后不会残留旧的 `ping` 或其他业务命令；已开启且未被插件覆盖的三个宿主管理命令会正常进入目录。默认每页 6 条：

```text
/help      # 第 1 页
/help 2    # 第 2 页
/h 3       # 使用别名查看第 3 页
```

页码超过总页数时会定位到最后一页。隐藏命令不会进入目录；每页回复会显示当前页、总页数和上一页/下一页命令。

### 动态插件目录

`plugin_bin_dir` 指定动态库文件的扫描目录。框架启动时会自动扫描该目录下的 `.so` / `.dll` / `.dylib` 文件：

```toml
plugin_bin_dir = "plugins/bin"
```

`plugin_config_dir` 保存动态插件自己的 TOML 配置，默认是 `config/plugins`：

```toml
plugin_config_dir = "config/plugins"
```

命令入口另有独立子表，Web 面板对应“配置 → 命令入口”：

```toml
[official_host.commands]
help_enabled = true          # 插件未接管 help/h 时提供分页目录
help_page_size = 6           # /help 2 查看第 2 页，范围 1-20
plugins_enabled = true       # /plugins：插件状态、启停和动态重扫
registry_enabled = true      # /registry：命令冲突和优先顺序
dynamic_errors_enabled = true # /dynamic-errors：动态插件错误状态
prefixes = ["/"]             # 支持多个前缀；空数组关闭前缀入口
private_bare_enabled = true  # 私聊可不带前缀
mention_enabled = true       # @机器人 后可直接输入命令
reply_enabled = true         # 回复机器人后可直接输入命令
```

文件名固定为 `<plugin_id>.toml`。API 0.6 插件可以在 Web 管理面板中生成表单并在线保存；手工编辑仍然支持。Docker 部署应把该目录放在持久化卷中，二进制部署应和 `plugin-state.toml` 一起备份。完整在线配置规则见 [动态插件 API 0.6 在线配置](/advanced/dynamic-config-v06)。

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

# GROUP_AND_C2C_EVENT: QQ 群 @/全量消息和 QQ 单聊 C2C 消息
# PUBLIC_GUILD_MESSAGES: 频道 @ 消息
# DIRECT_MESSAGE: 频道私信消息
intents = ["GROUP_AND_C2C_EVENT", "PUBLIC_GUILD_MESSAGES", "DIRECT_MESSAGE"]

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

官方 Bot 字段：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `appid` | 无 | QQ 开放平台 AppID，启用时不能为空 |
| `secret` | 无 | QQ 开放平台 AppSecret，启用时不能为空 |
| `sandbox` | `false` | 是否使用官方沙箱 OpenAPI 基地址 |
| `intents` | `[]` | Gateway Identify 时申请的事件位掩码 |
| `account_id` | 无 | 主动发送使用的稳定账号选择器，可填写长期不变的应用账号标识 |

常用 intent：

| 名称 | 用途 |
|------|------|
| `GROUP_AND_C2C_EVENT` | QQ 群 @、获准使用的全量群消息、QQ 单聊 C2C |
| `PUBLIC_GUILD_MESSAGES` | 公域频道 @ 和相关频道消息 |
| `DIRECT_MESSAGE` | 频道私信 |
| `INTERACTION` | 按钮和快捷菜单互动 |
| `MESSAGE_AUDIT` | 消息审核结果 |
| `GUILDS` / `GUILD_MEMBERS` | 频道及频道成员事件 |
| `GUILD_MESSAGE_REACTIONS` | 频道消息表情回应 |
| `FORUMS_EVENT` / `OPEN_FORUM_EVENT` | 论坛事件 |
| `AUDIO_ACTION` / `AUDIO_OR_LIVE_CHANNEL_MEMBER` | 音频和直播子频道事件 |

名称不区分大小写，未知名称会在配置校验阶段报错。`public_messages` 是 `GROUP_AND_C2C_EVENT` 的兼容别名，`forums` 是 `FORUMS_EVENT` 的兼容别名。平台授权和本地 `intents` 必须同时满足；填写 intent 本身不会开通平台权限。

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
plugin_config_dir = "config/plugins"

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
account_id = "2733944636"
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

queue_capacity 是每个启用 Bot 的独立队列容量，必须大于 0。offline_ttl_secs 是离线请求等待对应 Bot 上线的时间；设置为 0 会在离线时立即丢弃。详见 [API 0.4+ 实时主动推送](/advanced/dynamic-proactive-send-v04)。

每个 `[[bots]]` 可选配置 `account_id`。`id` 是部署实例别名，`account_id` 是插件主动发送时可长期引用的稳定外部账号；OneBot 通常填写 Bot QQ，也就是事件中的 `self_id`。旧配置无需增加该字段，按 `bot_id` 发送仍然兼容。多个启用 Bot 不能使用相同的 `account_id`，但禁用的备用传输实例可以与当前启用实例填写同一账号。

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
