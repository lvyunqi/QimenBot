# 命令开发

命令是 QimenBot 最核心的交互方式。用户通过 `/命令名 参数` 的格式触发命令，框架将请求路由到对应的处理函数。

## 基础命令

### 最简命令

```rust
#[command("回复 pong")]
async fn ping(&self) -> &str {
    "pong!"
}
```

用户发送 `/ping`，Bot 回复 `pong!`。

### 命令名推导规则

如果你没有显式指定命令名，宏会自动从**函数名**推导：

| 函数名 | 推导出的命令名 | 用户输入 |
|--------|--------------|---------|
| `ping` | `ping` | `/ping` |
| `echo` | `echo` | `/echo` |
| `group_info` | `group-info` | `/group-info` |
| `my_cmd` | `my-cmd` | `/my-cmd` |

规则：函数名中的下划线 `_` 自动替换为连字符 `-`。

你也可以手动指定命令名：

```rust
#[command(name = "greet", desc = "打招呼")]
async fn my_greeting_function(&self) -> &str {
    "你好！"
}
```

## `#[command]` 属性

```rust
#[command(
    "命令描述",                   // 必填，位置参数
    aliases = ["e", "回显"],      // 别名列表
    examples = ["/echo hello"],  // 使用示例
    category = "tools",          // 分类（默认 "general"）
    role = "admin",              // 权限要求
    hidden,                      // 隐藏（不显示在 /help 中）
)]
```

| 属性 | 类型 | 默认值 | 说明 |
|------|------|-------|------|
| 第一个参数 | `&str` | — | 命令描述（必填） |
| `name` | `&str` | 函数名推导 | 显式指定命令名 |
| `aliases` | `[&str]` | `[]` | 别名列表 |
| `examples` | `[&str]` | `[]` | 使用示例（显示在 /help 中） |
| `category` | `&str` | `"general"` | 命令分类 |
| `role` | `&str` | — | 权限要求：`"admin"` 或 `"owner"` |
| `hidden` | flag | — | 隐藏命令 |

### 别名

```rust
#[command("回显文本", aliases = ["e", "回显"])]
async fn echo(&self, args: Vec<String>) -> String {
    args.join(" ")
}
```

现在用户可以通过 `/echo`、`/e`、`/回显` 三种方式触发这个命令。

### 权限控制

```rust
// 任何人都可以使用（默认）
#[command("回复 pong")]
async fn ping(&self) -> &str { "pong!" }

// 仅管理员可用
#[command("禁言用户", role = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
    // ...
}

// 仅所有者可用
#[command("关闭 Bot", role = "owner")]
async fn stop(&self) -> &str { "正在关闭..." }
```

## 方法签名

宏会根据你的方法签名**自动检测**需要注入的参数。以下是所有支持的签名组合：

### 无参数

```rust
#[command("回复 pong")]
async fn ping(&self) -> &str {
    "pong!"
}
```

### 只有参数

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

`args` 是命令后面的文本按空格拆分后的列表：

```
/echo hello world → args = ["hello", "world"]
/echo             → args = []
```

### 只有上下文

```rust
#[command("查看身份")]
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String {
    let sender = ctx.sender_id();
    let scope = if ctx.is_group() { "群聊" } else { "私聊" };
    format!("你的 ID: {sender}，当前环境: {scope}")
}
```

`CommandPluginContext` 提供了丰富的上下文信息，详见[上下文对象](#上下文对象)。

### 上下文 + 参数

```rust
#[command("禁言用户", role = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
    // ctx 必须在 args 前面
    let user_id = args.first().and_then(|s| s.parse::<i64>().ok());
    let duration = args.get(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(60);

    match (ctx.group_id_i64(), user_id) {
        (Some(group_id), Some(uid)) => {
            let client = ctx.onebot_actions();
            let _ = client.set_group_ban(group_id, uid, duration).await;
            CommandPluginSignal::Reply(Message::text(
                format!("已禁言用户 {uid} {duration} 秒")
            ))
        }
        _ => CommandPluginSignal::Reply(Message::text("用法: /ban <QQ号> [秒数]")),
    }
}
```

::: warning 参数顺序
当同时使用 `ctx` 和 `args` 时，**`ctx` 必须在 `args` 前面**：
```rust
// ✅ 正确
async fn cmd(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>)

// ❌ 错误
async fn cmd(&self, args: Vec<String>, ctx: &CommandPluginContext<'_>)
```
:::

## 上下文对象

`CommandPluginContext` 提供了访问事件信息和执行操作的入口：

### 常用便捷方法

```rust
async fn example(&self, ctx: &CommandPluginContext<'_>) {
    // ── 发送者信息 ──
    ctx.sender_id();        // 发送者 ID（字符串）
    ctx.sender_id_i64();    // 发送者 ID（i64）

    // ── 聊天环境 ──
    ctx.is_group();         // 是否群聊
    ctx.is_private();       // 是否私聊
    ctx.group_id();         // 群号（字符串，私聊为空）
    ctx.group_id_i64();     // 群号（Option<i64>）
    ctx.chat_id();          // 聊天 ID（群号或用户 ID）

    // ── 消息内容 ──
    ctx.plain_text();       // 纯文本内容
    ctx.message();          // 完整 Message 对象

    // ── OneBot API ──
    let client = ctx.onebot_actions(); // 获取 API 客户端
    let info = client.get_login_info().await;
}
```

### 原始事件访问

通过 `ctx.event` 可以访问 `NormalizedEvent`，它提供了更多底层字段：

```rust
let event = ctx.event;
event.sender_nickname();  // 发送者昵称
event.sender_role();      // 群角色（owner/admin/member）
event.sender_card();      // 群名片
event.message_id();       // 消息 ID
event.self_id();          // Bot 自身 ID
event.is_at_self();       // 是否 @了 Bot
```

## 返回值

命令处理函数可以返回多种类型，框架自动转换：

### 字符串

```rust
// &str
async fn cmd(&self) -> &str { "hello" }

// String
async fn cmd(&self) -> String { format!("hello {}", "world") }
```

### Message 对象

```rust
async fn cmd(&self) -> Message {
    Message::builder()
        .text("你好 ")
        .face(1)
        .build()
}
```

### CommandPluginSignal

完全控制行为：

```rust
async fn cmd(&self) -> CommandPluginSignal {
    // 回复并继续处理链
    CommandPluginSignal::Reply(Message::text("已处理"))

    // 不做任何事
    CommandPluginSignal::Continue

    // 回复并终止后续插件
    CommandPluginSignal::Block(Message::text("命令已拦截"))

    // 静默终止
    CommandPluginSignal::Ignore
}
```

### Result 类型

```rust
async fn cmd(&self) -> Result<String, Box<dyn std::error::Error>> {
    let data = fetch_something().await?;
    Ok(format!("结果: {data}"))
}
// 如果 Err → 自动回复 "Error: {错误信息}"
```

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

    /// 带参数和别名的命令
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
        let Some(group_id) = ctx.group_id_i64() else {
            return CommandPluginSignal::Reply(Message::text("此命令仅在群聊中可用"));
        };

        let client = ctx.onebot_actions();
        match client.get_group_info(group_id, false).await {
            Ok(info) => CommandPluginSignal::Reply(Message::text(format!(
                "群名: {}\n群号: {}\n成员数: {}",
                info.group_name,
                info.group_id,
                info.member_count.unwrap_or(0),
            ))),
            Err(e) => CommandPluginSignal::Reply(Message::text(format!("查询失败: {e}"))),
        }
    }

    /// 终止插件链
    #[command("阻止后续插件处理")]
    async fn stop(&self) -> CommandPluginSignal {
        CommandPluginSignal::Block(Message::text("插件链已终止"))
    }
}
```
