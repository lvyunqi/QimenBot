# 静态插件开发

## 适用边界

静态插件与 `qimenbotd` 一起编译，适合框架内置能力、完整 async 逻辑、OneBot Action 调用和需要编译期类型检查的功能。它依赖 QimenBot 工作区中的 path crates，因此必须拥有主框架源码；只有安装包或远程服务器时不能完成静态插件集成。

权威参考：

- 概览：<https://lvyunqi.github.io/QimenBot/plugin/overview.html>
- 命令：<https://lvyunqi.github.io/QimenBot/plugin/commands.html>
- 消息：<https://lvyunqi.github.io/QimenBot/plugin/messages.html>
- 事件：<https://lvyunqi.github.io/QimenBot/plugin/events.html>
- 拦截器：<https://lvyunqi.github.io/QimenBot/plugin/interceptors.html>
- 官方 QQ Bot：<https://lvyunqi.github.io/QimenBot/plugin/qq-official.html>
- 官方 QQ Bot Markdown：<https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html>
- 仓库示例：`plugins/qimen-plugin-example/`

## 创建与链接

目录名使用 `qimen-plugin-` 前缀后，根工作区的 `plugins/qimen-plugin-*` glob 会自动发现它，不要再手工修改 workspace members。

```text
plugins/qimen-plugin-myplugin/
├── Cargo.toml
└── src/
    └── lib.rs
```

```toml
[package]
name = "qimen-plugin-myplugin"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true

[dependencies]
async-trait.workspace = true
tracing.workspace = true
qimen-error = { path = "../../crates/qimen-error" }
qimen-message = { path = "../../crates/qimen-message" }
qimen-plugin-api = { path = "../../crates/qimen-plugin-api" }
qimen-plugin-derive = { path = "../../crates/qimen-plugin-derive" }
```

按需增加 `tokio`、`serde` 或 `serde_json`，不要复制示例插件未使用的依赖。

静态插件只有被最终二进制链接后，`inventory` 注册才会生效：

1. 在 `apps/qimenbotd/Cargo.toml` 添加 path 依赖。
2. 在 `apps/qimenbotd/src/main.rs` 使用 `extern crate` 引入插件 crate。
3. 用 `std::hint::black_box` 引用至少一个公开模块的 `__QIMEN_MODULE_ID`，避免 Windows/MSVC 丢弃只含 inventory 构造器的目标文件。

```rust
extern crate qimen_plugin_myplugin;

fn retain_static_plugins() {
    std::hint::black_box(qimen_plugin_myplugin::MyPlugin::__QIMEN_MODULE_ID);
}
```

最后在 `config/base.toml` 中启用模块 ID：

```toml
[official_host]
plugin_modules = ["my-plugin"]
```

`plugin_modules` 填 `#[module(id = "...")]` 的 ID，不是 crate 名。插件被 `config/plugin-state.toml` 持久化为禁用时仍不会加载。

## 最小实现

```rust
use qimen_plugin_api::prelude::*;

#[module(
    id = "my-plugin",
    version = "0.1.0",
    name = "My Plugin",
    description = "示例静态插件"
)]
#[commands]
impl MyPlugin {
    #[command(
        "回复 pong",
        aliases = ["p"],
        examples = ["/ping"],
        category = "general"
    )]
    async fn ping(&self) -> &'static str {
        "pong"
    }
}
```

`#[module]` 与 `#[commands]` 会生成模块、命令/系统插件实现及 inventory 注册。优先使用这条宏路径；只有宏无法表达的生命周期或注册行为才手工实现 `Module`、`CommandPlugin` 或 `SystemPlugin` trait。

## 命令

`#[command]` 支持：

| 属性 | 形式 | 说明 |
|---|---|---|
| 描述 | 第一个字符串或 `desc = "..."` | `/help` 中的说明 |
| `name` | 字符串 | 默认由函数名的下划线转换成连字符 |
| `aliases` | 字符串数组 | 命令别名 |
| `examples` | 字符串数组 | 使用示例 |
| `category` | 字符串 | 默认 `general` |
| `role` | `admin` / `owner` | 权限门槛 |
| `scope` | `all` / `group` / `private` | 分发前自动过滤 |
| `hidden` | flag | 不出现在帮助列表 |

常用签名只有以下四种，`ctx` 必须位于 `args` 前：

```rust
async fn ping(&self) -> &'static str
async fn echo(&self, args: Vec<String>) -> String
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> Message
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal
```

`&str`、`String`、`Message`、`CommandPluginSignal` 和 `Result<T, E>` 会自动转换。需要控制插件链时使用：

- `Reply(message)`：回复并继续后续插件。
- `Continue`：不回复并继续。
- `Block(message)`：回复并终止后续插件。
- `Ignore`：静默终止。

不要把参数解析失败写成 `unwrap()`；返回明确用法或业务错误。

## 上下文与跨协议 ID

当前常用返回类型是可选值，不要沿用旧 skill 中的非空假设：

```rust
ctx.sender_id()   -> Option<&str>
ctx.chat_id()     -> Option<&str>
ctx.group_id()    -> Option<&str>
ctx.message()     -> Option<&Message>
ctx.sender_id_i64() / ctx.group_id_i64() -> Option<i64>
```

面向 OneBot 和官方 QQ Bot 的插件应使用字符串 ID：

```rust
let sender = ctx.sender_id().unwrap_or("unknown");
let chat = ctx.chat_id().unwrap_or("unknown");
```

官方 QQ Bot 使用 openid、group_openid、guild/channel ID，数字转换通常返回 `None`。消息 ID 也优先读取字符串形式。只有明确限定 OneBot 11 且调用 OneBot Action 时才转换为 `i64`。

`ctx.onebot_actions()` 提供异步 OneBot Action，例如群资料、禁言、踢人和主动发送。它不是官方 QQ OpenAPI；跨协议普通回复应返回 `Message`，让运行时按群、C2C、频道或 DMS 自动选择发送端点。

## 消息与官方 QQ Bot

通用消息优先使用 `Message::builder()`：

```rust
let message = Message::builder()
    .text("你好 ")
    .at(sender)
    .text("，处理完成")
    .image("https://example.com/result.png")
    .build();
```

可解析 `plain_text()`、`at_list()`、`image_urls()`、`has_reply()` 和 `reply_id()`。Markdown、Keyboard、Ark、Embed 及群/C2C 媒体存在平台限制；实现前先读 [官方 QQ Bot Markdown 文档](https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html)、官方 QQ Bot 参考与 `message_demo.rs`。官方 Markdown 页面列出的标题、文字样式、链接、图片、列表、引用、分隔线和换行属于平台支持范围；`<br>`、`<font>` 等 HTML 标签是否生效要按实际场景验证，不要把 OneBot 消息段或浏览器 HTML 原样假定为官方支持。

静态插件生成官方 QQ 本地媒体时返回 `base64://...` 消息段，不直接读取 Bot AppSecret 或调用上传接口。群/C2C 会由宿主完成分片预上传，频道/DMS 只支持本地图片 multipart。图片、视频、语音和文件的内联上限分别为 20 MB、30 MB、20 MB、32 MB；大文件改用 QQ 可访问的 HTTPS URL。

## 系统事件与拦截器

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal

#[request(Friend, GroupInvite)]
async fn on_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal

#[meta(Heartbeat)]
async fn on_heartbeat(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal
```

事件原始字段通过 `ctx.event` 读取。处理申请时保留 `flag`、`sub_type` 等原值；未知或缺失字段应返回 `Continue`，不要构造无效审批动作。

拦截器实现异步 trait，并通过模块属性注册：

```rust
pub struct AuditInterceptor;

#[async_trait]
impl MessageEventInterceptor for AuditInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        tracing::debug!(bot_id, text = %event.plain_text(), "收到消息");
        true
    }
}

#[module(id = "my-plugin", interceptors = [AuditInterceptor])]
```

`pre_handle = false` 会阻断后续消息处理；`after_completion` 在所有处理结束后逆序执行。锁不能跨 `.await` 持有，状态共享使用线程安全类型。

## 验证

先验证插件本身，再验证最终宿主链接：

```bash
cargo fmt --check
cargo check -p qimen-plugin-myplugin
cargo test -p qimen-plugin-myplugin
cargo check -p qimenbotd
```

至少增加以下测试：

- 模块公开的命令名、别名、角色和作用域符合预期。
- 参数缺失、非数字 ID、私聊/群聊和字符串 ID 不会 panic。
- 事件与拦截器的放行、阻断和信号符合预期。

运行宿主后确认启动报告包含模块 ID，并在 Web 管理面板“插件”页检查实际状态和命令清单。找不到模块时依次检查 daemon 依赖、`extern crate`、`black_box`、`plugin_modules` 和 `plugin-state.toml`。

Runtime 不提供业务命令。`ping`、`echo`、`status` 等测试命令必须由插件自身声明；停用插件后命令和帮助项会同时消失。命令入口由 `[official_host.commands]` 统一控制，插件不应自行判断固定 `/` 前缀或解析平台 @ 标签。
