# 官方 QQ Bot 接入

官方 QQ Bot 不需要登录 QQ 客户端，也不需要 NapCat、Lagrange 等 OneBot 实现。QimenBot 通过 Gateway WebSocket 接收事件，再通过 QQ 开放平台 OpenAPI 发送消息。

接入前先分清两个概念：

- **QQ 群**：普通 QQ 群，平台使用 `group_openid`、`member_openid` 等字符串 ID。
- **频道**：QQ 频道，平台使用 `guild_id`、`channel_id` 和频道用户 ID。

两者的事件、权限和发送接口不同。QimenBot 会在适配层统一它们，但开放平台仍可能只给当前机器人开通其中一部分能力。

当前版本支持以下消息入口：

| 场景 | Gateway 事件 | 说明 |
|------|--------------|------|
| QQ 群 @ | `GROUP_AT_MESSAGE_CREATE` | 不需要全量群消息权限时，先用这个场景测试 |
| QQ 群全量消息 | `GROUP_MESSAGE_CREATE` | 需要开放平台单独开通全量消息权限 |
| QQ 单聊 | `C2C_MESSAGE_CREATE` | 官方文档也称 C2C 消息 |
| 频道 @ | `AT_MESSAGE_CREATE` | 机器人所在频道的子频道消息 |
| 频道普通消息 | `MESSAGE_CREATE` | 是否下发取决于机器人权限和 intent |
| 频道私信 | `DIRECT_MESSAGE_CREATE` | 官方文档也称 DMS |

发送侧支持文本、Markdown、Keyboard、群/C2C 媒体上传、频道图片、Ark、Embed、消息撤回和互动 ACK。具体限制见[官方 QQ Bot 插件适配](/plugin/qq-official)。

## 第 1 步：创建机器人

1. 打开 [QQ 开放平台](https://q.qq.com/)，登录后创建机器人应用。
2. 在应用管理页找到 `AppID` 和 `AppSecret`。
3. 按准备测试的场景配置机器人能力、事件订阅和测试范围。
4. 将机器人加入测试群或频道，或使用平台提供的测试环境。

平台页面名称可能随版本调整，找不到入口时以[官方 QQ Bot API 文档](https://bot.q.qq.com/wiki/develop/api-v2/)的“开发准备”和“事件订阅”说明为准。

::: danger AppSecret 不能公开
`AppSecret` 等同于机器人密码。不要把它写进 `config/*.toml`、截图、日志、Issue 或聊天记录，也不要提交到 Git。已经泄露的 Secret 应立即在开放平台重置，旧值不能继续使用。
:::

## 第 2 步：开通事件权限

平台侧允许订阅某类事件，QimenBot 才能在 Gateway 鉴权时申请对应 intent。只改本地配置不能绕过平台权限。

初次接入建议配置下面三项：

| QimenBot 配置名 | 官方 intent | 能收到的消息 |
|------------------|-------------|--------------|
| `GROUP_AND_C2C_EVENT` | Group and C2C | QQ 群 @、获准使用的全量群消息、QQ 单聊 |
| `PUBLIC_GUILD_MESSAGES` | Public Guild Messages | 频道 @ 和获准使用的频道消息 |
| `DIRECT_MESSAGE` | Direct Message | 频道私信 |

`GROUP_AND_C2C_EVENT` 并不自动授予“全量群消息”权限。没有该权限时，群内普通消息不会下发，但群内 @ 机器人仍可通过 `GROUP_AT_MESSAGE_CREATE` 到达。需要读取所有群消息时，应在开放平台按官方要求申请，审核通过后再测试 `GROUP_MESSAGE_CREATE`。

配置 intent 时还要注意：

- 开放平台的事件订阅和本地 `intents` 必须同时包含目标能力。
- intent 名称不区分大小写，文档和新配置建议使用上表中的官方大写名称。
- `public_messages`、`public_guild_messages`、`direct_message` 等旧写法继续兼容。
- 配置了机器人没有权限使用的 intent 时，平台可能在 Identify 阶段拒绝会话。

## 第 3 步：保存凭据

推荐把凭据放在项目根目录的 `.env` 中：

```dotenv
QQBOT_APPID=你的 AppID
QQBOT_SECRET=你的 AppSecret
```

`.env` 已被 Git 忽略。`qimenbotd` 从项目根目录启动时会自动加载它，并替换 TOML 中的 `${QQBOT_APPID}` 和 `${QQBOT_SECRET}`。

也可以只给当前终端设置环境变量。

PowerShell：

```powershell
$env:QQBOT_APPID = "你的 AppID"
$env:QQBOT_SECRET = "你的 AppSecret"
```

Linux 或 macOS：

```bash
export QQBOT_APPID='你的 AppID'
export QQBOT_SECRET='你的 AppSecret'
```

在 PowerShell 中运行 `setx` 后，变量只对新开的终端生效。当前窗口要立即启动时，应使用 `$env:...`。

## 第 4 步：配置 QimenBot

框架默认只读取 `config/base.toml`。先确认 `[official_host]` 加载了基础测试插件：

```toml
[official_host]
builtin_modules = ["command", "admin"]
plugin_modules = ["example-basic", "example-message"]
```

然后在同一个文件中加入 Bot 实例：

```toml
[[bots]]
id = "qq-official"
protocol = "qq-official"
transport = "gateway"
enabled = true

appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
sandbox = false

intents = ["GROUP_AND_C2C_EVENT", "PUBLIC_GUILD_MESSAGES", "DIRECT_MESSAGE"]
enabled_modules = ["command", "admin"]
owners = []
admins = []
```

字段含义：

| 字段 | 是否必填 | 说明 |
|------|----------|------|
| `id` | 是 | QimenBot 内部实例名，多 Bot 部署时不能重复 |
| `protocol` | 是 | 官方机器人固定为 `qq-official` |
| `transport` | 是 | 官方机器人固定为 `gateway` |
| `appid` | 是 | 建议通过环境变量注入 |
| `secret` | 是 | 建议通过环境变量注入 |
| `sandbox` | 否 | `true` 使用官方沙箱 OpenAPI，默认 `false` |
| `intents` | 是 | Gateway Identify 时申请的事件范围 |
| `owners` / `admins` | 否 | 填官方字符串 ID，不是传统 QQ 号 |

::: warning 配置模板不会自动加载
`config/bots/qq-official.toml` 是参考模板。修改这个文件不会影响默认启动配置，实际内容必须放进 `config/base.toml`，或通过 `QIMEN_CONFIG_PATH` 指向另一个完整配置文件。
:::

使用独立配置文件时：

```powershell
$env:QIMEN_CONFIG_PATH = "config/dev.toml"
cargo run -p qimenbotd
```

```bash
QIMEN_CONFIG_PATH=config/dev.toml cargo run -p qimenbotd
```

## 第 5 步：启动

从源码运行：

```bash
cargo run -p qimenbotd
```

使用 Release 包时，在解压目录中运行：

```powershell
.\qimenbotd.exe
```

```bash
./qimenbotd
```

连接成功时会看到类似日志：

```text
official host startup report ... plugin_modules=example-basic,example-message
connecting to QQ official Gateway bot_id=qq-official ...
QQ official Gateway connected bot_id=qq-official
```

这三处分别说明：

1. 示例插件已加入命令注册表。
2. AppID、Secret 和 Gateway 地址获取流程已开始。
3. WebSocket 已连接，并已发送 Identify 或 Resume。

`Gateway connected` 只表示连接建立，不能证明事件权限已经正确。还要发送一条消息完成下一步验证。

## 第 6 步：逐项测试

先测纯文本，再测富消息。一次只测一个场景，出现问题时更容易判断是权限、事件还是发送接口。

### QQ 单聊

向机器人发送：

```text
/ping
```

正常回复：

```text
pong!
```

### QQ 群 @

在群里输入 `@机器人 /ping`。不要只发送 `/ping`，除非机器人已经获准接收全量群消息。

QimenBot 同时识别官方当前使用的 `<qqbot-at-user id="..." />` 提及标签、旧版 `<@!id>` 标签和 `mentions[].is_you`。适配完成后，命令处理器收到的文本是 `/ping`，不会把 @ 标签当成命令内容。

### QQ 群全量消息

仅在开放平台已批准全量群消息权限后，直接在群里发送 `/ping`。事件应以 `GROUP_MESSAGE_CREATE` 下发。

全量模式中的 @ 也可能仍以 `GROUP_MESSAGE_CREATE` 下发，而不是 `GROUP_AT_MESSAGE_CREATE`。QimenBot 会根据 `mentions[].is_you` 判断消息是否指向机器人，插件不需要只靠事件名判断。

### 频道和频道私信

- 子频道中发送 `@机器人 /ping`，验证 `AT_MESSAGE_CREATE`。
- 向机器人发送频道私信 `/ping`，验证 `DIRECT_MESSAGE_CREATE`。

每次收到消息后，debug 日志中应能看到事件进入运行时；回复成功后会有 QQ 官方 action 执行记录。若事件日志存在而回复失败，应检查紧随其后的 OpenAPI 错误，而不是继续修改 intent。

## 测试富消息

确认 `/ping` 正常后，再使用 `example-message`：

| 命令 | 测试内容 | 适用场景 |
|------|----------|----------|
| `/qq-md` | Markdown content | 群、C2C、频道、DMS，实际权限以平台为准 |
| `/qq-md-template [template_id]` | Markdown 模板参数 | 需要已配置的官方模板 |
| `/qq-keyboard` | Markdown + 自定义 Keyboard | 需要机器人按钮能力 |
| `/qq-keyboard-template [keyboard_id]` | 模板 Keyboard | 需要有效模板 ID |
| `/qq-ark` | Ark | 频道、DMS |
| `/qq-embed` | Embed | 频道、DMS |
| `/qq-media image <url>` | 图片上传 | QQ 群、C2C |
| `/qq-media record <url>` | 语音上传 | QQ 群、C2C |
| `/qq-media video <url>` | 视频上传 | QQ 群、C2C |
| `/qq-media file <url>` | 文件上传 | QQ 群、C2C |

群和 C2C 媒体不能把普通 URL 直接放进消息体。QimenBot 会先调用对应 `/files` 接口，取回 `file_info`，再以 `msg_type = 7` 发送。测试 URL 必须能被 QQ 服务器访问；`localhost`、内网地址和需要登录的下载地址通常不可用。

自研插件生成图片时，不需要先搭建图床。动态插件可以直接返回：

```rust
CommandResponse::builder()
    .image_base64(&png_base64)
    .build()
```

QQ 群和 C2C 会自动完成官方分片预上传，频道和 DMS 会自动改用 `file_image` multipart。图片 Base64 上限为 20 MB，支持的文件头包括 PNG、JPEG、GIF、WebP 和 BMP，初次验证建议使用 PNG。不要把本地路径或 `file://` 地址当作图片参数；插件应读取文件后编码为 Base64。

## 被动回复限制

回复用户刚发来的消息属于被动回复。QQ 开放平台对回复窗口和次数有限制：

| 场景 | 有效时间 | 同一消息最多回复 |
|------|----------|------------------|
| QQ 群 | 5 分钟 | 5 条 |
| QQ 单聊 C2C | 60 分钟 | 4 条 |

群和 C2C 的多次回复由 `msg_id + msg_seq` 区分。QimenBot 会为同一条来信自动分配递增的 `msg_seq`，插件直接返回回复即可，不要复用固定序号。

耗时任务应尽快先回复“正在处理”，再根据机器人已有权限决定是否主动推送最终结果。超过平台窗口后，继续携带原 `msg_id` 会被官方接口拒绝。

## 常见问题

### 启动时报 `missing qq-official appid` 或 `secret`

环境变量没有成功替换。按顺序检查：

1. 启动目录是否为项目根目录。
2. `.env` 变量名是否正好是 `QQBOT_APPID` 和 `QQBOT_SECRET`。
3. TOML 中是否写成 `${QQBOT_APPID}`、`${QQBOT_SECRET}`。
4. 使用 Release 包时，`.env` 是否放在当前工作目录，而不是只放在二进制所在目录的上级。

### Token 请求返回 401 或鉴权错误

AppID 和 AppSecret 不匹配、Secret 已重置，或复制时带入了空格。重新从开放平台获取凭据。不要把 Secret 打进日志进行比对。

### Gateway 已连接，但完全收不到消息

这通常发生在事件订阅阶段：

1. 确认机器人已经进入当前测试群、频道或测试成员范围。
2. 确认开放平台已订阅目标事件。
3. 确认本地 `intents` 包含对应项。
4. 先用 C2C `/ping`，再测试群 @，最后测试全量群消息。
5. 全量权限未开通时，不要用群内普通 `/ping` 判断程序是否正常。

### 私聊正常，群内 @ 不回复

先看是否收到 `GROUP_AT_MESSAGE_CREATE` 或 `GROUP_MESSAGE_CREATE`：

- 没有事件：检查 `GROUP_AND_C2C_EVENT`、群测试范围和平台权限。
- 有事件但没有命令命中：确认发送的是 `@机器人 /ping`，并确认运行版本包含当前 QQ @ 解析支持。
- 命令已命中但发送失败：查看 `/v2/groups/{group_openid}/messages` 的错误分类、错误码和 `trace_id`。

群全量事件中的 @ 由 `mentions[].is_you` 判断。插件若需要自行判断，应使用 `event.is_at_self()`，不要只匹配 `GROUP_AT_MESSAGE_CREATE`。

### 收到消息，但 `/ping` 没有任何命令回复

检查启动报告中是否有 `example-basic`。它必须同时满足：

```toml
[official_host]
plugin_modules = ["example-basic"]
```

`config/plugin-state.toml` 中也不能把它持久化为禁用状态。可用 `/plugins` 查看当前插件状态。

### 返回 `unknown qq-official intent`

本地配置中存在不支持的拼写。优先使用：

```toml
intents = ["GROUP_AND_C2C_EVENT", "PUBLIC_GUILD_MESSAGES", "DIRECT_MESSAGE"]
```

### OpenAPI 返回 403

403 通常是场景权限不足，而不是 Gateway 连接问题。确认机器人是否拥有目标群、C2C、频道或 DMS 的发送能力，消息类型是否获准使用。日志中的 `category=Permission`、官方错误码和 `trace_id` 是向平台排查时最有用的信息。

### OpenAPI 返回 429

429 表示触发频控。QimenBot 会读取 `retry_after`，并按 Bot 和发送路由短暂退避；退避期间，同一路由的新发送会直接失败，避免不断请求官方接口。应降低发送频率，不要用无限重试绕过频控。

### Markdown、Keyboard、Ark 或 Embed 失败

先回到 `/ping` 验证文本链路。文本正常而富消息失败时，再检查：

- 当前场景是否支持该消息结构。
- Markdown 或 Keyboard 模板 ID 是否有效。
- 机器人是否已经开通按钮、模板或对应富消息能力。
- 群/C2C 是否使用了允许的 `msg_type`。

### Base64 图片回复失败

按日志中的失败阶段判断：

- `invalid base64`：插件返回的不是完整 Base64，或把数据截断了。
- `must be PNG...`：消息段标记为图片，但解码后的文件头不是受支持的图片格式。
- `exceeds the 20 MB memory limit`：改用 QQ 可直接访问的 HTTPS URL。
- `/upload_prepare` 或 `/upload_part_finish` 失败：检查官方媒体权限、文件类型、错误码和 `trace_id`。
- 预签名 `PUT` 失败：通常是网络、超时或对象存储 URL 失效；不要手工给该 URL 添加 QQ 鉴权头。
- 频道/DMS 返回 400 或 403：确认当前机器人和会话已开通图片发送能力，并在真实 DMS 会话中复测。

## 继续阅读

- [官方 QQ Bot 插件适配](/plugin/qq-official)：字符串 ID、消息解析、回复和富消息写法。
- [传输层：官方 QQ Bot Gateway](/advanced/transport#官方-qq-bot-gateway)：Token、Heartbeat、Resume、OpenAPI 路由和错误分类。
- [运行时原理](/advanced/runtime)：消息去重、回复序号、互动 ACK 和事件分发。
- [QQ Bot API v2 官方文档](https://bot.q.qq.com/wiki/develop/api-v2/)：平台权限、接口字段和最新政策。
