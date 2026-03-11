# QimenBot — Gemini Context

QimenBot 是基于 Rust 的高性能 Bot 框架，OneBot 11 协议，宏驱动插件开发。

## 关键路径

- 插件 API：`crates/qimen-plugin-api/src/lib.rs`
- 宏定义：`crates/qimen-plugin-derive/src/lib.rs`
- 消息类型：`crates/qimen-message/src/lib.rs`
- 示例插件：`plugins/qimen-plugin-example/src/`

## 插件开发速查

导入：`use qimen_plugin_api::prelude::*;`

```rust
#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("描述", aliases = ["别名"], role = "admin")]
    async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text("回复"))
    }

    #[notice(GroupPoke)]
    async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        SystemPluginSignal::Reply(Message::text("别戳我"))
    }

    #[request(Friend)]
    async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or("").to_string();
        SystemPluginSignal::ApproveFriend { flag, remark: None }
    }
}
```

4 种命令签名：`(&self)`, `(&self, ctx)`, `(&self, args)`, `(&self, ctx, args)`

返回值：`&str`/`String`/`Message`/`CommandPluginSignal`/`Result<T,E>` 均可自动转换。

上下文：`ctx.sender_id()`, `ctx.group_id_i64()`, `ctx.is_group()`, `ctx.plain_text()`, `ctx.message()`, `ctx.onebot_actions()`

消息：`Message::builder().text("").at("QQ号").image("URL").face("1").reply(id).build()`

OneBot API：`ctx.onebot_actions().send_group_msg(group_id, msg).await`

## 编码规范

- 中英文双语注释
- 使用 `tracing` 日志，不用 `println!`
- 新插件放 `plugins/`，根 Cargo.toml workspace members 注册
- `cargo check --workspace` 验证
