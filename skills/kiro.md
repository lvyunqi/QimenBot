---
mode: auto
description: QimenBot 插件开发指导 — 编辑 plugins/ 目录下的 Rust 代码时自动加载
---

# QimenBot 插件开发

QimenBot 是 Rust Bot 框架，OneBot 11 协议，宏驱动插件开发。

## 关键路径

- 插件 API：`crates/qimen-plugin-api/src/lib.rs`
- 宏定义：`crates/qimen-plugin-derive/src/lib.rs`
- 消息类型：`crates/qimen-message/src/lib.rs`
- 示例插件：`plugins/qimen-plugin-example/src/`

## 插件模板

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0", name = "名称", description = "描述")]
#[commands]
impl MyPlugin {
    #[command("描述", aliases = ["别名"], examples = ["/cmd arg"], role = "admin")]
    async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
        CommandPluginSignal::Reply(Message::text("回复"))
    }

    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        SystemPluginSignal::Reply(Message::text("别戳我"))
    }

    #[request(Friend)]
    async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or("").to_string();
        SystemPluginSignal::ApproveFriend { flag, remark: None }
    }

    #[meta(Heartbeat)]
    async fn on_heartbeat(&self) -> SystemPluginSignal { SystemPluginSignal::Continue }
}
```

## 宏速查

| 宏 | 用途 |
|---|---|
| `#[module(id, version, name, description, interceptors)]` | 模块声明 |
| `#[commands]` | 生成 Plugin 实现 |
| `#[command("描述", aliases, examples, category, role, hidden)]` | 命令 |
| `#[notice(Route1, Route2)]` | 通知事件 |
| `#[request(Route1)]` | 请求事件 |
| `#[meta(Route1)]` | 元事件 |

## 命令签名

`(&self)` / `(&self, ctx: &CommandPluginContext<'_>)` / `(&self, args: Vec<String>)` / `(&self, ctx, args)`

返回值：`&str`/`String` → Reply(text)，`Message` → Reply(msg)，`Result<T,E>` → Ok转T/Err转错误消息

## 信号

- CommandPluginSignal: `Reply(Message)` | `Continue` | `Block(Message)` | `Ignore`
- SystemPluginSignal: `Continue` | `Reply(Message)` | `ApproveFriend{flag,remark}` | `RejectFriend` | `ApproveGroupInvite{flag,sub_type}` | `Block` | `Ignore`

## 上下文

- CommandPluginContext: `sender_id()`, `group_id_i64()`, `is_group()`, `plain_text()`, `message()`, `onebot_actions()`
- SystemPluginContext: `bot_id`, `event`(原始JSON), `onebot_actions()`

## 消息

```rust
Message::builder().text("文本").at("QQ号").image("URL").face("1").reply(id).keyboard(kb).build()
```

## OneBot API

```rust
let actions = ctx.onebot_actions();
actions.send_group_msg(group_id, msg).await
actions.set_group_ban(group_id, user_id, duration).await
actions.get_group_info(group_id, false).await
```

## 拦截器

```rust
pub struct MyInterceptor;
#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool { true }
}
// 注册：#[module(interceptors = [MyInterceptor])]
```

## 规范

- 中英文双语注释，tracing 日志
- 新插件放 `plugins/`，根 Cargo.toml workspace members 注册
- `cargo check --workspace` 验证
