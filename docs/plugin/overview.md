# 插件开发概览

本页教你从零开始编写一个 QimenBot 插件。只需要 5 分钟，你就能理解整个插件开发流程。

## 你的第一个插件

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin")]
#[commands]
impl MyPlugin {
    #[command("回复 pong")]
    async fn ping(&self) -> &str {
        "pong!"
    }
}
```

就这么简单！用户发送 `/ping`，Bot 回复 `pong!`。

### 这段代码做了什么？

| 代码 | 作用 |
|------|------|
| `use qimen_plugin_api::prelude::*` | 导入所有需要的类型 |
| `#[module(id = "my-plugin")]` | 声明一个 ID 为 `my-plugin` 的插件模块 |
| `#[commands]` | 扫描 impl 块，自动注册命令和事件处理器 |
| `#[command("回复 pong")]` | 将 `ping` 函数注册为 `/ping` 命令 |
| `-> &str` | 返回字符串，框架自动转为回复消息 |

::: info 宏帮你做了什么？
`#[module]` + `#[commands]` 宏在编译时自动帮你：
1. 创建 `struct MyPlugin;` 结构体
2. 实现 `Module` trait（插件注册）
3. 实现 `CommandPlugin` trait（命令处理）
4. 生成命令注册代码

你只需要专注于写业务逻辑，框架负责一切"胶水代码"。
:::

## 一个更完整的插件

实际开发中，一个插件通常会包含：命令、事件处理器、拦截器。

```rust
use qimen_plugin_api::prelude::*;

// ── 可选：定义拦截器 ──
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        // 返回 true = 放行，false = 拦截
        tracing::info!("收到消息: {}", event.plain_text());
        true
    }
}

// ── 声明插件模块 ──
#[module(
    id = "my-plugin",
    version = "0.1.0",
    name = "我的插件",
    description = "一个示例插件",
    interceptors = [MyInterceptor]
)]
#[commands]
impl MyPlugin {
    // 命令处理器
    #[command("回复 pong")]
    async fn ping(&self) -> &str {
        "pong!"
    }

    // 带参数的命令
    #[command("回显文本", aliases = ["e"])]
    async fn echo(&self, args: Vec<String>) -> String {
        args.join(" ")
    }

    // 通知事件处理器
    #[notice(GroupPoke)]
    async fn on_poke(&self) -> &str {
        "别戳我！"
    }

    // 请求事件处理器
    #[request(Friend)]
    async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
        SystemPluginSignal::ApproveFriend {
            flag,
            remark: Some("自动同意".to_string()),
        }
    }
}
```

## `#[module]` 属性一览

| 属性 | 必填 | 默认值 | 说明 |
|------|:----:|-------|------|
| `id` | ✅ | — | 模块唯一标识，用于配置中引用 |
| `version` | ❌ | `"0.1.0"` | 模块版本号 |
| `name` | ❌ | 结构体名称 | 模块显示名称 |
| `description` | ❌ | `""` | 模块描述 |
| `interceptors` | ❌ | `[]` | 拦截器类型列表 |

## 插件能做什么？

| 能力 | 宏 / 方法 | 说明 |
|------|----------|------|
| [处理命令](/plugin/commands) | `#[command]` | 响应 `/ping`、`/echo` 等用户命令 |
| [构建消息](/plugin/messages) | `MessageBuilder` | 发送文本、图片、@、表情、按钮等 |
| [处理事件](/plugin/events) | `#[notice]` `#[request]` `#[meta]` | 处理戳一戳、入群、好友申请等 |
| [拦截消息](/plugin/interceptors) | `MessageEventInterceptor` | 在命令处理前后插入自定义逻辑 |
| [调用 API](/api/onebot-client) | `ctx.onebot_actions()` | 调用 40+ 个 OneBot API（发消息、踢人、禁言等） |

## 注册插件到框架

QimenBot 使用 `inventory` 机制实现**编译时自动注册**——`#[module]` 宏会在编译期间自动将你的插件注册到全局清单中，框架启动时通过 `inventory::iter` 发现所有已链接的插件。**你无需修改框架的任何源代码。**

整个流程只需 4 步：

### 第 1 步：创建插件 Crate

在 `plugins/` 目录下创建以 `qimen-plugin-` 为前缀的目录（workspace 使用 glob `plugins/qimen-plugin-*` 自动发现）：

```bash
cargo init plugins/qimen-plugin-myplugin --lib
```

编辑 `plugins/qimen-plugin-myplugin/Cargo.toml`：

```toml
[package]
name = "qimen-plugin-myplugin"
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true

[dependencies]
qimen-plugin-api = { path = "../../crates/qimen-plugin-api" }
qimen-plugin-derive = { path = "../../crates/qimen-plugin-derive" }
async-trait.workspace = true
tracing.workspace = true
```

然后在 `plugins/qimen-plugin-myplugin/src/lib.rs` 中编写插件代码：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", name = "我的插件")]
#[commands]
impl MyPlugin {
    #[command("回复 pong")]
    async fn ping(&self) -> &str {
        "pong!"
    }
}
```

::: info 不需要手动编辑 workspace members
根 `Cargo.toml` 中的 `members` 已配置为 `"plugins/qimen-plugin-*"`，只要你的目录名以 `qimen-plugin-` 为前缀，Cargo 就能自动发现。
:::

### 第 2 步：链接插件到主程序

编辑 `apps/qimenbotd/Cargo.toml`，添加你的插件依赖：

```toml
[dependencies]
# ... 其他依赖
qimen-plugin-myplugin = { path = "../../plugins/qimen-plugin-myplugin" }
```

然后编辑 `apps/qimenbotd/src/main.rs`，添加两行代码确保链接器包含插件：

```rust
use qimen_error::Result;
use qimen_official_host::run_official_host;

// ↓ 第一行：强制链接器包含插件 crate
extern crate qimen_plugin_myplugin;

#[tokio::main]
async fn main() -> Result<()> {
    // ↓ 第二行：引用插件的具体符号，防止链接器优化掉 inventory 注册项
    std::hint::black_box(qimen_plugin_myplugin::MyPlugin::__QIMEN_MODULE_ID);

    run_official_host("config/base.toml").await
}
```

::: warning 为什么需要 `extern crate` 和 `black_box`？
在 Windows (MSVC) 上，`use crate as _` 不足以让链接器保留只包含 `inventory` 注册信息的目标文件。必须使用 `extern crate` 并通过 `std::hint::black_box()` 引用一个具体符号（`__QIMEN_MODULE_ID`），链接器才会包含对应的目标文件，inventory 注册才能生效。

在 Linux/macOS 上通常不需要 `black_box`，但为了**跨平台兼容**，建议始终添加。
:::

### 第 3 步：在配置中启用

```toml
# config/base.toml
[official_host]
plugin_modules = ["my-plugin"]  # ← 添加你的插件 module id
```

这里的 `"my-plugin"` 对应 `#[module(id = "my-plugin")]` 中声明的 id。

::: tip Bot 级别的模块控制
每个 Bot 实例可以通过 `enabled_modules` 选择性启用模块。如果留空，则使用 `official_host.builtin_modules` 中的全部内置模块。插件模块只要在 `plugin_modules` 中列出即可对所有 Bot 生效。
:::

### 第 4 步：编译运行

```bash
cargo run
```

启动时日志中会显示 inventory 发现的插件数量：

```
INFO inventory plugin modules discovered, count=1, modules=my-plugin
```

如果你的插件没有出现在日志中，检查：
1. `apps/qimenbotd/Cargo.toml` 是否添加了依赖
2. `main.rs` 中是否有 `extern crate` 和 `black_box` 两行
3. 插件 crate 是否能通过 `cargo check -p qimen-plugin-myplugin` 编译通过

### 新增插件速查清单

| 步骤 | 要改的文件 | 做什么 |
|:----:|-----------|--------|
| 1 | `plugins/qimen-plugin-xxx/` | 创建插件 crate，用 `#[module]` + `#[commands]` |
| 2 | `apps/qimenbotd/Cargo.toml` | 添加 `qimen-plugin-xxx = { path = "..." }` |
| 3 | `apps/qimenbotd/src/main.rs` | 添加 `extern crate` + `black_box` |
| 4 | `config/base.toml` | `plugin_modules` 中添加插件 id |

**不需要修改 `qimen-official-host` 或框架中的任何其他代码。**

::: tip 不想重新编译？
如果你希望修改插件后无需重新编译整个框架，可以考虑使用 [动态插件](/plugin/dynamic)。动态插件编译为独立的 `.so/.dll` 文件，通过 `/plugins reload` 热重载。
:::

## 下一步

- [命令开发](/plugin/commands) — 别名、权限、参数注入、返回值类型
- [消息构建](/plugin/messages) — 图片、@、表情、交互按钮
- [事件处理](/plugin/events) — 戳一戳、入群、好友申请
- [拦截器](/plugin/interceptors) — 日志、冷却、黑名单
- [动态插件](/plugin/dynamic) — FFI 热重载插件
