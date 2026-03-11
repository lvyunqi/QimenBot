# QimenBot 插件开发规则

QimenBot 是 Rust Bot 框架，OneBot 11 协议，宏驱动插件开发。

## 核心模式

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("描述", aliases = ["别名"], role = "admin")]
    async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text("回复"))
    }

    #[notice(GroupPoke)]
    async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        SystemPluginSignal::Reply(Message::text("别戳"))
    }
}
```

## 关键规则

- 导入：`use qimen_plugin_api::prelude::*;`
- `#[module]` 必须有 `id`，可选 version, name, description, interceptors
- `#[command]` 属性：描述(必填), aliases, examples, category, role("admin"/"owner"), hidden
- 命令 4 种签名：无参 / ctx / args / ctx+args
- 返回 `&str`/`String`/`Message`/`CommandPluginSignal`/`Result<T,E>` 自动转换
- 系统事件：`#[notice(路由)]`, `#[request(路由)]`, `#[meta(路由)]`
- CommandPluginSignal: Reply | Continue | Block | Ignore
- SystemPluginSignal: Continue | Reply | ApproveFriend | RejectFriend | ApproveGroupInvite | Block | Ignore
- 上下文：sender_id(), group_id_i64(), is_group(), plain_text(), message(), onebot_actions()
- 消息：`Message::builder().text("").at("QQ").image("URL").build()`
- OneBot: `ctx.onebot_actions().send_group_msg(gid, msg).await`
- 拦截器实现 `MessageEventInterceptor`，`pre_handle` 返回 true 放行 false 拦截

## 编码规范

- 中英文双语注释
- tracing 日志，不用 println!
- 新插件放 `plugins/`，根 Cargo.toml workspace members 注册
- `cargo check --workspace` 验证
- 参考 `plugins/qimen-plugin-example/src/`
