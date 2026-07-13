# 动态插件 API 0.5 Webhook Gateway

QimenBot v0.1.11 在 Runtime 中提供框架级 HTTP Webhook Gateway。动态插件可以声明精确的 HTTP 路由，由宿主统一监听端口、限制请求大小和并发数，并把请求转换成 ABI 稳定的 `WebhookRequest` 调用插件。

Webhook 不依赖 OneBot 或 QQ 官方 Gateway 收到新事件。即使没有 Bot 在线、没有 Heartbeat，也可以接收 HTTP 请求；只有插件要继续向 Bot 推送消息时，才要求目标 Bot 已配置并通过 `account_id` 或实例 `bot_id` 显式选择。

仓库外插件可以直接使用 crates.io `0.1.12`：

```toml
[dependencies]
abi-stable-host-api = "0.1.12"
qimen-dynamic-plugin-derive = "0.1.12"
abi_stable = "0.11"
```

## 启用网关

网关默认关闭，并只建议先监听本机回环地址：

```toml
[official_host.webhook]
enabled = true
bind = "127.0.0.1:8088"
base_path = "/webhooks"
max_body_bytes = 1048576
request_timeout_ms = 5000
max_in_flight = 64
access_token = "replace-with-a-long-random-token"
```

| 配置项 | 默认值 | 说明 |
|---|---:|---|
| `enabled` | `false` | 是否启动 HTTP Gateway |
| `bind` | `127.0.0.1:8088` | TCP 监听地址；启用时必须是有效的 SocketAddr |
| `base_path` | `/webhooks` | 所有插件 Webhook 的公共前缀；不允许 query、fragment、末尾 `/`、`//`、`.`/`..` 路径段或通配符 |
| `max_body_bytes` | `1048576` | 单请求最大 body，必须大于 0 |
| `request_timeout_ms` | `5000` | 等待同步插件回调返回的时间，必须大于 0 |
| `max_in_flight` | `64` | 全局同时执行的 Webhook 回调数，必须大于 0 |
| `access_token` | 空 | 非空时要求 `Authorization: Bearer <token>` |

生产环境建议让 QimenBot 继续监听 `127.0.0.1`，再由 Caddy、Nginx 或其他反向代理负责 TLS、来源 IP 策略和公网限流。不要把空 token 的网关直接暴露到公网。

## 声明插件路由

Webhook 是 API 0.5 功能，插件必须显式声明 `api = "0.5"`：

```rust
use abi_stable_host_api::{WebhookRequest, WebhookResponse};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "build-events", version = "0.1.0", api = "0.5")]
mod plugin {
    use super::*;

    #[webhook(method = "POST", path = "/events")]
    fn receive(req: &WebhookRequest) -> WebhookResponse {
        let body = String::from_utf8_lossy(req.body.as_slice());
        eprintln!("query={} body={body}", req.query.as_str());
        WebhookResponse::text(202, "accepted")
    }
}
```

插件局部路径必须：

- 以 `/` 开头；
- 是精确静态路径，不支持 `*`、参数段或通配符；
- 不包含 query、fragment、`//` 或 `..` 路径穿越；
- 同一 HTTP method 和完整路径不能重复。

过程宏会把 method 规范为大写并生成独立导出，同时捕获 Webhook 回调中的 panic 并转换为 `500`，避免 panic 跨越 `extern C` FFI 边界导致宿主进程中止：

```text
qimen_plugin_webhook_descriptors_v1
```

旧 `PluginDescriptor` 的内存布局没有增加字段，因此 API 0.1 至 0.4 插件保持 ABI 兼容。

## 完整 URL 规则

宿主按以下规则自动加入插件命名空间：

```text
{base_path}/{plugin_id}{plugin_local_path}
```

上面的示例最终监听：

```text
POST /webhooks/build-events/events
```

插件之间即使都声明 `/events`，只要 `plugin_id` 不同，完整路径也不同。若同一插件重复声明相同 method/path，或最终路由冲突，网关安装路由失败。

## 请求与响应类型

`WebhookRequest` 字段固定为：

| 字段 | 内容 |
|---|---|
| `method` | HTTP method |
| `path` | 实际完整 URL path |
| `query` | 不含开头 `?` 的原始 query string |
| `headers_json` | 请求头 JSON；单值是字符串，多值是字符串数组 |
| `body` | 原始字节数组 |
| `remote_addr` | 宿主看到的 TCP peer 地址 |

`remote_addr` 在反向代理之后通常是代理地址。若业务依赖真实来源 IP，应只信任由受控代理写入的头，并在代理层限制客户端不能伪造该头。

响应示例：

```rust
WebhookResponse::text(200, "ok").with_headers_json(
    r#"{"content-type":"text/plain; charset=utf-8","x-event-id":"123"}"#,
)
```

插件可以设置状态码、响应 body 和 JSON 格式响应头。宿主会忽略非法头以及 `connection`、`transfer-encoding`、`content-length`、`upgrade` 等 hop-by-hop 头。

## 从 Webhook 主动发送 Bot 消息

Webhook 没有“当前 Bot”上下文，因此必须明确选择 Bot。OneBot 推荐在宿主配置中设置稳定的 `account_id`（Bot QQ / `self_id`），避免插件依赖可变的实例别名。下面的账号选择接口从 crate `0.1.12` 开始提供：

```rust
use abi_stable_host_api::{BotApi, SendEnqueueStatus};

let status = BotApi::for_account("2733944636")
    .send_group_msg("123456", "Webhook received");

if status != SendEnqueueStatus::Accepted {
    eprintln!("send was not accepted: {status:?}");
}
```

富消息或频道目标使用：

```rust
let status = SendBuilder::channel("channel-id")
    .guild_id("guild-id")
    .bot_account("2733944636")
    .text("Webhook received")
    .try_send();
```

按部署实例选择的 `BotApi::for_bot(...)` 和 `.bot(...)` 继续兼容，适合确实需要区分同一账号不同传输实例的情况。

不要在 Webhook 回调中使用旧 `BotApi::send_group_msg(...)` 或 `SendBuilder::send()`。这些接口写入“回调结束后 flush”的旧队列，无法推断 Webhook 应使用哪个 Bot；宿主会丢弃这类发送并记录警告。

`Accepted` 只表示宿主已复制并接受请求，不代表网络发送已经成功。实时发送的队列、离线 TTL 和协议目标映射见 [API 0.4+ 实时主动推送](/advanced/dynamic-proactive-send-v04)。

## 认证与第三方签名

`access_token` 是网关级统一 Bearer token，适合限制谁可以进入插件路由。第三方平台自己的 HMAC、时间戳、nonce、事件 ID 和重放保护仍由插件实现，因为每个平台的签名算法不同。

推荐验证顺序：

1. 从 `headers_json` 取签名、时间戳和事件 ID；
2. 对原始 `body` 计算签名，不要先把 JSON 重新序列化；
3. 使用常量时间比较；
4. 拒绝超出允许时间窗口的请求；
5. 对事件 ID 或 nonce 做短期去重；
6. 验证完成后才修改状态或提交主动发送。

## 超时、并发和热重载

- 超过 `max_body_bytes` 返回 `413`；
- Bearer token 错误返回 `401`；
- 路由不存在返回 `404`；
- 达到 `max_in_flight` 返回 `429`；
- 插件回调超过 `request_timeout_ms` 返回 `504`；
- 热重载暂停或插件不可用时返回 `503`；
- 插件错误或 panic 返回 `500`。

同步 FFI 代码无法被 Rust 安全地强制终止。因此 `504` 只表示 HTTP 客户端停止等待，不表示插件回调已经停止。超时后的回调仍在隔离的 blocking 线程中运行，并继续持有动态库生命周期读锁和并发 permit，直到真正返回。

执行 `/plugins reload` 时，Runtime 会：

1. 停止接收新 Webhook；
2. 等待所有在途回调真正返回；
3. 调用插件 `shutdown`、解绑 Host API 并卸载旧动态库；
4. 扫描新动态库；
5. 重新绑定 Host API、读取 `config/plugins/<plugin_id>.toml` 并调用 `init`；
6. 只为初始化成功的插件恢复 Webhook 路由。

插件自己的后台线程仍必须在 `shutdown` 中停止并 `join`。否则宿主无法安全卸载包含线程代码的动态库。

## 本地测试

```bash
curl -i \
  -H 'Authorization: Bearer replace-with-a-long-random-token' \
  -H 'Content-Type: application/json' \
  -d '{"event":"test"}' \
  'http://127.0.0.1:8088/webhooks/build-events/events?source=curl'
```

可先把 `bind` 保持为回环地址。确认插件日志、HTTP 状态和响应体正确后，再配置反向代理和第三方平台回调 URL。
