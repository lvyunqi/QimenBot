# 插件开发概览

QimenBot 通过过程宏将插件开发降至最简——最少只需要 **7 行代码**就能完成一个可用的插件。

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

这段代码做了什么：

1. `#[module(id = "my-plugin")]` — 声明一个 ID 为 `my-plugin` 的插件模块
2. `#[commands]` — 扫描 impl 块，自动注册其中的命令和事件处理器
3. `#[command("回复 pong")]` — 将 `ping` 函数注册为 `/ping` 命令
4. 返回 `&str` — 框架自动将字符串转换为回复消息

::: info 宏的魔法
`#[module]` 宏会自动帮你：
- 创建 `struct MyPlugin;` 结构体（不需要手动写）
- 实现 `Module` trait
- 实现 `CommandPlugin` trait
- 生成命令注册代码

你只需要专注于业务逻辑。
:::

## 插件结构

一个完整的插件通常包含以下部分：

```rust
use qimen_plugin_api::prelude::*;

// 可选：定义拦截器
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, _bot_id: &str, _event: &NormalizedEvent) -> bool {
        true // 返回 true 放行
    }
}

// 声明模块
#[module(
    id = "my-plugin",
    version = "0.1.0",
    name = "我的插件",
    description = "这是一个示例插件",
    interceptors = [MyInterceptor]
)]
#[commands]
impl MyPlugin {
    // 命令处理器
    #[command("这是一个命令")]
    async fn my_command(&self) -> &str {
        "命令回复"
    }

    // 通知事件处理器
    #[notice(GroupPoke)]
    async fn on_poke(&self) -> &str {
        "被戳了！"
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

## `#[module]` 属性

| 属性 | 必填 | 默认值 | 说明 |
|------|------|-------|------|
| `id` | ✅ | — | 模块唯一标识，用于配置中引用 |
| `version` | ❌ | `"0.1.0"` | 模块版本号 |
| `name` | ❌ | 结构体名称 | 模块显示名称 |
| `description` | ❌ | `""` | 模块描述 |
| `interceptors` | ❌ | `[]` | 拦截器类型列表 |

## 注册插件到框架

### 第 1 步：在 Cargo.toml 中添加依赖

如果你的插件是框架内的 crate：

```toml
# plugins/my-plugin/Cargo.toml
[package]
name = "qimen-my-plugin"
edition.workspace = true

[dependencies]
qimen-plugin-api.workspace = true
async-trait.workspace = true
```

### 第 2 步：在 official-host 中注册

在 `crates/qimen-official-host/src/lib.rs` 中添加你的模块：

```rust
// 在 register_plugin_modules 函数中
if ids.contains("my-plugin") {
    register_module::<my_plugin::MyPlugin>(&mut modules);
}
```

### 第 3 步：在配置中启用

```toml
[official_host]
plugin_modules = ["my-plugin"]

[[bots]]
enabled_modules = ["command", "my-plugin"]
```

## 下一步

- [命令开发](/plugin/commands) — 学习所有命令特性（别名、权限、参数等）
- [消息构建](/plugin/messages) — 学习构建富媒体消息
- [事件处理](/plugin/events) — 学习处理系统事件
- [拦截器](/plugin/interceptors) — 学习编写拦截器
- [动态插件](/plugin/dynamic) — 学习开发动态加载的插件
