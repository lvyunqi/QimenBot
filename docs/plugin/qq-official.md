# 官方 QQ Bot 插件适配

本页面向插件作者，说明如何让插件在官方 QQ Bot 下正常工作。你不需要先理解 Gateway、OpenAPI 这些底层细节；写插件时只要记住一个原则：

> 能用框架通用能力就用通用能力，少依赖 OneBot 专属的 QQ 号、群号和群管理接口。

## 先写一个能跑的命令

普通文本命令不需要特殊处理。下面这个插件在 OneBot 11 和官方 QQ Bot 下都能回复：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "hello")]
#[commands]
impl HelloPlugin {
    #[command("打招呼")]
    async fn hello(&self) -> &str {
        "你好，我在。"
    }
}
```

如果只是收文本、回文本，官方 QQ Bot 和 OneBot 的写法基本一样。

## 获取用户和会话 ID

官方 QQ Bot 不会把传统 QQ 号直接交给插件。插件拿到的是官方平台分配的字符串 ID，例如 `openid`、`member_openid`、`group_openid` 或频道用户 ID。

所以写插件时，优先使用字符串 ID：

```rust
#[command("查看身份")]
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String {
    let sender = ctx.sender_id().unwrap_or("unknown");
    let chat = ctx.chat_id().unwrap_or("unknown");

    format!("sender={sender}\nchat={chat}")
}
```

常用方法：

| 方法 | 官方 QQ Bot 下的含义 |
|------|----------------------|
| `ctx.sender_id()` | 发送者 ID，可能是 `member_openid`、`user_openid` 或频道用户 ID |
| `ctx.chat_id()` | 当前会话 ID，可能是群 openid、用户 openid、频道 ID 或频道私信 ID |
| `ctx.group_id()` | 仅 QQ 群消息有值，返回 `group_openid` |
| `ctx.event.message_id_str()` | 消息 ID，官方消息 ID 通常是字符串 |

不推荐在兼容官方 QQ Bot 的插件里使用：

| 方法 | 原因 |
|------|------|
| `ctx.sender_id_i64()` | 官方 ID 通常不是数字 |
| `ctx.group_id_i64()` | 官方群 ID 是 `group_openid`，不是传统数字群号 |
| `ctx.event.message_id()` | 官方消息 ID 通常不是数字 |

## 判断消息来自哪里

如果只区分“群聊”和“私聊”，可以用：

```rust
let scope = if ctx.is_group() {
    "QQ 群"
} else if ctx.is_private() {
    "QQ 单聊"
} else {
    "频道或其他场景"
};
```

官方 QQ Bot 还支持频道消息。想分得更细，可以看 `message_type`：

```rust
let message_type = ctx.event.message_type().unwrap_or("unknown");

let scene = match message_type {
    "group" => "QQ 群 @ 消息",
    "private" => "QQ 单聊 C2C",
    "channel" => "频道 @ 消息",
    "channel_private" => "频道私信",
    _ => "其他消息",
};

format!("当前场景：{scene}")
```

场景对应关系：

| 官方场景 | `message_type()` | `sender_id()` | `chat_id()` |
|----------|------------------|---------------|-------------|
| QQ 群 @ | `group` | `member_openid` | `group_openid` |
| QQ 单聊 C2C | `private` | `user_openid` | `user_openid` |
| 频道 @ | `channel` | 频道用户 ID | `channel_id` |
| 频道私信 | `channel_private` | 频道用户 ID | `guild_id` |

## 回复消息

最简单的回复方式是直接返回字符串或 `Message`：

```rust
#[command("ping")]
async fn ping(&self) -> &str {
    "pong"
}
```

```rust
#[command("帮助")]
async fn help(&self) -> Message {
    Message::builder()
        .text("可用命令：/ping /whoami")
        .build()
}
```

框架会根据消息来源自动选择正确的官方发送接口。插件通常不需要自己判断是 QQ 群、单聊还是频道。

## 富文本和按钮

官方 QQ Bot 支持 Markdown、Keyboard、Ark、Embed、媒体上传等能力。最常用的是 Markdown 和按钮：

```rust
use qimen_message::keyboard::*;

#[command("菜单")]
async fn menu(&self) -> Message {
    let keyboard = KeyboardBuilder::new()
        .command_button("帮助", "/help")
        .command_button("状态", "/status")
        .build();

    Message::builder()
        .markdown("# 菜单\n请选择一个操作。")
        .keyboard(keyboard)
        .build()
}
```

如果你只想做文本插件，不需要使用这些能力。富文本能力是否可用还取决于官方平台的场景支持和机器人权限。

示例插件里已经提供了几条测试命令：

| 命令 | 用途 |
|------|------|
| `/qq-md` | 测试 Markdown |
| `/qq-keyboard` | 测试 Markdown + Keyboard |
| `/qq-ark` | 测试 Ark |
| `/qq-embed` | 测试 Embed |
| `/qq-media image <url>` | 测试图片上传发送 |

启用 `example-message` 模块后即可测试这些命令。

## OneBot API 不是通用 API

`ctx.onebot_actions()` 是 OneBot 11 API 客户端，适合 OneBot 11：

```rust
let client = ctx.onebot_actions();
let _ = client.set_group_ban(group_id, user_id, 60).await;
```

这类接口通常不适合官方 QQ Bot。原因很简单：官方 QQ Bot 没有传统 QQ 号和群号，也不一定提供同样的群管理能力。

如果插件要同时兼容 OneBot 和官方 QQ Bot，可以这样写：

```rust
#[command("群信息")]
async fn group_info(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
    let Some(group_id) = ctx.group_id_i64() else {
        return CommandPluginSignal::Reply(Message::text(
            "当前协议不支持这个 OneBot 群信息接口。",
        ));
    };

    match ctx.onebot_actions().get_group_info(group_id, false).await {
        Ok(info) => CommandPluginSignal::Reply(Message::text(format!(
            "群名：{}",
            info.group_name
        ))),
        Err(err) => CommandPluginSignal::Reply(Message::text(format!("获取失败：{err}"))),
    }
}
```

这段代码的含义是：只有拿得到数字群号时才调用 OneBot 群信息接口；拿不到时给用户一个清楚的提示。

## 权限配置

插件里的 `role = "admin"`、`role = "owner"` 仍然可用。区别在于配置里的 ID 要跟协议一致：

```toml
[[bots]]
id = "qq-official"
protocol = "qq-official"

owners = ["用户 openid 或频道用户 ID"]
admins = ["用户 openid 或频道用户 ID"]
```

可以先用 `/whoami` 或自己写的身份命令查看当前 `sender_id()`，再把这个字符串填进配置。

## 推荐写法

| 目标 | 推荐写法 |
|------|----------|
| 获取用户 | `ctx.sender_id().unwrap_or("unknown")` |
| 获取当前会话 | `ctx.chat_id().unwrap_or("unknown")` |
| 获取消息 ID | `ctx.event.message_id_str()` |
| 回复文本 | 返回 `&str` 或 `String` |
| 回复富文本 | 返回 `Message` |
| 判断场景 | `ctx.event.message_type()` |
| 调 OneBot 群管接口 | 先确认 `ctx.group_id_i64()` 有值 |

## 一个完整小例子

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "qqbot-demo")]
#[commands]
impl QqBotDemo {
    #[command("查看当前会话")]
    async fn whereami(&self, ctx: &CommandPluginContext<'_>) -> String {
        let sender = ctx.sender_id().unwrap_or("unknown");
        let chat = ctx.chat_id().unwrap_or("unknown");
        let ty = ctx.event.message_type().unwrap_or("unknown");

        format!("类型：{ty}\n发送者：{sender}\n会话：{chat}")
    }

    #[command("发送菜单")]
    async fn menu(&self) -> Message {
        Message::builder()
            .markdown("# 菜单\n- /whereami 查看当前会话\n- /ping 测试连通")
            .build()
    }
}
```

这个例子不依赖数字 QQ 号，也不调用 OneBot 专属 API，因此更容易同时跑在 OneBot 11 和官方 QQ Bot 上。
