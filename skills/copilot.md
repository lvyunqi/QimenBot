# QimenBot — Copilot Instructions

QimenBot 是基于 Rust 的高性能 Bot 框架，OneBot 11 协议，宏驱动插件开发。

## 项目结构

- `crates/qimen-plugin-api/` — 插件 API（traits, contexts, signals）
- `crates/qimen-plugin-derive/` — 过程宏（#[module], #[commands], #[command], #[notice], #[request], #[meta]）
- `crates/qimen-message/` — 消息类型（Message, MessageBuilder, Segment, KeyboardBuilder）
- `plugins/qimen-plugin-example/` — 完整示例插件，开发新插件时首先参考此目录

## 插件开发规则

- 所有插件导入 `use qimen_plugin_api::prelude::*;`
- 模块用 `#[module(id = "...", version = "...")]` + `#[commands]` 声明
- 命令用 `#[command("描述")]`，支持 aliases, examples, category, role, hidden 属性
- 系统事件用 `#[notice(...)]`, `#[request(...)]`, `#[meta(...)]`
- 命令方法 4 种签名：无参 / 仅 ctx / 仅 args / ctx+args
- 返回值支持 `&str`, `String`, `Message`, `CommandPluginSignal`, `Result<T,E>` 自动转换
- CommandPluginSignal: Reply(Message), Continue, Block(Message), Ignore
- SystemPluginSignal: Continue, Reply, ApproveFriend, RejectFriend, ApproveGroupInvite, RejectGroupInvite, Block, Ignore
- 上下文方法：sender_id(), group_id_i64(), is_group(), is_private(), plain_text(), message(), onebot_actions()
- 消息构建：`Message::builder().text("").at("QQ号").image("URL").build()`
- OneBot API：`ctx.onebot_actions().send_group_msg(group_id, msg).await`

## 编码规范

- 中英文双语注释
- 使用 `tracing` 日志，不用 `println!`
- 新插件放 `plugins/`，需在根 `Cargo.toml` workspace members 注册
- 编写后 `cargo check --workspace` 验证
