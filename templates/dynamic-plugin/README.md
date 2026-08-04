# QimenBot 动态插件模板

这是 QimenBot 动态插件 API 0.6 的 Rust `cdylib` 模板，包含在线配置、后台主动发送和 Webhook。

- 动态插件 ABI API `0.6`
- `config.schema.json` 和 `config.ui.json`
- `config_apply = "reload"`
- 可选 `#[validate_config]` 业务校验

> **预发布依赖**
>
> crates.io `0.1.12` 只支持 API 0.1 至 0.5。当前模板把两个 QimenBot 专用依赖固定到公开提交 `f6aa64841a6afca024b8f6e99b2945f05c1f007a`，没有主框架源码也能构建 API 0.6，并且不会随分支移动而改变依赖内容。
>
> 正式发布后，运行 `cargo search abi-stable-host-api --limit 1` 和 `cargo search qimen-dynamic-plugin-derive --limit 1`。确认两个包的同一版本明确支持 API 0.6，再把两条 Git 依赖一起改成该 crates.io 版本。

## 创建项目

从 QimenBot 仓库复制模板：

```bash
cp -R templates/dynamic-plugin qimen-dynamic-plugin-myplugin
cd qimen-dynamic-plugin-myplugin
```

也可以在任意目录执行 `cargo new --lib qimen-dynamic-plugin-myplugin`，然后复制本模板的 `src/lib.rs`、`config.schema.json` 和 `config.ui.json`。模板使用公开 Git revision，不需要本机存在 QimenBot 源码，也不能改成指向作者电脑目录的 path 依赖。

模板中的空 `[workspace]` 用来避免它在 QimenBot 源码树内被根工作区接管。复制到独立仓库后可以保留，也可以删除，不影响插件 API。

复制后把包名 `qimen-dynamic-plugin-template` 和插件 ID `template-plugin` 改成自己的稳定名称，例如包名 `qimen-dynamic-plugin-myplugin`、插件 ID `myplugin`。同时修改日志前缀、Schema 标题和 Webhook 示例中的默认名。插件 ID 会决定配置文件名、启停状态和 Webhook URL，发布后不要随意修改。

## 当前模板包含

- `#[command]` 命令和字符串 ID 处理。
- `#[init]` / `#[shutdown]` 生命周期。
- API 0.4 至 0.6 Host API 实时主动发送。
- API 0.6 JSON Schema 在线配置。
- `#[validate_config]` 跨字段业务校验。
- `account_id` 稳定账号选择，也兼容 `bot_id` 实例选择。
- API 0.5 `#[webhook]` 路由。
- 热重载前停止并 `join` 后台线程。

不需要后台推送或 Webhook 时可以删除对应代码。保留在线配置时必须继续声明 `api = "0.6"`；不需要在线配置时可以删除 `config_*` 参数并继续使用 API 0.5。

## 插件配置

加载插件后，在 Web 管理面板打开“插件”，找到插件卡片并点击“设置”。宿主会根据 `config.schema.json` 生成表单，保存到 `config/plugins/<插件ID>.toml`。

也可以手工创建同一个文件：

```toml
[background_push]
enabled = true
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

`config_apply = "reload"` 表示保存后自动执行插件 `shutdown`，写入文件，再用新配置调用 `init`。新配置导致初始化失败时，宿主恢复旧文件并重新加载旧配置。

`webhook_secret` 在 Schema 中标记为只写密钥。管理 API 只返回“已配置”，不会返回原文。插件读取 `config.config_json` 时仍能得到宿主合并后的完整值，禁止把整个配置 JSON 打进日志。

需要新增字段时先修改 `config.schema.json`。中文名称写在 `title`，范围和必填规则写在 Schema；徽标、滑杆、单位和字段顺序写在 `config.ui.json`。完整规则见：

- <https://lvyunqi.github.io/QimenBot/advanced/dynamic-config-v06.html>

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
