# QimenBot — AI Agent Instructions

> 通用 AGENTS.md 标准，适用于 Kiro / Qodo / GitHub Copilot 等支持此规范的工具。

## 项目概览

QimenBot 是基于 Rust 构建的高性能 Bot 框架，OneBot 11 协议，宏驱动插件开发。

## 项目结构

```
apps/qimenbotd/             # 主程序
apps/qimenctl/              # CLI 工具
crates/qimen-plugin-api/    # 插件 API（traits, contexts, signals）
crates/qimen-plugin-derive/ # 过程宏
crates/qimen-message/       # 消息类型
crates/qimen-protocol-core/ # NormalizedEvent
plugins/                    # 插件目录
  qimen-plugin-example/     # 完整示例（重要参考）
```

## 插件开发核心

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
        SystemPluginSignal::Reply(Message::text("别戳"))
    }

    #[request(Friend)]
    async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or("").to_string();
        SystemPluginSignal::ApproveFriend { flag, remark: None }
    }
}
```

命令 4 种签名：`(&self)` / `(&self, ctx)` / `(&self, args)` / `(&self, ctx, args)`

返回值自动转换：`&str`/`String`/`Message`/`CommandPluginSignal`/`Result<T,E>`

信号：CommandPluginSignal: Reply|Continue|Block|Ignore，SystemPluginSignal: Continue|Reply|ApproveFriend|RejectFriend|ApproveGroupInvite|Block|Ignore

上下文：`sender_id()`, `group_id_i64()`, `is_group()`, `plain_text()`, `message()`, `onebot_actions()`

消息：`Message::builder().text("").at("QQ号").image("URL").build()`

拦截器：实现 `MessageEventInterceptor` trait，`#[module(interceptors = [...])]` 注册

## 编码规范

- 中英文双语注释
- tracing 日志
- 新插件放 `plugins/`，根 Cargo.toml workspace members 注册
- `cargo check --workspace` 验证

## 重要参考

- 示例插件：`plugins/qimen-plugin-example/src/`
- 插件 API：`crates/qimen-plugin-api/src/lib.rs`
- 宏定义：`crates/qimen-plugin-derive/src/lib.rs`
