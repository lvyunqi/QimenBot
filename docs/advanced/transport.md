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

官方 QQ Bot 与 OneBot 的传输模型不同：事件通过 Gateway WebSocket 下发，发送消息、上传媒体、撤回消息通过 HTTP OpenAPI 完成。因此配置上使用 `protocol = "qq-official"` 和 `transport = "gateway"`。

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
intents = ["public_messages", "public_guild_messages", "direct_message"]
```

### 工作流程

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

### Intents

| intent | 覆盖事件 |
|--------|----------|
| `public_messages` | QQ 群 @ 消息、QQ 单聊 C2C 消息 |
| `public_guild_messages` | 频道 @ 消息 |
| `direct_message` | 频道私信消息 |

没有开启对应 intent 时，Gateway 能连接成功，但收不到相关事件。

### 错误与频控

官方 OpenAPI 发送失败会被归一化为失败动作响应，Gateway 会话不会因此断开。429 频控会读取 `retry_after` 信息并对 bot + route 做短期 backoff；backoff 期间同路由发送会直接返回失败响应，避免持续撞频控。

## Echo 关联

WebSocket 传输中，框架使用 `echo` 字段将请求与响应关联：

```json
// 发送请求
{"action": "send_msg", "params": {...}, "echo": "req-001"}

// 收到响应
{"status": "ok", "data": {...}, "echo": "req-001"}
```

框架内部维护一个 pending 请求映射表，根据 `echo` 值将响应路由到对应的等待者。

## Access Token 鉴权

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

官方 QQ Bot Gateway 客户端位于 `qimen-transport-qqbot`：

```rust
pub struct QqBotGatewayClient {
    // 维护 access token、Gateway URL、session_id、seq、heartbeat 和 shard 状态
}
```

它复用底层 WebSocket 能力，但把官方协议相关的 token、opcode、Identify、Resume、Heartbeat、OpenAPI endpoint 和错误分类都限制在 `qimen-transport-qqbot` 内，避免污染通用 WebSocket 传输层。

## 选择建议

| 场景 | 推荐 |
|------|------|
| 本地开发 | WS 正向（配置最简单） |
| 生产部署（同机器） | WS 正向 |
| 生产部署（跨网络） | WS 反向（框架在公网） |
| 不需要实时推送 | HTTP |
| 需要高可靠性 | WS 正向 + 自动重连 |
| 接入官方 QQ Bot | Gateway |
