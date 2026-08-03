# 动态插件开发

## 先建立正确心智模型

动态插件是独立 Rust `cdylib`，宿主在运行时通过动态加载器读取描述符和回调。它不需要 QimenBot 主框架源码，可以放在完全独立的 Git 仓库中开发、测试和发布。

动态插件的边界：

- 回调是同步 FFI，不写 `async fn`，也不把 Tokio runtime 或 Rust 标准库对象跨 ABI 传递。
- 命令、事件、拦截器和 Webhook 由过程宏导出；网络发送通过宿主队列完成。
- `.so`、`.dll`、`.dylib` 只能加载到操作系统、CPU、C 运行时兼容的宿主。
- 热重载会真正卸载动态库；插件创建的线程和回调必须先安全停止。

权威入口：

- 完整教程：<https://lvyunqi.github.io/QimenBot/plugin/dynamic.html>
- API 0.4+ 主动发送：<https://lvyunqi.github.io/QimenBot/advanced/dynamic-proactive-send-v04.html>
- API 0.5 Webhook：<https://lvyunqi.github.io/QimenBot/advanced/dynamic-webhook-v05.html>
- crates.io Host API：<https://crates.io/crates/abi-stable-host-api>
- crates.io 过程宏：<https://crates.io/crates/qimen-dynamic-plugin-derive>
- 独立完整示例：<https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example>
- 可复制模板：<https://github.com/lvyunqi/QimenBot/tree/main/templates/dynamic-plugin>

## 三套版本不能混用

| 版本 | 示例 | 作用 |
|---|---|---|
| QimenBot Release | `v0.1.16` | 宿主程序版本 |
| crates.io 包版本 | `0.1.12` | Rust 依赖发布版本 |
| 动态 ABI API | `api = "0.5"` | 插件与宿主协商的能力版本 |

本参考已用 crates.io `0.1.12` 验证。创建项目时先查询当前非 yanked 最新版，并让两个 QimenBot 专用 crate 使用同一版本：

```bash
cargo search abi-stable-host-api --limit 1
cargo search qimen-dynamic-plugin-derive --limit 1
```

新插件显式使用 `api = "0.5"`。API 0.5 包含 API 0.4 的实时主动发送并增加 Webhook；省略 `api` 会生成兼容旧宿主的 API 0.3 插件，不应作为新项目默认值。

## 无主框架源码的完整起步流程

在任意目录创建独立库：

```bash
cargo new --lib qimen-dynamic-plugin-myplugin
cd qimen-dynamic-plugin-myplugin
```

`Cargo.toml`：

```toml
[package]
name = "qimen-dynamic-plugin-myplugin"
edition = "2024"
version = "0.1.0"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "0.1.12"
qimen-dynamic-plugin-derive = "0.1.12"
abi_stable = "0.11"
serde_json = "1"
```

空 `[workspace]` 只用于把 QimenBot 源码树内的动态插件与根工作区隔离。仓库外的独立项目不需要它；从本仓库模板复制出去时可以保留，也可以删除。禁止把外部插件依赖写成 QimenBot 本地 path。

`src/lib.rs`：

```rust
use abi_stable_host_api::{
    CommandRequest, CommandResponse, PluginInitConfig, PluginInitResult,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(id = "my-plugin", version = "0.1.0", api = "0.5")]
mod plugin {
    use super::*;

    #[init]
    fn init(config: PluginInitConfig) -> PluginInitResult {
        let json = config.config_json.as_str();
        if !json.is_empty() && serde_json::from_str::<serde_json::Value>(json).is_err() {
            return PluginInitResult::err("插件配置不是有效 JSON");
        }
        PluginInitResult::ok()
    }

    #[command(
        name = "hello",
        description = "向发送者打招呼",
        aliases = "hi,你好",
        category = "general"
    )]
    fn hello(req: &CommandRequest) -> CommandResponse {
        let nickname = req.sender_nickname.as_str();
        let display = if nickname.is_empty() {
            req.sender_id.as_str()
        } else {
            nickname
        };
        CommandResponse::text(&format!("你好，{display}"))
    }
}
```

过程宏负责生成描述符、命令/路由回调、生命周期、Host API 绑定及 API 0.5 Webhook 导出。不要手写 `#[unsafe(no_mangle)]`，除非明确选择文档中的手动 FFI 路径。

## 命令、事件与拦截器

动态命令属性使用字符串，而不是静态插件的数组语法：

```rust
#[command(
    name = "status",
    description = "查看状态",
    aliases = "st,状态",
    category = "tools",
    role = "admin",
    scope = "group"
)]
fn status(req: &CommandRequest) -> CommandResponse
```

`role` 支持空、`admin`、`owner`；`scope` 支持空/`all`、`group`、`private`。

常用请求字段全部是 ABI 稳定值：`args`、`command_name`、`sender_id`、`group_id`、`sender_nickname`、`message_id`、`timestamp`、`raw_event_json`。读取字符串使用 `.as_str()`。跨协议时不要把 ID 假定为数字；平台专属字段从 `raw_event_json` 解析，并为缺失字段提供降级。

事件路由：

```rust
#[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
fn on_poke(req: &NoticeRequest) -> NoticeResponse {
    NoticeResponse {
        action: DynamicActionResponse::text_reply(req.route.as_str()),
    }
}
```

`kind` 为 `notice`、`request` 或 `meta`，`events` 是逗号分隔的 QimenBot 路由名。未知字段从 `req.raw_event_json` 解析。

每个插件最多各有一个 `#[pre_handle]` 和 `#[after_completion]`：

```rust
#[pre_handle]
fn filter(req: &InterceptorRequest) -> InterceptorResponse {
    if req.message_text.as_str().contains("blocked-word") {
        InterceptorResponse::block()
    } else {
        InterceptorResponse::allow()
    }
}
```

拦截器不能长时间阻塞；耗时任务移到受控工作线程或外部服务。

## 回复消息

被动回复使用 `CommandResponse`，宿主自动回复当前来信会话：

```rust
CommandResponse::builder()
    .reply(req.message_id.as_str())
    .at(req.sender_id.as_str())
    .text(" 已处理")
    .face(1)
    .image_url("https://example.com/result.png")
    .build_auto()
```

可用 `.text()`、`.at()`、`.at_all()`、`.face()`、`.image_url()`、`.image_base64()`、`.record()`、`.reply()`。简单回复使用 `CommandResponse::text()`，不回复使用 `CommandResponse::ignore()`。

富消息在不同协议上的支持不同。官方 QQ Bot 的 ID、媒体上传和回复额度由宿主约束；插件应优先返回通用段并检查真实平台测试结果。

## 配置与生命周期

宿主从 `config/plugins/<plugin_id>.toml` 读取配置，转成 JSON 后传给 `#[init]`：

```toml
# config/plugins/my-plugin.toml
[feature]
enabled = true
message = "hello"
```

`PluginInitConfig` 还提供 `plugin_id`、`plugin_dir` 和 `data_dir`。初始化失败返回 `PluginInitResult::err(...)`，宿主会跳过插件并记录原因。

`#[shutdown]` 必须释放文件、数据库、socket、线程和全局状态。热重载顺序是：停止路由、调用 shutdown、等待回调安全结束、解绑 Host API、卸载动态库、重新扫描并 init 新库。

## 主动发送：区分旧队列和实时 Host API

| 写法 | 可用位置 | 行为 |
|---|---|---|
| `BotApi::send_group_msg(...)` | 所有 API 的同步回调内 | 写入插件本地兼容队列，当前 FFI 回调返回后 flush |
| `SendBuilder::group(...).send()` | 所有 API 的同步回调内 | 同上；不能用于脱离回调长期运行的线程 |
| `BotApi::for_bot(...).send_*()` | API 0.4/0.5 | 立即提交宿主实时队列，可用于后台线程 |
| `BotApi::for_account(...).send_*()` | crates 0.1.12 + API 0.4/0.5 | 按稳定账号选择 Bot |
| `SendBuilder...bot(...).try_send()` | API 0.4/0.5 | 富媒体实时提交并返回状态 |

优先使用稳定 `account_id`，避免部署实例 `bot_id` 改名后修改插件：

```rust
let status = BotApi::for_account("2733944636")
    .send_group_msg("group-id", "后台任务完成");

let status = SendBuilder::channel("channel-id")
    .bot_account("2733944636")
    .guild_id("guild-id")
    .text("频道通知")
    .try_send();
```

检查 `SendEnqueueStatus`：`Accepted` 表示宿主已接收入队；其余状态包括 `HostUnavailable`、`InvalidRequest`、`BotNotFound`、`BotDisabled`、`QueueFull` 和 `HostShuttingDown`。失败重试必须限次并退避，不能压满队列。

后台线程可以在 API 0.5 插件的 `#[init]` 中启动，但必须持有停止信号与 `JoinHandle`：

```rust
#[shutdown]
fn shutdown() {
    STOP.store(true, Ordering::Release);
    if let Some(handle) = WORKER.lock().ok().and_then(|mut slot| slot.take()) {
        handle.thread().unpark();
        let _ = handle.join();
    }
}
```

动态库卸载后仍运行的线程会调用失效代码并导致进程崩溃，这是必须阻止的边界。

## API 0.5 Webhook

```rust
#[webhook(method = "POST", path = "/events")]
fn receive(req: &WebhookRequest) -> WebhookResponse {
    let body = String::from_utf8_lossy(req.body.as_slice());
    WebhookResponse::text(200, &format!("received {} bytes", body.len()))
}
```

完整地址是 `{base_path}/{plugin_id}{path}`。Webhook Gateway 必须在宿主配置中启用。生产环境需要：

- 网关 Bearer token、回环监听与反向代理 TLS。
- 按第三方规范校验原始 body 的 HMAC、时间戳、nonce 和重放。
- 限制 body、并发与回调时间；同步回调不执行长任务。
- 主动发消息时显式选择 `bot_id` 或 `account_id`。

## 构建 target 与产物

| 宿主 | 插件 target | 产物 |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `qimen_dynamic_plugin_myplugin.dll` |
| Linux x64 GNU | `x86_64-unknown-linux-gnu` | `libqimen_dynamic_plugin_myplugin.so` |
| Linux ARM64 GNU | `aarch64-unknown-linux-gnu` | `libqimen_dynamic_plugin_myplugin.so` |
| Docker amd64 | `x86_64-unknown-linux-gnu` | `.so` |
| Docker arm64 | `aarch64-unknown-linux-gnu` | `.so` |
| macOS Intel | `x86_64-apple-darwin` | `.dylib` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `.dylib` |
| Linux musl 安装包 | 不支持 | `Dynamic loading not supported` |

同平台构建：

```bash
cargo fmt --check
cargo check
cargo test
cargo build --release
```

显式 target：

```bash
rustup target add x86_64-unknown-linux-gnu
cargo build --release --target x86_64-unknown-linux-gnu
```

跨系统或 CPU 时，仅安装 Rust target 通常不够，还需要 linker 和目标 libc。优先在部署机、同架构 CI 或与宿主一致/更旧的容器中构建。Linux 插件使用过新的 glibc 构建会报 `GLIBC_x.y not found`。

## 部署与加载

复制动态库到宿主 `plugin_bin_dir`，默认 `plugins/bin/`：

```bash
cp target/release/libqimen_dynamic_plugin_myplugin.so /opt/qimenbot/plugins/bin/
file /opt/qimenbot/runtime/qimenbotd /opt/qimenbot/plugins/bin/libqimen_dynamic_plugin_myplugin.so
ldd /opt/qimenbot/plugins/bin/libqimen_dynamic_plugin_myplugin.so
```

然后执行：

```text
/plugins reload
/plugins
/dynamic-errors
```

`/plugins enable <id>`、`/plugins disable <id>` 会写入 `config/plugin-state.toml`。重新复制二进制但仍未加载时，先检查它是否被持久化禁用。

常见错误：

| 错误 | 原因 | 处理 |
|---|---|---|
| `Dynamic loading not supported` | musl 宿主 | 换 GNU 包或 Docker |
| `wrong ELF class` / `Exec format error` | CPU 或位数不一致 | 用正确 target 重建 |
| `GLIBC_x.y not found` | 构建机 glibc 过新 | 在相同或更旧环境重建 |
| `cannot open shared object file` | 依赖库缺失 | 用 `ldd` 查缺失依赖 |
| `undefined symbol` / API 不兼容 | 依赖、ABI API 或旧文件混用 | 核对 crate 版本、`api` 和实际部署产物 |
| 热重载崩溃 | 后台线程或 FFI 回调未结束 | 在 shutdown 停止并 join，避免悬空回调 |

不要提交 QimenBot 主仓库中的 `plugins/bin/`、`config/plugins/*.toml`、数据库或运行时资源。独立插件仓库可自行发布源码和按 target 构建的 Release 资产，但必须清楚标注兼容的宿主、API 和平台。

需要进入 GitHub 插件商城时，继续阅读 [插件商城发布](marketplace-publishing.md)。商城要求公开源码、明确许可证、固定数字仓库 ID、不可变 SemVer、按 target 的资产大小与 SHA256，以及 GNU/Linux 最低 glibc。静态插件只能展示源码，动态插件才支持在线安装。
