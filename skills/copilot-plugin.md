---
applyTo: "plugins/**/*.rs"
---

# QimenBot 插件开发指导

对 `plugins/` 目录下的 Rust 文件自动生效。

## 插件模板

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("描述")]
    async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text("回复"))
    }
}
```

## 关键规则

- `#[module]` 必须有 `id`，可选 version, name, description, interceptors
- `#[command]` 第一个参数是描述字符串，可选 aliases, examples, category, role("admin"/"owner"), hidden
- 命令签名：`(&self)` / `(&self, ctx)` / `(&self, args)` / `(&self, ctx, args)`
- 返回 `&str`/`String`/`Message`/`CommandPluginSignal`/`Result<T,E>` 自动转换
- 系统事件：`#[notice(路由)]`、`#[request(路由)]`、`#[meta(路由)]`
- 拦截器实现 `MessageEventInterceptor` trait，在 `#[module(interceptors = [...])]` 注册
- 参考 `plugins/qimen-plugin-example/src/`
