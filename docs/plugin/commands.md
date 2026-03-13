# 命令开发

命令是 QimenBot 最核心的交互方式。用户发送 `/命令名 参数`，Bot 通过命令处理函数做出响应。

## 最简命令

```rust
#[command("回复 pong")]
async fn ping(&self) -> &str {
    "pong!"
}
```

用户发送 `/ping`，Bot 回复 `pong!`。就是这么简单。

## 命令名怎么来的？

如果你没有手动指定命令名，宏会**自动从函数名推导**：

| 函数名 | 推导出的命令名 | 用户输入 |
|--------|--------------|---------|
| `ping` | `ping` | `/ping` |
| `echo` | `echo` | `/echo` |
| `group_info` | `group-info` | `/group-info` |
| `my_cmd` | `my-cmd` | `/my-cmd` |

规则很简单：下划线 `_` 变连字符 `-`。

你也可以手动指定：

```rust
#[command(name = "greet", desc = "打招呼")]
async fn my_greeting_function(&self) -> &str {
    "你好！"
}
// 命令名是 "greet"，不是 "my-greeting-function"
```

## `#[command]` 完整属性

```rust
#[command(
    "命令描述",                   // 必填，显示在 /help 中
    aliases = ["e", "回显"],      // 别名
    examples = ["/echo hello"],  // 使用示例
    category = "tools",          // 分类（默认 "general"）
    role = "admin",              // 权限要求
    scope = "group",             // 作用域（默认 "all"）
    hidden,                      // 不显示在 /help 中
)]
```

| 属性 | 类型 | 默认值 | 说明 |
|------|------|-------|------|
| 第一个参数 | `&str` | **必填** | 命令描述（显示在 `/help` 中） |
| `name` | `&str` | 函数名推导 | 显式指定命令名 |
| `aliases` | `[&str]` | `[]` | 别名列表，用户可以用别名触发 |
| `examples` | `[&str]` | `[]` | 使用示例（显示在 `/help` 中） |
| `category` | `&str` | `"general"` | 命令分类 |
| `role` | `&str` | 无限制 | `"admin"`（管理员）或 `"owner"`（所有者） |
| `scope` | `&str` | `"all"` | 作用域：`"all"`（全部）、`"group"`（仅群聊）、`"private"`（仅私聊） |
| `hidden` | flag | — | 隐藏命令，不在 `/help` 中显示 |

### 别名示例

```rust
#[command("回显文本", aliases = ["e", "回显"])]
async fn echo(&self, args: Vec<String>) -> String {
    args.join(" ")
}
```

现在 `/echo hello`、`/e hello`、`/回显 hello` 三种方式都能触发。

### 权限控制

```rust
// 任何人都能用（默认）
#[command("回复 pong")]
async fn ping(&self) -> &str { "pong!" }

// 仅管理员可用
#[command("禁言用户", role = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> String {
    // ...
}

// 仅所有者可用
#[command("重载插件", role = "owner")]
async fn reload(&self) -> &str { "正在重载..." }
```

::: info 权限层级
| 角色 | 配置来源 | 权限范围 |
|------|---------|---------|
| **Owner** | `owners = ["QQ号"]` | 所有命令 |
| **Admin** | `admins = ["QQ号"]` 或群管理员 | `role = "admin"` 的命令 |
| **Anyone** | 所有用户 | 无权限限制的命令 |
:::

### 作用域控制

通过 `scope` 属性声明命令的生效范围，框架在分发时**自动过滤**，不匹配的环境下命令静默忽略：

```rust
// 默认：群聊 + 私聊都可用
#[command("回复 pong")]
async fn ping(&self) -> &str { "pong!" }

// 仅群聊可用（私聊中发送 /group-only 不会触发）
#[command("仅群聊命令", scope = "group")]
async fn group_only(&self) -> &str { "这是群聊命令" }

// 仅私聊可用
#[command("仅私聊命令", scope = "private")]
async fn private_only(&self) -> &str { "这是私聊命令" }
```

| `scope` 值 | 说明 |
|------------|------|
| `"all"` (默认) | 群聊和私聊均可触发 |
| `"group"` | 仅在群聊中触发 |
| `"private"` | 仅在私聊中触发 |

::: tip 与手动检查的区别
使用 `scope` 属性后，不匹配的命令**不会出现在 `/help` 列表中**（按环境过滤），且分发层直接跳过，无需在回调函数内部手动判断 `ctx.is_group()`。
:::

## 方法签名

宏根据你的方法参数**自动注入**所需内容。共四种写法：

### 无参数 — 最简单

```rust
#[command("回复 pong")]
async fn ping(&self) -> &str {
    "pong!"
}
```

### 只有 `args` — 获取用户输入

```rust
#[command("回显文本")]
async fn echo(&self, args: Vec<String>) -> String {
    if args.is_empty() {
        "用法: /echo <文本>".to_string()
    } else {
        args.join(" ")
    }
}
```

`args` 是命令名后面的文本，按空格拆分：

```
/echo hello world → args = ["hello", "world"]
/echo             → args = []
```

### 只有 `ctx` — 获取上下文信息

```rust
#[command("查看身份")]
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String {
    let sender = ctx.sender_id();
    let scope = if ctx.is_group() { "群聊" } else { "私聊" };
    format!("你的 ID: {sender}，当前环境: {scope}")
}
```

### `ctx` + `args` — 两者都要

```rust
#[command("禁言用户", role = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
    let user_id = args.first().and_then(|s| s.parse::<i64>().ok());
    let duration = args.get(1).and_then(|s| s.parse::<i64>().ok()).unwrap_or(60);

    match (ctx.group_id_i64(), user_id) {
        (Some(gid), Some(uid)) => {
            let _ = ctx.onebot_actions().set_group_ban(gid, uid, duration).await;
            CommandPluginSignal::Reply(Message::text(format!("已禁言 {uid} {duration} 秒")))
        }
        _ => CommandPluginSignal::Reply(Message::text("用法: /ban <QQ号> [秒数]")),
    }
}
```

::: warning 参数顺序
`ctx` 必须在 `args` **前面**，不能反过来：
```rust
// ✅ 正确
async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>)

// ❌ 错误，编译不通过
async fn cmd(&self, args: Vec<String>, ctx: &CommandPluginContext<'_>)
```
:::

## CommandPluginContext 常用方法

`ctx` 是你和框架交互的入口，以下是最常用的方法：

### 发送者信息

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `ctx.sender_id()` | `&str` | 发送者 QQ 号 |
| `ctx.sender_id_i64()` | `Option<i64>` | 发送者 QQ 号（数字） |
| `ctx.event.sender_nickname()` | `Option<&str>` | 发送者昵称 |
| `ctx.event.sender_role()` | `Option<&str>` | 群角色：`"owner"` / `"admin"` / `"member"` |
| `ctx.event.sender_card()` | `Option<&str>` | 群名片 |

### 聊天环境

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `ctx.is_group()` | `bool` | 是否在群聊中 |
| `ctx.is_private()` | `bool` | 是否在私聊中 |
| `ctx.group_id()` | `&str` | 群号（私聊返回空字符串） |
| `ctx.group_id_i64()` | `Option<i64>` | 群号（私聊返回 None） |
| `ctx.chat_id()` | `&str` | 聊天 ID（群号或用户 ID） |

### 消息内容

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `ctx.plain_text()` | `String` | 消息纯文本（去掉图片、@等） |
| `ctx.message()` | `&Message` | 完整消息对象（包含所有段） |
| `ctx.event.message_id()` | `Option<i64>` | 消息 ID |
| `ctx.event.is_at_self()` | `bool` | 用户是否 @了 Bot |

### 调用 API

```rust
let client = ctx.onebot_actions();
let info = client.get_login_info().await?;
let _ = client.send_group_msg(group_id, Message::text("hello")).await;
```

完整 API 列表见 [OneBot API 客户端](/api/onebot-client)。

## 返回值

命令处理函数支持多种返回值类型，框架自动帮你转换：

| 返回类型 | 效果 | 示例 |
|---------|------|------|
| `&str` | 发送文本回复 | `"pong!"` |
| `String` | 发送文本回复 | `format!("hello {}", name)` |
| `Message` | 发送富媒体回复 | `Message::builder().text("hi").face(1).build()` |
| `CommandPluginSignal` | 完全控制行为 | `Reply` / `Continue` / `Block` / `Ignore` |
| `Result<T, E>` | Ok 正常处理，Err 发送错误信息 | `Ok("done")` / `Err(e)` → `"Error: ..."` |

### CommandPluginSignal 详解

当你需要精确控制行为时，返回 `CommandPluginSignal`：

| 信号 | 效果 |
|------|------|
| `Reply(Message)` | 发送回复消息，**继续**执行后续插件 |
| `Continue` | 什么都不做，继续后续插件 |
| `Block(Message)` | 发送回复消息，**终止**后续所有插件 |
| `Ignore` | 什么都不做，**终止**后续所有插件 |

::: tip 什么时候用 Block？
当你想"独占"处理这个命令时，用 `Block` 可以防止后续插件也响应同一个命令。比如一个验证码插件，验证成功后不希望其他插件再处理这条消息。
:::

## 完整示例

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "example-basic", version = "0.1.0")]
#[commands]
impl BasicModule {
    /// 最简命令
    #[command("回复 pong")]
    async fn ping(&self) -> &str {
        "pong!"
    }

    /// 带参数和别名
    #[command("回显文本", aliases = ["e"])]
    async fn echo(&self, args: Vec<String>) -> String {
        if args.is_empty() {
            "用法: /echo <文本>".to_string()
        } else {
            args.join(" ")
        }
    }

    /// 使用上下文查询身份
    #[command("查看身份信息")]
    async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String {
        let sender = ctx.sender_id();
        let nickname = ctx.event.sender_nickname().unwrap_or("未知");
        let scope = if ctx.is_group() { "群聊" } else { "私聊" };
        format!("ID: {sender}\n昵称: {nickname}\n环境: {scope}")
    }

    /// 管理员命令：查询群信息
    #[command("查看群信息", aliases = ["gi"], role = "admin")]
    async fn group_info(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
        let Some(gid) = ctx.group_id_i64() else {
            return CommandPluginSignal::Reply(Message::text("此命令仅在群聊中可用"));
        };

        match ctx.onebot_actions().get_group_info(gid, false).await {
            Ok(info) => CommandPluginSignal::Reply(Message::text(format!(
                "群名: {}\n群号: {}\n成员数: {}",
                info.group_name, info.group_id, info.member_count.unwrap_or(0),
            ))),
            Err(e) => CommandPluginSignal::Reply(Message::text(format!("查询失败: {e}"))),
        }
    }

    /// 仅群聊命令
    #[command("仅群聊打招呼", scope = "group")]
    async fn group_only(&self, ctx: &CommandPluginContext<'_>) -> String {
        format!("群 {} 的朋友你好！", ctx.group_id())
    }

    /// 仅私聊命令
    #[command("仅私聊悄悄话", scope = "private")]
    async fn private_only(&self) -> &str {
        "这是一条只在私聊中可见的消息~"
    }

    /// 终止插件链
    #[command("阻止后续插件处理", hidden)]
    async fn stop(&self) -> CommandPluginSignal {
        CommandPluginSignal::Block(Message::text("插件链已终止"))
    }
}
```
