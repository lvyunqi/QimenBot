# QimenBot 动态插件示例

本项目是一个独立构建的 Rust `cdylib`，演示 QimenBot 动态插件 API 0.4。生成物是 Linux 的 `.so`、Windows 的 `.dll` 或 macOS 的 `.dylib`，由 QimenBot 在运行时加载。

示例同时覆盖 API 0.4 实时主动推送和 API 0.1 至 0.3 的兼容发送路径。

## 在 QimenBot 仓库外开发

动态插件不需要引用本地 QimenBot 源码。在任意目录创建独立 crate 后，可以直接使用 crates.io 依赖：

```toml
[package]
name = "qimen-dynamic-plugin-myplugin"
version = "0.1.0"
edition = "2024"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "0.1.10"
qimen-dynamic-plugin-derive = "0.1.10"
abi_stable = "0.11"
serde_json = "1"
```

仓库外项目不需要 `[workspace]`。如果把插件目录放在 QimenBot 仓库内、但不加入主 workspace，则像本示例一样增加一个空的 `[workspace]` 表。

crate 发布版本 `0.1.10` 与动态插件 ABI API `0.4` 是两套版本。需要实时主动推送时必须显式写出 `api = "0.4"`；未声明 `api` 时，过程宏仍生成兼容旧宿主的 API 0.3 插件。

## 快速开始

### 1. 编译

动态插件不属于根 workspace，必须进入插件目录单独构建：

```bash
cd plugins/qimen-dynamic-plugin-example
cargo build --release
```

### 2. 部署

把与宿主操作系统和 CPU 架构匹配的动态库复制到 QimenBot 的 `plugin_bin_dir`：

```bash
# Linux
cp target/release/libqimen_dynamic_plugin_example.so ../../plugins/bin/

# macOS
cp target/release/libqimen_dynamic_plugin_example.dylib ../../plugins/bin/

# Windows PowerShell
Copy-Item target/release/qimen_dynamic_plugin_example.dll ../../plugins/bin/
```

### 3. 加载

启动 QimenBot，或在 Bot 中执行 `/plugins reload` 热重载动态库。

## 本示例包含

| 功能 | 说明 |
|---|---|
| `greet`（别名 `hi`、`hello`） | 读取命令发送者信息并返回文本 |
| `legacy-notify` | 演示 API 0.1 至 0.3 的回调后 flush 发送路径 |
| `proactive-send` | 显式指定 Bot 和目标，通过 API 0.4 实时入队 |
| `#[pre_handle]` | 记录收到的消息并允许继续分发 |
| `GroupPoke`、`PrivatePoke` | 演示动态系统事件路由 |
| `#[init]` / `#[shutdown]` | 启动后台推送线程，并在卸载前停止和 `join` |

插件声明如下：

```rust
use abi_stable_host_api::*;
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "dynamic-example", version = "0.1.0", api = "0.4")]
mod example {
    use super::*;

    #[command(name = "greet", description = "Greet the sender", aliases = "hi,hello")]
    fn greet(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(&format!("Hello, {}!", req.sender_id))
    }
}
```

过程宏会生成插件描述符、命令和事件回调、生命周期函数，以及 API 0.4 所需的 Host API bind/unbind 导出。

## 配置后台实时推送

为示例插件创建本地配置 `config/plugins/dynamic-example.toml`：

```toml
[background_push]
bot_id = "qq-main"
kind = "group"
target_id = "123456"
message = "API 0.4 background push"
interval_secs = 60
```

`bot_id` 必须对应 QimenBot 配置中的一个启用 Bot。示例线程在 `init` 后立即尝试发送，此后按 `interval_secs` 间隔继续发送，不依赖命令、事件或 Heartbeat。

`kind` 支持以下目标：

| kind | target_id | guild_id |
|---|---|---|
| `private` | OneBot `user_id` 或 QQ 官方 `openid` | 不需要 |
| `group` | OneBot `group_id` 或 QQ 官方 `group_openid` | 不需要 |
| `channel` | `channel_id` | OneBot 必填；QQ 官方可省略 |
| `channel_private` | OneBot `user_id`；QQ 官方 `guild_id` | OneBot 必填 |

OneBot 频道示例：

```toml
[background_push]
bot_id = "qq-reverse"
kind = "channel"
target_id = "channel-100"
guild_id = "guild-200"
message = "频道实时通知"
interval_secs = 60
```

`config/plugins/*.toml` 是部署环境的本地配置，不应提交到框架仓库。

## API 0.4 实时发送

纯文本私聊或群聊可以使用 `BotApi::for_bot`：

```rust
use abi_stable_host_api::{BotApi, SendEnqueueStatus};

let status = BotApi::for_bot("qq-main")
    .send_group_msg("123456", "实时通知");

if status != SendEnqueueStatus::Accepted {
    eprintln!("宿主未接受请求: {status:?}");
}
```

富消息、频道上下文和发送选项使用 `SendBuilder`：

```rust
let status = SendBuilder::channel("channel-100")
    .guild_id("guild-200")
    .bot("qq-reverse")
    .text("频道通知")
    .try_send();
```

`try_send()` 必须先调用 `.bot(...)`。宿主不会选择最近事件的 Bot，也不会在多个 Bot 中自动挑选一个。

实时接口返回以下稳定状态：

| 状态 | 含义 |
|---|---|
| `Accepted` | 宿主已经复制请求并接受入队 |
| `HostUnavailable` | Host API 尚未绑定或当前不可用 |
| `InvalidRequest` | 请求字段、目标类型或 JSON 无效 |
| `BotNotFound` | `bot_id` 不存在 |
| `BotDisabled` | Bot 已配置但被禁用 |
| `QueueFull` | 对应 Bot 的有界队列已满 |
| `HostShuttingDown` | Runtime 正在关闭，不再接受新请求 |

`Accepted` 只确认入队，不等待网络响应。每个 Bot 独立保序，实际开始发送后的网络失败不会自动重试，以避免服务端已收到消息但响应丢失时产生重复发送。

宿主队列可在 `config/base.toml` 中配置：

```toml
[official_host.proactive_send]
queue_capacity = 256
offline_ttl_secs = 60
```

`offline_ttl_secs = 0` 表示 Bot 离线时立即丢弃请求。

## 旧发送路径仍然兼容

API 0.1 至 0.3 的接口没有改变：

```rust
BotApi::send_group_msg("123456", "回调结束后发送");

SendBuilder::private("10001")
    .text("回调结束后发送")
    .send();
```

这两个调用写入插件侧旧队列，宿主在当前动态插件回调结束后通过 `qimen_plugin_flush_sends` 取走并发送。后台线程需要实时发送时，应使用 `BotApi::for_bot(...)` 或 `.bot(...).try_send()`。

## 后台线程和安全卸载

API 0.4 的 Host API 在插件 `init` 前完成绑定，所以 `init` 创建的线程可以立即主动发送。插件必须在 `shutdown` 中通知线程退出并等待 `join` 完成；宿主随后才会 unbind Host API 并卸载动态库。

本示例使用 `AtomicBool`、`thread::park_timeout` 和保存的 `JoinHandle` 实现这一顺序。不要让插件线程在 `shutdown` 返回后继续执行动态库中的代码。

## 参考文档

- [动态插件开发](../../docs/plugin/dynamic.md)
- [API 0.4 实时主动推送](../../docs/advanced/dynamic-proactive-send-v04.md)
- [动态插件 FFI API](../../docs/api/ffi-api.md)
