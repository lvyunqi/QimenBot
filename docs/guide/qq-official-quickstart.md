# 官方 QQ Bot 接入

本教程说明如何在 QimenBot 中接入官方 QQ Bot。官方 Bot 使用 Gateway 接收事件，使用 OpenAPI 发送消息；框架会把 QQ 群、QQ 单聊、频道消息统一转换成 `NormalizedEvent`，再交给命令、权限、限流、拦截器和插件流水线处理。

::: tip 当前支持范围
当前适配器已支持 QQ 群 @ 消息、QQ 单聊 C2C、频道 @ 消息和频道私信。受官方平台策略影响，群和频道能力是否能立即联调取决于账号、机器人类型和平台审核状态。
:::

## 准备凭据

在 QQ 开放平台创建机器人后，准备两项凭据：

| 变量 | 说明 |
|------|------|
| `QQBOT_APPID` | 机器人 AppID |
| `QQBOT_SECRET` | 机器人 AppSecret |

可以写入本地 `.env`、系统环境变量，或部署平台提供的环境变量配置：

```text
QQBOT_APPID=你的 AppID
QQBOT_SECRET=你的 AppSecret
```

`qimenbotd` 启动时会自动加载项目根目录下的 `.env`，配置文件里的 `${QQBOT_APPID}`、`${QQBOT_SECRET}` 会被替换成环境变量值。

## 配置 Bot 实例

框架默认读取 `config/base.toml`。如果要临时使用其他配置文件，可以设置 `QIMEN_CONFIG_PATH`：

```bash
QIMEN_CONFIG_PATH=config/dev.toml cargo run -p qimenbotd
```

最小配置如下：

```toml
[[bots]]
id = "qq-official"
protocol = "qq-official"
transport = "gateway"
enabled = true

appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
sandbox = false

intents = ["public_messages", "public_guild_messages", "direct_message"]
enabled_modules = ["command", "admin"]
owners = []
admins = []
```

字段说明：

| 字段 | 说明 |
|------|------|
| `protocol` | 必须为 `qq-official` |
| `transport` | 必须为 `gateway` |
| `appid` / `secret` | 建议从环境变量注入 |
| `sandbox` | 是否使用沙箱环境 |
| `intents` | 订阅官方 Gateway 事件 |
| `owners` / `admins` | 使用字符串 ID，可填 `openid`、`member_openid` 或频道用户 ID |

常用 intent：

| intent | 事件 |
|--------|------|
| `public_messages` | QQ 群 @ 消息、QQ 单聊 C2C 消息 |
| `public_guild_messages` | 频道 @ 消息 |
| `direct_message` | 频道私信消息 |

::: tip 配置模板
`config/bots/qq-official.toml` 是参考模板，不会被框架自动加载。实际运行时请将同等配置放入 `config/base.toml`，或通过 `QIMEN_CONFIG_PATH` 指向自定义配置文件。
:::

## 启动验证

启动守护进程：

```bash
cargo run -p qimenbotd
```

连接成功时会看到类似日志：

```text
registered bot instance bot_id=qq-official protocol=QqOfficial transport=Gateway
connecting to QQ official Gateway bot_id=qq-official
QQ official Gateway connected bot_id=qq-official
```

先用 QQ 单聊发送 `/ping` 做最小闭环。回复成功时会出现：

```text
received QQ official event bot_id=qq-official kind=Message
executed QQ official action bot_id=qq-official
```

如果暂时无法拉群或频道，可以先保留 `public_guild_messages` 和 `direct_message`，等测试环境可用后再验证 QQ 群 @、频道 @ 和频道私信。

## 插件兼容说明

官方 QQ Bot 事件不会暴露传统 QQ 号。插件需要按字符串 ID 处理用户和会话：

```rust
let sender = ctx.sender_id().unwrap_or("unknown");
let chat = ctx.chat_id().unwrap_or("unknown");
```

不同场景的 ID 映射：

| 场景 | `ctx.sender_id()` | `ctx.chat_id()` / `ctx.group_id()` |
|------|-------------------|------------------------------------|
| QQ 群 @ | `member_openid` | `group_openid` |
| QQ 单聊 C2C | `user_openid` | `user_openid` |
| 频道 @ | 频道用户 ID | `channel_id` |
| 频道私信 | 频道用户 ID | `guild_id` |

不要在官方 Bot 插件里依赖 `sender_id_i64()`、`group_id_i64()` 或真实 QQ 号；这些方法主要用于 OneBot 这类数字 ID 协议。`onebot_actions()` 中的群管理、群资料、禁言等接口也是 OneBot 专用能力，不等价于官方 OpenAPI。

## 富文本测试

开发配置可以加载 `example-message` 模块，用下面的命令测试官方富文本能力：

| 命令 | 能力 |
|------|------|
| `/qq-md` | Markdown content |
| `/qq-md-template [template_id]` | Markdown 模板参数 |
| `/qq-keyboard` | Markdown + 自定义 Keyboard |
| `/qq-keyboard-template [keyboard_id]` | Markdown + 模板 Keyboard |
| `/qq-ark` | Ark payload |
| `/qq-embed` | Embed payload |
| `/qq-media image <url>` | QQ 群/C2C 图片 media 上传 |
| `/qq-media record <url>` | QQ 群/C2C 语音 media 上传 |
| `/qq-media video <url>` | QQ 群/C2C 视频 media 上传 |
| `/qq-media file <url>` | QQ 群/C2C 文件 media 上传 |

Ark、Embed 主要面向频道消息；QQ 群和 C2C 的图片、语音、视频、文件会先调用官方 `/files` 上传，再以 `msg_type = 7` 发送。

## 常见问题

### 鉴权失败

检查 `QQBOT_APPID` 和 `QQBOT_SECRET` 是否为空，配置文件是否把占位符写成普通文本，当前运行目录是否能加载 `.env`。

### Gateway 已连接但收不到消息

检查开放平台事件订阅和配置中的 `intents`。QQ 群与 C2C 需要 `public_messages`，频道 @ 需要 `public_guild_messages`，频道私信需要 `direct_message`。

### 回复失败或权限不足

官方 OpenAPI 会按场景区分频道、群、C2C、DMS endpoint。确认机器人拥有对应场景的发送权限，并确认事件来源类型和发送 action 路由一致。

### Ark 或 Embed 返回 invalid content

这通常和官方能力开放、消息结构、场景支持范围有关。先用 `/qq-md` 或普通文本确认发送链路正常，再检查 Ark、Embed 是否已在目标场景通过官方审核或配置。

### 触发频控

429 会被框架归类为频控错误，并对 bot + route 做短期 backoff。降低主动发送频率，避免在 backoff 期间重复推送同一路由消息。

### 被动回复过期

官方平台对被动回复窗口有限制。插件收到消息后应尽快回复；耗时任务建议先返回处理中提示，再用允许的主动消息能力补发结果。
