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

### 第 1 步：创建 Cargo 包

在 `plugins/` 目录下创建你的插件：

```toml
# plugins/my-plugin/Cargo.toml
[package]
name = "qimen-my-plugin"
edition.workspace = true

[dependencies]
qimen-plugin-api.workspace = true
async-trait.workspace = true
```

### 第 2 步：在 Official Host 中注册

编辑 `crates/qimen-official-host/src/lib.rs`，添加你的模块：

```rust
// 在 register_plugin_modules 函数中
if ids.contains("my-plugin") {
    register_module::<my_plugin::MyPlugin>(&mut modules);
}
```

### 第 3 步：在配置中启用

```toml
# config/base.toml
[official_host]
plugin_modules = ["my-plugin"]  # ← 添加你的插件 ID

[[bots]]
enabled_modules = ["command", "my-plugin"]  # ← 在 Bot 上启用
```

::: tip 不想重新编译？
如果你希望修改插件后无需重新编译整个框架，可以考虑使用 [动态插件](/plugin/dynamic)。动态插件编译为独立的 `.so/.dll` 文件，通过 `/plugins reload` 热重载。
:::

## 下一步

- [命令开发](/plugin/commands) — 别名、权限、参数注入、返回值类型
- [消息构建](/plugin/messages) — 图片、@、表情、交互按钮
- [事件处理](/plugin/events) — 戳一戳、入群、好友申请
- [拦截器](/plugin/interceptors) — 日志、冷却、黑名单
- [动态插件](/plugin/dynamic) — FFI 热重载插件
