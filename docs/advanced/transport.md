# 传输层

QimenBot 支持多种传输方式连接 OneBot 实现。传输层负责底层通信，与上层协议逻辑解耦。

## 传输模式概览

| 模式 | 方向 | 说明 | 适用场景 |
|------|------|------|---------|
| **WS 正向** | 框架 → OneBot | 框架主动连接 OneBot 的 WebSocket | 最常用，配置简单 |
| **WS 反向** | OneBot → 框架 | 框架监听，OneBot 连接过来 | 框架在公网时 |
| **HTTP API** | 框架 → OneBot | 通过 HTTP 调用 API | 简单场景 |
| **HTTP POST** | OneBot → 框架 | OneBot 推送事件到框架 | 配合 HTTP API 使用 |

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
- 一个框架监听端口，多个 OneBot 实现连接
- 防火墙只允许出站连接的环境

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

    /// 接收下一个事件
    pub async fn next_event(&mut self) -> Option<String>;
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

## 选择建议

| 场景 | 推荐 |
|------|------|
| 本地开发 | WS 正向（配置最简单） |
| 生产部署（同机器） | WS 正向 |
| 生产部署（跨网络） | WS 反向（框架在公网） |
| 不需要实时推送 | HTTP |
| 需要高可靠性 | WS 正向 + 自动重连 |
