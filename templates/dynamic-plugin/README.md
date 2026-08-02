# QimenBot 动态插件模板

这是一个不依赖 QimenBot 主框架源码的 Rust `cdylib` 模板，使用：

- crates.io `abi-stable-host-api = 0.1.12`
- crates.io `qimen-dynamic-plugin-derive = 0.1.12`
- 动态插件 ABI API `0.5`

crate 发布版本和 `api = "0.5"` 是两套版本，不要混为一谈。创建项目时可先在 crates.io 确认最新非 yanked 版本，并让两个 QimenBot crate 保持一致。

## 创建项目

从 QimenBot 仓库复制模板：

```bash
cp -R templates/dynamic-plugin qimen-dynamic-plugin-myplugin
cd qimen-dynamic-plugin-myplugin
```

也可以在任意目录执行 `cargo new --lib qimen-dynamic-plugin-myplugin`，然后使用本模板的 `Cargo.toml` 和 `src/lib.rs`。仓库外开发不需要 QimenBot 源码，也不能使用指向主框架的 path 依赖。

模板中的空 `[workspace]` 用来避免它在 QimenBot 源码树内被根工作区接管。复制到独立仓库后可以保留，也可以删除，不影响插件 API。

将所有 `{{name}}` 替换为稳定的插件 ID，例如 `myplugin`。插件 ID 同时决定配置文件名、启停状态和 Webhook URL，发布后不要随意修改。

## 当前模板包含

- `#[command]` 命令和字符串 ID 处理。
- `#[init]` / `#[shutdown]` 生命周期。
- API 0.4/0.5 Host API 实时主动发送。
- `account_id` 稳定账号选择，也兼容 `bot_id` 实例选择。
- API 0.5 `#[webhook]` 路由。
- 热重载前停止并 `join` 后台线程。

不需要后台推送或 Webhook 时可以删除对应代码，但保留显式 `api = "0.5"`。

## 插件配置

在 QimenBot 宿主创建 `config/plugins/<插件ID>.toml`：

```toml
[background_push]
# 推荐填写宿主 [[bots]].account_id，避免实例 id 改名后修改插件。
account_id = "2733944636"
group_id = "123456789"
message = "定时通知"
interval_secs = 60
```

也可以把 `account_id` 换成 `bot_id = "qq-main"`，两者只能配置一个。宿主 Bot 配置示例：

```toml
[[bots]]
id = "qq-main"
account_id = "2733944636"
enabled = true
```

## Webhook Gateway

宿主默认关闭 Webhook Gateway，需要在 `config/base.toml` 启用：

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

模板路由的完整地址为：

```text
POST /webhooks/<插件ID>/events
```

生产环境应由反向代理提供 TLS，并在插件中按第三方协议校验原始 body 的 HMAC、时间戳和重放。

## 编译

```bash
cargo fmt --check
cargo check
cargo test
cargo build --release
```

动态库必须匹配 QimenBot 宿主：

| 宿主 | Rust target | 产物 |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `qimen_dynamic_plugin_myplugin.dll` |
| Linux x64 GNU / Docker amd64 | `x86_64-unknown-linux-gnu` | `libqimen_dynamic_plugin_myplugin.so` |
| Linux ARM64 GNU / Docker arm64 | `aarch64-unknown-linux-gnu` | `libqimen_dynamic_plugin_myplugin.so` |
| macOS Intel | `x86_64-apple-darwin` | `libqimen_dynamic_plugin_myplugin.dylib` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `libqimen_dynamic_plugin_myplugin.dylib` |
| Linux musl 安装包 | 不支持动态插件 | - |

跨系统或 CPU 构建还需要目标 linker 与 libc。Linux 插件应在与宿主相同或更旧的 glibc 环境构建。

## 部署与加载

复制产物到宿主 `plugin_bin_dir`，默认 `plugins/bin/`：

```bash
cp target/release/libqimen_dynamic_plugin_myplugin.so /opt/qimenbot/plugins/bin/
file /opt/qimenbot/runtime/qimenbotd /opt/qimenbot/plugins/bin/libqimen_dynamic_plugin_myplugin.so
ldd /opt/qimenbot/plugins/bin/libqimen_dynamic_plugin_myplugin.so
```

然后在 Bot 中执行：

```text
/plugins reload
/plugins
/dynamic-errors
```

加载失败时先检查 CPU、GNU/musl、glibc、缺失共享库和 `config/plugin-state.toml`。完整开发与排错说明见：

- <https://lvyunqi.github.io/QimenBot/plugin/dynamic.html>
- <https://github.com/lvyunqi/QimenBot/tree/main/skills/qimenbot-plugin-development>
