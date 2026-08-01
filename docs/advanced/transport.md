# 传输层

QimenBot 支持多种传输方式连接不同协议实现。传输层负责底层通信，与上层协议逻辑解耦；官方 QQ Bot 由于 Gateway 和 OpenAPI 强绑定，使用独立的协议传输封装。

## 传输模式概览

| 模式 | 方向 | 说明 | 适用场景 |
|------|------|------|---------|
| **WS 正向** | 框架 → OneBot | 框架主动连接 OneBot 的 WebSocket | 最常用，配置简单 |
| **WS 反向** | OneBot → 框架 | 框架监听，OneBot 连接过来 | 框架在公网时 |
| **HTTP API** | 框架 → OneBot | 通过 HTTP 调用 API | 简单场景 |
| **HTTP POST** | OneBot → 框架 | OneBot 推送事件到框架 | 配合 HTTP API 使用 |
| **Gateway** | 框架 → 官方 QQ Bot | Gateway 收事件，OpenAPI 发动作 | 官方 QQ Bot |

## 正向 WebSocket（推荐）

框架主动连接 OneBot 实现提供的 WebSocket 端点。

### 配置

```toml
[[bots]]
id        = "qq-main"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"   # OneBot WS 地址
# access_token = "your-token"       # 可选鉴权
```

### 工作流程

```
QimenBot                    OneBot 实现
    |                           |
    |--- WebSocket CONNECT ---->|
    |<-- 101 Switching ---------|
    |                           |
    |<-- Event (JSON) ----------|  收到消息
    |--- Action (JSON) -------->|  发送操作
    |<-- Action Response -------|  操作结果
    |                           |
```

### 自动重连

连接断开后框架会自动重连，使用指数退避策略：

| 重试次数 | 等待时间 |
|---------|---------|
| 1 | 1 秒 |
| 2 | 2 秒 |
| 3 | 4 秒 |
| 4 | 8 秒 |
| ... | ... |
| 最大 | 60 秒 |

连接稳定运行一段时间后退避计数器自动重置。

### TLS 支持

使用 `wss://` 前缀启用 TLS 加密连接：

```toml
endpoint = "wss://bot.example.com:3001"
```

## 反向 WebSocket

框架监听端口，等待 OneBot 实现连接过来。

### 配置

```toml
[[bots]]
id        = "qq-reverse"
protocol  = "onebot11"
transport = "ws-reverse"
bind      = "0.0.0.0:6701"          # 监听地址
path      = "/onebot/reverse"       # WebSocket 路径
# access_token = "your-token"       # 可选鉴权
```

### 工作流程

```
QimenBot                    OneBot 实现
    |                           |
    |<-- WebSocket CONNECT -----|  OneBot 主动连接
    |--- 101 Switching -------->|
    |                           |
    |<-- Event (JSON) ----------|
    |--- Action (JSON) -------->|
    |                           |
```

### 适用场景

- 框架部署在公网服务器，OneBot 实现在内网
- OneBot 实现断线后主动重连同一个监听地址
- 防火墙只允许出站连接的环境

## 使用 qimenctl 模拟 OneBot 11 客户端

当命令没有回复时，可以让 `qimenctl` 临时充当 OneBot 11 实现端，不经过真实 QQ 客户端直接验证框架内部链路。模拟器会完成标准反向 WebSocket 握手、Token 鉴权、`lifecycle.connect` 上报、array 格式消息事件上报，并为框架发出的每个 Action 自动回写相同 `echo` 的成功响应。

它覆盖的实际链路如下：

```text
qimenctl 模拟事件
  -> 反向 WebSocket
  -> OneBot 11 解码
  -> Runtime 命令匹配
  -> 静态或动态插件回调
  -> send_msg Action
  -> qimenctl echo 响应
```

::: warning 会话占用
测试时应先断开真实 OneBot 客户端，或者为测试单独配置一个 `ws-reverse` Bot、端口和路径。不要让模拟器和真实客户端同时承担同一个 Bot 的反向 WebSocket 会话。
:::

### 按 Bot 配置测试

先启动 `qimenbotd`，再在另一个终端发送私聊事件：

```bash
cargo run -p qimenctl -- simulate-onebot11 \
  --bot qq-reverse \
  --message /ping \
  --user-id 10000 \
  --self-id 10001
```

`--bot` 会从 `config/base.toml` 读取对应 Bot。监听地址为 `0.0.0.0` 或 `[::]` 时，CLI 会自动改用本机回环地址连接；Bot 必须启用并使用 `protocol = "onebot11"`、`transport = "ws-reverse"`。

群聊事件增加 `--group-id`：

```bash
cargo run -p qimenctl -- simulate-onebot11 \
  --bot qq-reverse \
  --message /ping \
  --user-id 10000 \
  --self-id 10001 \
  --group-id 20000
```

### 按显式端点测试

显式端点模式不读取 `config/base.toml`，适合从独立目录使用已构建的 `qimenctl`，或连接专用测试监听器。Token 建议通过环境变量读取，避免出现在命令历史和进程参数中：

```bash
export QQ_REVERSE_TOKEN='replace-me'
./qimenctl simulate-onebot11 \
  --endpoint ws://127.0.0.1:6710/onebot/qimenbot \
  --access-token-env QQ_REVERSE_TOKEN \
  --message /ping \
  --user-id 10000 \
  --self-id 10001
```

也可以精确重放一个 OneBot 11 JSON 对象：

```bash
./qimenctl simulate-onebot11 \
  --endpoint ws://127.0.0.1:6710/onebot/qimenbot \
  --raw-event ./test-event.json \
  --no-lifecycle
```

`--message` 与 `--raw-event` 二选一；`--bot` 与 `--endpoint` 也二选一。默认等待首个 Action 10 秒，收到首个 Action 后继续收集 750 毫秒，可通过 `--timeout-secs` 和 `--idle-millis` 调整。

### 如何看测试结果

| 现象 | 优先检查 |
|------|----------|
| WebSocket 握手失败 | 监听端口、路径、Token、防火墙，以及服务是否已启动 |
| 框架日志没有 `received OneBot event` | 事件未进入 Runtime，检查连接和事件 JSON |
| 有事件日志但没有命令命中日志 | 命令名、前缀、作用域、插件描述符中的 commands/aliases |
| 已命中命令但 CLI 收不到 Action | 插件回调、FFI 调用、返回值或发送队列 |
| CLI 打印 Action 并显示 acknowledged | 从事件到发送响应的完整框架链路已通过 |

该工具故意不新增公网调试 HTTP 接口，因此不会在生产服务上额外暴露事件注入入口。

## HTTP 传输

HTTP 模式将事件接收和 API 调用分为两个方向。

### 配置

```toml
[[bots]]
id        = "qq-http"
protocol  = "onebot11"
transport = "http"
endpoint  = "http://127.0.0.1:5700"  # OneBot HTTP API 地址
# bind    = "0.0.0.0:5701"           # 事件接收地址
```

### 工作流程

```
事件推送 (HTTP POST):
OneBot --POST /event--> QimenBot

API 调用 (HTTP):
QimenBot --POST /send_msg--> OneBot
         <-- JSON Response --
```

## 官方 QQ Bot Gateway

官方 QQ Bot 与 OneBot 的传输模型不同：

- Gateway WebSocket 只负责下发事件和维护会话。
- OpenAPI HTTP 负责发送消息、上传媒体、撤回消息和确认互动。
- 鉴权使用 AppID 和 AppSecret 换取的短期 access token，不使用 OneBot 的 `access_token`。

因此官方机器人固定使用 `protocol = "qq-official"` 和 `transport = "gateway"`。

### 配置

```toml
[[bots]]
id        = "qq-official"
protocol  = "qq-official"
transport = "gateway"
enabled   = true

appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
sandbox = false
intents = ["GROUP_AND_C2C_EVENT", "PUBLIC_GUILD_MESSAGES", "DIRECT_MESSAGE"]
```

`appid` 和 `secret` 应通过环境变量注入。`sandbox = true` 时，OpenAPI 基地址改为官方沙箱地址；Gateway URL 仍由 `/gateway/bot` 动态获取。

### 建立连接的顺序

```
QimenBot                         QQ Bot OpenAPI
    |                                  |
    |--- POST /app/getAppAccessToken ->|  AppID + Secret
    |<-- access_token -----------------|
    |--- GET /gateway/bot ------------>|  获取 Gateway URL
    |<-- wss://... --------------------|
    |                                  |
    |=== WebSocket Gateway ============|
    |<-- Hello ------------------------|
    |--- Identify / Resume ----------->|
    |<-- Dispatch(Message/Notice) -----|  收到事件
    |--- Heartbeat ------------------->|
    |<-- Heartbeat ACK ----------------|
    |                                  |
    |--- POST /messages -------------->|  插件回复或主动发送
    |<-- message response -------------|
```

实际执行过程如下：

1. `POST https://bots.qq.com/app/getAppAccessToken`，用 AppID 和 AppSecret 换取 access token。
2. 缓存 token，并在官方 `expires_in` 到期前 60 秒重新获取。
3. `GET /gateway/bot`，取得本次应连接的 `wss://` 地址和建议分片数。
4. WebSocket 建立后等待 `Hello`，读取 `heartbeat_interval`。
5. 没有旧会话时发送 `Identify`；已有 `session_id` 时发送 `Resume`。
6. 收到 `READY` 后保存 `session_id`，收到业务 Dispatch 后交给适配器和运行时。
7. 运行时成功处理 Dispatch 后才提交该帧的序号，用于后续 Resume。

token 请求使用 JSON 字段 `appId` 和 `clientSecret`。OpenAPI 请求使用 `Authorization: QQBot <token>`，并带 `X-Union-Appid`。日志不会主动输出 Secret 或完整 token。

### Gateway opcode

| opcode | 名称 | QimenBot 行为 |
|--------|------|---------------|
| `0` | Dispatch | 解析 `t`、`s`、`id` 和 `d`，交给协议适配器 |
| `1` | Heartbeat | 立即发送带当前序号的 Heartbeat |
| `2` | Identify | 新会话连接时由客户端发送 |
| `6` | Resume | 有 `session_id` 时由客户端发送 |
| `7` | Reconnect | 结束当前连接并进入重连 |
| `9` | Invalid Session | 清空 `session_id` 和序号，下次重新 Identify |
| `10` | Hello | 读取心跳间隔 |
| `11` | Heartbeat ACK | 清除等待 ACK 状态 |

`READY` 和 `RESUMED` 都作为 `EventKind::Meta` 进入归一化事件层。`READY` 还会更新 Gateway 会话中的 `session_id` 和分片信息。

### 心跳、序号和 Resume

客户端按 `Hello.heartbeat_interval` 定时发送：

```json
{"op": 1, "d": 123}
```

`d` 是最后一条已成功处理的 Dispatch 序号；还没有序号时为 `null`。发送心跳后如果在下一次心跳前仍未收到 ACK，运行时会主动重连，避免连接表面存在但已经无法收事件。

Resume 请求包含：

```json
{
  "op": 6,
  "d": {
    "token": "QQBot token",
    "session_id": "READY 返回的会话 ID",
    "seq": 123
  }
}
```

以下情况会进入重连：

- Gateway 发送 `Reconnect`。
- 收到 `Invalid Session`。
- WebSocket 关闭或事件流结束。
- 超过空闲超时。
- Heartbeat ACK 超时。
- 当前会话发生传输或事件处理错误。

有可恢复会话时优先 Resume；`Invalid Session` 会清空恢复信息并重新 Identify。外层使用指数退避，稳定连接达到阈值后重置退避时间。

### 分片状态

`/gateway/bot` 可能返回建议分片数。当前运行时校验返回值，但每个 Bot 实例使用一个 `[0, 1]` 非分片连接；当平台建议多个分片时会输出警告：

```text
QQ official Gateway recommends multiple shards; using one unsharded connection
```

大规模机器人需要真正多分片时，不能把这条警告当作已完成分片接入，应在部署前扩展 Bot 实例和会话调度策略。

### Intents 配置

配置值会在启动时转换为 Gateway 位掩码。未知名称会直接导致配置校验失败。

| intent | 覆盖事件 |
|--------|----------|
| `GUILDS` | 频道创建、更新、删除 |
| `GUILD_MEMBERS` | 频道成员变更 |
| `GUILD_MESSAGES` | 私域频道消息 |
| `GUILD_MESSAGE_REACTIONS` | 频道消息表情回应 |
| `DIRECT_MESSAGE` | 频道私信消息 |
| `OPEN_FORUM_EVENT` | 开放论坛事件 |
| `AUDIO_OR_LIVE_CHANNEL_MEMBER` | 音视频或直播子频道成员进出 |
| `GROUP_AND_C2C_EVENT` | QQ 群 @/全量消息、QQ 单聊 C2C 消息 |
| `INTERACTION` | 按钮、快捷菜单等互动事件 |
| `MESSAGE_AUDIT` | 消息审核结果 |
| `FORUMS_EVENT` | 论坛事件 |
| `AUDIO_ACTION` | 音频事件 |
| `PUBLIC_GUILD_MESSAGES` | 公域频道 @ 和相关频道消息 |

名称不区分大小写。兼容别名包括 `public_messages`（等价于 `GROUP_AND_C2C_EVENT`）和 `forums`（等价于 `FORUMS_EVENT`）。

本地位掩码和平台授权缺一不可。没有开启对应 intent 时，Gateway 仍可能连接成功，但不会下发目标事件；申请了机器人未获授权的高权限 intent 时，Identify 也可能被平台拒绝。

### 消息事件映射

| Gateway 事件 | 归一化 `message_type` | 会话目标 |
|--------------|--------------------------|----------|
| `GROUP_AT_MESSAGE_CREATE` | `group` | `group_openid` |
| `GROUP_MESSAGE_CREATE` | `group` | `group_openid` |
| `C2C_MESSAGE_CREATE` | `private` | `user_openid` |
| `AT_MESSAGE_CREATE` | `channel` | `channel_id` |
| `MESSAGE_CREATE` | `channel` | `channel_id` |
| `DIRECT_MESSAGE_CREATE` | `channel_private` | `guild_id` |

原始 Dispatch 的 `d` 对象保存在 `raw_json.qqbot_payload`，`t` 保存为 `extensions.event_type`。消息 ID 全程按字符串保留。

全量群消息可能通过 `message_scene.ext` 携带 `msg_idx=...`。适配器提取后放入 `extensions.msg_idx`，运行时将它加入去重键，避免同一个消息 ID 下的不同投递片段相互覆盖。

### OpenAPI 路由

| 动作 | HTTP 请求 |
|------|-----------|
| 获取 Gateway | `GET /gateway/bot` |
| 发送频道消息 | `POST /channels/{channel_id}/messages` |
| 发送 QQ 群消息 | `POST /v2/groups/{group_openid}/messages` |
| 上传 QQ 群媒体 | `POST /v2/groups/{group_openid}/files` |
| 发送 C2C 消息 | `POST /v2/users/{openid}/messages` |
| 上传 C2C 媒体 | `POST /v2/users/{openid}/files` |
| 发送频道私信 | `POST /dms/{guild_id}/messages` |
| 确认互动 | `PUT /interactions/{interaction_id}` |
| 撤回消息 | 对对应消息 URL 发送 `DELETE` |

发送和撤回路由都由归一化会话类型决定。不能拿 `group_openid` 调频道 endpoint，也不能把 DMS 的 `guild_id` 当作 C2C `openid`。

### 消息 payload 约束

QQ 群和 C2C 请求使用 `msg_type`：

| `msg_type` | 内容 | 支持场景 |
|------------|------|----------|
| `0` | 文本 | QQ 群、C2C |
| `2` | Markdown | QQ 群、C2C |
| `6` | Input notify | C2C |
| `7` | 媒体 `file_info` | QQ 群、C2C |
| `8` | Card | QQ 群 |

频道和 DMS 使用各自的消息结构，不发送 `msg_type`。编码器还会执行以下检查：

- `msg_type = 2` 必须带 `markdown`，同时移除普通 `content`。
- `msg_type = 6` 必须带 `input_notify`。
- `msg_type = 7` 必须带已上传的 `media` 或可上传 URL。
- `msg_type = 8` 必须带 `card`。
- Ark 和 Embed 不能同时存在，且不用于群/C2C 请求。
- 群/C2C 专属的 Card、Input notify 和 media 字段不会发到频道或 DMS。

### 被动回复字段

官方接口使用三种互斥依据：

- `msg_id`：回复一条聊天消息。
- `event_id`：响应没有消息 ID 的事件。
- `is_wakeup = true`：允许场景中的主动唤醒请求。

三者不能同时发送。QimenBot 回复消息时优先使用 `msg_id`；只有事件没有消息 ID 时才退回 `event_id`。

群和 C2C 对同一条来信的多次回复还需要正整数 `msg_seq`。运行时按 `bot + route + msg_id` 分配递增序号，状态保留一小时；显式提供序号时会校验为正数，并使后续自动序号从更大值继续。频道和 DMS 不发送 `msg_seq`。

### 媒体上传

群和 C2C 的图片、视频、语音和文件使用两段式流程：

```text
Message segment URL
  -> /files { file_type, url, srv_send_msg }
  <- { file_info, ... }
  -> /messages { msg_type: 7, media: { file_info } }
```

文件类型映射：

| 消息段 | `file_type` |
|--------|-------------|
| `image` | `1` |
| `video` | `2` |
| `record` / `audio` / `voice` | `3` |
| `file` | `4` |

高层适配只把 `http://` 和 `https://` 识别为可上传 URL。上传响应必须包含 `file_info`，发送阶段只携带这个字段，不把完整上传响应原样塞回消息体。

`srv_send_msg = true` 只允许主动、纯媒体的群/C2C 上传，不能同时携带文本、Markdown、Keyboard、回复 ID 或事件 ID。其他组合会在请求发出前被拒绝。

### 撤回和互动 ACK

撤回支持四个场景：

| 场景 | DELETE 路由 |
|------|-------------|
| 频道 | `/channels/{channel_id}/messages/{message_id}` |
| QQ 群 | `/v2/groups/{group_openid}/messages/{message_id}` |
| C2C | `/v2/users/{openid}/messages/{message_id}` |
| DMS | `/dms/{guild_id}/messages/{message_id}` |

频道和 DMS 撤回可携带 `hidetip`。目标 ID 和消息 ID 都按字符串传递。

对 `INTERACTION_CREATE` 中要求 ACK 的类型，运行时自动调用 `PUT /interactions/{interaction_id}` 并发送 `{"code": 0}`。ACK 失败会记录警告，但不会伪装成聊天回复。

### 错误与频控

OpenAPI 非 2xx 响应会保留以下信息：

- HTTP 状态码。
- `code`、`err_code` 或 `errcode`。
- `message`、`errmsg` 或 `msg`。
- `retry_after` / `retry_after_ms`。
- `trace_id`。

错误会归类为：

| 分类 | 常见原因 |
|------|----------|
| `Authentication` | token、AppID 或 Secret 无效 |
| `Permission` | 机器人未开通目标接口或消息类型 |
| `RateLimited` | HTTP 429 或官方频控错误码 |
| `NotFound` | endpoint、目标或消息不存在 |
| `BadRequest` | payload 字段、消息类型或 ID 错误 |
| `Server` | 官方 5xx |
| `Unknown` | 无法识别的官方错误响应 |

连接失败和请求超时仍作为传输错误返回。OpenAPI 发送失败会成为失败动作响应，不会因为一次接口错误重启 Gateway 会话。`RateLimited` 会读取 `retry_after`，按 `bot + route` 设置 1 秒至 60 秒的 backoff；backoff 期间同一路由直接返回失败，成功发送后清除对应状态。

排查官方接口问题时，应记录 endpoint、HTTP 状态、错误码、分类和 `trace_id`。不要把 AppSecret 或完整 access token 一起复制到 Issue。

### 官方文档对照

- [WebSocket 事件链路](https://bot.q.qq.com/wiki/develop/api-v2/dev-prepare/event-emit/websocket.html)
- [消息发送和回复说明](https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/overview.html)
- [群全量消息事件](https://bot.q.qq.com/wiki/develop/api-v2/autogen/event/group_message_create.html)
- [群 @ 消息事件](https://bot.q.qq.com/wiki/develop/api-v2/autogen/event/group_at_message_create.html)
- [发送群消息](https://bot.q.qq.com/wiki/develop/api-v2/autogen/api/v2_groups_group_openid_messages.post.html)
- [发送 C2C 消息](https://bot.q.qq.com/wiki/develop/api-v2/autogen/api/v2_users_user_openid_messages.post.html)
- [互动事件 ACK](https://bot.q.qq.com/wiki/develop/api-v2/autogen/api/interactions_interaction_id.put.html)
- [OpenAPI 错误码](https://bot.q.qq.com/wiki/develop/api-v2/openapi/error/error.html)

## OneBot 11 Echo 关联

WebSocket 传输中，框架使用 `echo` 字段将请求与响应关联：

```json
// 发送请求
{"action": "send_msg", "params": {...}, "echo": "req-001"}

// 收到响应
{"status": "ok", "data": {...}, "echo": "req-001"}
```

框架内部维护一个 pending 请求映射表，根据 `echo` 值将响应路由到对应的等待者。

## OneBot 11 Access Token 鉴权

通过 `access_token` 字段配置鉴权：

```toml
access_token = "your-secret-token"
# 或使用环境变量
access_token = "${QQ_TOKEN}"
```

- **WS 正向** — Token 作为 URL 参数传递
- **WS 反向** — 验证连接时的 Authorization 头
- **HTTP** — 作为请求头或参数传递

## 传输层类型

### OneBot11ForwardWsClient

正向 WebSocket 客户端：

```rust
pub struct OneBot11ForwardWsClient {
    // 内部字段
}

impl OneBot11ForwardWsClient {
    /// 连接到 OneBot WS 端点
    pub async fn connect(endpoint: &str, access_token: Option<&str>) -> Result<Self>;

    /// 接收下一个事件
    pub async fn next_event(&mut self) -> Option<String>;

    /// 发送文本帧
    pub async fn send_text(&self, text: &str) -> Result<()>;

    /// 发送并等待 echo 响应
    pub async fn send_text_await_echo(
        &self,
        text: &str,
        echo: &str,
        timeout: Duration,
    ) -> Result<String>;
}
```

### WsReverseServer

反向 WebSocket 服务端：

```rust
pub struct WsReverseServer {
    // 内部字段
}

impl WsReverseServer {
    /// 绑定并监听
    pub async fn bind(config: WsReverseConfig) -> Result<Self>;

    /// 等待下一个完成鉴权和握手的连接
    pub async fn next_connection(&mut self) -> Option<OneBot11ReverseWsConnection>;
}

pub struct WsReverseConfig {
    pub bind: String,
    pub path: String,
    pub access_token: Option<String>,
}

impl OneBot11ReverseWsConnection {
    /// 接收下一个事件
    pub async fn next_event(&mut self) -> Option<String>;

    /// 发送 Action 并按 echo 等待响应
    pub async fn send_text_await_echo(
        &self,
        text: &str,
        echo: &str,
        timeout: Duration,
    ) -> Result<String>;
}
```

### ReconnectPolicy

重连策略：

```rust
pub struct ReconnectPolicy {
    pub initial_delay: Duration,                 // 初始等待时间
    pub max_delay: Duration,                     // 最大等待时间
    pub stable_connection_threshold: Duration,   // 稳定连接阈值
    pub idle_timeout: Duration,                  // 空闲超时
}
```

### QqBotGatewayClient

官方 QQ Bot 的 OpenAPI 和 Gateway 客户端都位于 `qimen-transport-qqbot`：

```rust
pub struct QqBotOpenApiClient {
    // HTTP client、access token 缓存和 OpenAPI 配置
}

pub struct QqBotGatewayClient {
    // WebSocket、session_id、seq、heartbeat 和 shard 状态
}
```

Gateway 客户端复用底层 WebSocket 能力；OpenAPI 客户端负责 token、HTTP endpoint 和错误解析。官方 opcode、Identify、Resume、Heartbeat、OpenAPI 路由和错误分类都限制在 `qimen-transport-qqbot` 内，不进入通用 WebSocket 传输层。

## 选择建议

| 场景 | 推荐 |
|------|------|
| 本地开发 | WS 正向（配置最简单） |
| 生产部署（同机器） | WS 正向 |
| 生产部署（跨网络） | WS 反向（框架在公网） |
| 不需要实时推送 | HTTP |
| 需要高可靠性 | WS 正向 + 自动重连 |
| 接入官方 QQ Bot | Gateway |
