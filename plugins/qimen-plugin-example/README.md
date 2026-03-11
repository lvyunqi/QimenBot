# QimenBot 示例插件

这是 QimenBot 框架的**官方示例插件**，通过真实可运行的代码，全面展示框架的各项核心功能。如果你是第一次开发 QimenBot 插件，建议从这里开始。

## 目录

- [快速开始](#快速开始)
- [项目结构](#项目结构)
- [模块总览](#模块总览)
  - [basic.rs — 基础命令](#basicrs--基础命令)
  - [message_demo.rs — 消息构建与提取](#message_demors--消息构建与提取)
  - [event_demo.rs — 系统事件处理](#event_demors--系统事件处理)
  - [interceptor_demo.rs — 拦截器](#interceptor_demors--拦截器)
- [宏系统详解](#宏系统详解)
  - [`#[module]` — 模块声明宏](#module--模块声明宏)
  - [`#[commands]` — 命令/事件扫描宏](#commands--命令事件扫描宏)
  - [`#[command]` — 命令定义宏](#command--命令定义宏)
  - [`#[notice]` / `#[request]` / `#[meta]` — 系统事件宏](#notice--request--meta--系统事件宏)
  - [宏展开完整示例](#宏展开完整示例)
- [核心概念详解](#核心概念详解)
  - [什么是 Module？](#什么是-module)
  - [命令插件的工作流程](#命令插件的工作流程)
  - [CommandPluginSignal 返回信号](#commandpluginsignal-返回信号)
  - [SystemPluginSignal 返回信号](#systempluginsignal-返回信号)
  - [MessageBuilder 消息构建器](#messagebuilder-消息构建器)
  - [拦截器的运行机制](#拦截器的运行机制)
- [如何在 Official Host 中启用](#如何在-official-host-中启用)
- [以此为模板创建你自己的插件](#以此为模板创建你自己的插件)

---

## 快速开始

**编译检查**（确保代码无误）：

```bash
cargo check --package qimen-plugin-example
```

**运行测试**：

```bash
cargo test --package qimen-plugin-example
```

如果以上命令都正常通过，说明插件代码没有问题，可以放心阅读和修改。

---

## 项目结构

```
plugins/qimen-plugin-example/
├── Cargo.toml              # 依赖配置
├── README.md               # 你正在看的文档
└── src/
    ├── lib.rs              # 入口：声明并导出所有模块
    ├── basic.rs            # 基础命令（ping、echo、whoami、group-info、ban、stop）
    ├── message_demo.rs     # 消息构建与提取（rich、parse、card、reply-quote、keyboard）
    ├── event_demo.rs       # 系统事件处理（戳一戳、入群欢迎、撤回、好友请求、群邀请、心跳）
    └── interceptor_demo.rs # 拦截器（日志记录、冷却限频）
```

`lib.rs` 是整个插件的入口文件，它的作用很简单：

```rust
// 声明子模块
mod basic;
mod event_demo;
mod interceptor_demo;
mod message_demo;

// 把各模块的结构体导出，让外部（比如 official host）能用到
pub use basic::BasicModule;
pub use event_demo::EventDemoModule;
pub use interceptor_demo::{CooldownInterceptor, LoggingInterceptor};
pub use message_demo::MessageDemoModule;
```

---

## 模块总览

### `basic.rs` — 基础命令

这个文件展示了**写命令最常用的几种模式**。所有命令都在一个 `impl` 块中定义，通过宏自动注册。

#### 命令列表

| 命令 | 别名 | 说明 | 展示了什么 |
|------|------|------|-----------|
| `/ping` | — | 回复 `pong!` | 最简单的命令，无参数，直接返回 `Message` |
| `/echo <文本>` | `/e` | 把你说的话复读回来 | 带参数 `args: Vec<String>` + 别名 `aliases` |
| `/whoami` | — | 显示你的 ID、昵称、角色、聊天场景 | `CommandPluginContext` 的各种便捷方法 |
| `/group-info` | `/gi` | 显示当前群的名称和成员数 | `OneBotActionClient` 调用 API |
| `/ban <用户ID> [秒数]` | — | 在群里禁言某人（默认 60 秒） | `role = "admin"` 权限控制 + `set_group_ban` |
| `/stop` | — | 回复后终止插件链 | `Block` 信号 |

#### 代码要点

**最简命令** — 不需要任何上下文，直接返回消息：

```rust
#[command("Reply with pong", examples = ["/ping"], category = "examples")]
async fn ping(&self) -> Message {
    Message::text("pong!")
}
```

**带参数的命令** — 框架会自动把命令后面的文字按空格拆分，传入 `args`：

```rust
#[command("Echo back the given text", aliases = ["e"],
          examples = ["/echo hello", "/e world"], category = "examples")]
async fn echo(&self, args: Vec<String>) -> Message {
    let text = if args.is_empty() {
        "(empty)".to_string()
    } else {
        args.join(" ")
    };
    Message::text(format!("echo: {text}"))
}
```

> 用户发送 `/echo hello world` 时，`args` 会是 `["hello", "world"]`。

**获取发送者信息** — 通过 `CommandPluginContext` 读取：

```rust
#[command("Show your identity info", examples = ["/whoami"], category = "examples")]
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
    let sender = ctx.sender_id().unwrap_or("unknown");        // 发送者 ID
    let nickname = ctx.event.sender_nickname().unwrap_or("?"); // 昵称
    let role = ctx.event.sender_role().unwrap_or("?");         // 角色：owner/admin/member
    let is_group = ctx.is_group();                              // 是否群聊
    // ...
}
```

**调用 OneBot API** — 通过 `ctx.onebot_actions()` 获取客户端：

```rust
let actions = ctx.onebot_actions();
let info = actions.get_group_info(group_id, false).await?;
```

常用 API 还有：`send_group_msg`、`send_private_msg`、`set_group_ban`、`get_group_member_info` 等。

**权限控制** — 加上 `role = "admin"` 后，只有群管理员/群主才能使用：

```rust
#[command("Ban a user", role = "admin", examples = ["/ban 123456 60"], category = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
    // 普通成员执行此命令时，框架会自动拒绝，不会进入这个函数
    // ...
}
```

---

### `message_demo.rs` — 消息构建与提取

这个文件展示了如何**构建各种类型的消息**，以及如何**从收到的消息中提取信息**。

#### 命令列表

| 命令 | 别名 | 说明 | 展示了什么 |
|------|------|------|-----------|
| `/rich` | — | 发送包含文字、@、表情、图片、链接的富媒体消息 | `MessageBuilder` 链式调用 |
| `/parse` | — | 分析当前消息的内容（@列表、图片、引用等） | `Message` 的提取方法 |
| `/card` | — | 发送一个分享卡片 | `MessageBuilder.share()` |
| `/reply-quote` | `/rq` | 引用回复当前消息 | `MessageBuilder.reply(message_id)` |
| `/keyboard` | `/kb` | 发送带交互按钮的键盘 | `KeyboardBuilder` |

#### 代码要点

**MessageBuilder 链式构建** — 像搭积木一样拼消息：

```rust
Message::builder()
    .text("Hello ")           // 文字
    .at(sender)               // @某人
    .text("\n")
    .face("21")               // QQ 表情（编号）
    .image("https://...")     // 图片（URL 或本地路径）
    .share("https://...", "标题")  // 分享链接卡片
    .build()                  // 构建完成，返回 Message
```

> `MessageBuilder` 支持的类型还有：`record`（语音）、`video`（视频）、`at_all`（@全体）、`reply`（引用回复）、`markdown`、`xml`、`json_msg` 等。

**从消息中提取信息** — 用 `Message` 的各种方法：

```rust
let msg = ctx.message().unwrap();

msg.plain_text()     // 获取纯文本内容（过滤掉图片、@等）
msg.at_list()        // 获取所有 @的用户 ID 列表，如 ["123", "456"]
msg.image_urls()     // 获取所有图片的 URL
msg.has_reply()      // 是否包含引用回复
msg.reply_id()       // 获取引用的消息 ID
msg.has_image()      // 是否包含图片
msg.has_at_all()     // 是否包含 @全体成员
```

**引用回复** — 用 `reply()` 指定要引用的消息 ID：

```rust
let msg = Message::builder()
    .reply(message_id.to_string())    // 引用某条消息
    .text("这是一条引用回复")           // 追加文字
    .build();
```

**交互键盘** — 用 `KeyboardBuilder` 创建按钮：

```rust
let kb = KeyboardBuilder::new()
    .command_button("Ping", "/ping")       // 点击后发送 /ping 命令
    .command_button("Whoami", "/whoami")
    .row()                                  // 换行
    .jump_button("GitHub", "https://...")   // 点击后打开链接
    .row()
    .build();

Message::builder()
    .text("请选择操作：")
    .keyboard(kb)
    .build()
```

> 按钮类型有三种：`command_button`（发送命令）、`jump_button`（跳转链接）、`callback_button`（触发回调）。

---

### `event_demo.rs` — 系统事件处理

这个文件展示了如何处理**除消息命令以外的各种事件**，比如戳一戳、入群、撤回、好友请求等。

#### 事件处理器列表

| 宏 | 监听的事件 | 说明 | 展示了什么 |
|----|-----------|------|-----------|
| `#[notice(GroupPoke, PrivatePoke)]` | 戳一戳 | 被戳时回复"别戳我" | 判断是否戳了机器人自己 |
| `#[notice(GroupIncreaseApprove, GroupIncreaseInvite)]` | 新成员入群 | 发送欢迎消息并 @新人 | `onebot_actions().send_group_msg()` |
| `#[notice(GroupRecall)]` | 消息撤回 | 提示谁撤回了什么消息 | 从原始 JSON 读取事件字段 |
| `#[request(Friend)]` | 好友请求 | 自动同意 | `ApproveFriend` 信号 |
| `#[request(GroupInvite)]` | 群邀请 | 自动同意 | `ApproveGroupInvite` 信号 |
| `#[meta(Heartbeat)]` | 心跳 | 记录日志 | `#[meta]` 宏 |

#### 代码要点

**监听通知事件** — 用 `#[notice(...)]` 宏，括号内填事件类型：

```rust
#[notice(GroupPoke, PrivatePoke)]  // 同时监听群戳一戳和私聊戳一戳
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    // ctx.event 是原始 JSON (serde_json::Value)，可以读取任意字段
    let target = ctx.event.get("target_id").and_then(|v| v.as_i64());
    let self_id = ctx.event.get("self_id").and_then(|v| v.as_i64());
    // ...
}
```

**读取事件字段** — `ctx.event` 是一个 `serde_json::Value`，用 `.get("字段名")` 读取：

```rust
let user_id = ctx.event.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0);
let group_id = ctx.event.get("group_id").and_then(|v| v.as_i64()).unwrap_or(0);
let flag = ctx.event.get("flag").and_then(|v| v.as_str()).unwrap_or("");
```

**在事件处理器中发送消息** — 不能直接 `Reply`（因为通知事件没有"回复"目标），需要用 `onebot_actions` 主动发送：

```rust
let actions = ctx.onebot_actions();
let _ = actions.send_group_msg(group_id, welcome_message).await;
```

**处理好友/群请求** — 返回特殊的信号即可：

```rust
// 同意好友请求
SystemPluginSignal::ApproveFriend {
    flag,               // 请求标识（从事件中获取）
    remark: None,       // 备注（可选）
}

// 同意群邀请
SystemPluginSignal::ApproveGroupInvite {
    flag,               // 请求标识
    sub_type,           // 子类型（从事件中获取，一般是 "invite"）
}
```

> 你也可以用 `RejectFriend` 或 `RejectGroupInvite` 来拒绝请求。

#### 可监听的事件类型速查

**通知事件**（`#[notice(...)]`）：

| 类型 | 说明 |
|------|------|
| `GroupPoke` / `PrivatePoke` | 戳一戳 |
| `GroupIncreaseApprove` / `GroupIncreaseInvite` | 新成员入群（管理同意 / 被邀请） |
| `GroupDecreaseLeave` / `GroupDecreaseKick` / `GroupDecreaseKickMe` | 成员退群 / 被踢 / 机器人被踢 |
| `GroupRecall` / `FriendRecall` | 群消息撤回 / 好友消息撤回 |
| `GroupAdminSet` / `GroupAdminUnset` | 设置/取消管理员 |
| `GroupBanBan` / `GroupBanLiftBan` | 禁言 / 解除禁言 |
| `FriendAdd` | 新好友添加成功 |
| `GroupUpload` | 群文件上传 |
| `GroupCard` | 群名片变更 |
| `EssenceAdd` / `EssenceDelete` | 精华消息添加 / 移除 |

**请求事件**（`#[request(...)]`）：

| 类型 | 说明 |
|------|------|
| `Friend` | 好友请求 |
| `GroupAdd` | 主动加群请求 |
| `GroupInvite` | 被邀请入群 |

**元事件**（`#[meta(...)]`）：

| 类型 | 说明 |
|------|------|
| `Heartbeat` | 心跳 |
| `LifecycleEnable` / `LifecycleDisable` / `LifecycleConnect` | 生命周期 |

---

### `interceptor_demo.rs` — 拦截器

拦截器在**每条消息事件**被处理的前后运行，适合做全局性的事情，比如日志、限频、黑名单。

本文件包含两个拦截器：

#### 1. `LoggingInterceptor` — 日志记录

在每条消息处理前后打印日志，展示了 `NormalizedEvent` 的各种便捷方法：

```rust
pub struct LoggingInterceptor;

#[async_trait]
impl MessageEventInterceptor for LoggingInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        let sender = event.sender_id().unwrap_or("unknown");   // 发送者 ID
        let chat = event.chat_id().unwrap_or("unknown");       // 聊天 ID（群号或私聊对方ID）
        let text = event.plain_text();                          // 消息纯文本
        let is_group = event.is_group();                        // 是否群聊
        let is_private = event.is_private();                    // 是否私聊
        tracing::info!(bot_id, sender, chat, "incoming message");
        true  // 返回 true = 放行，返回 false = 拦截
    }

    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        // 所有插件处理完毕后执行
        tracing::debug!("processing completed");
    }
}
```

#### 2. `CooldownInterceptor` — 冷却限频

每个用户 3 秒内只能发一条有效消息，展示了**如何在拦截器中维护状态**：

```rust
pub struct CooldownInterceptor {
    // 用 Mutex 包裹 HashMap，记录每个用户的最后发送时间
    last_message: Mutex<HashMap<String, Instant>>,
}
```

工作原理：
1. 收到消息时，检查该用户距离上次发送是否超过 3 秒
2. 如果**不到 3 秒** → 返回 `false`，拦截此消息（后续插件不会收到）
3. 如果**超过 3 秒** → 更新时间戳，返回 `true`，放行消息

#### 如何注册拦截器

在 `#[module(...)]` 宏中用 `interceptors = [...]` 声明：

```rust
#[module(id = "my-plugin", version = "0.1.0",
         interceptors = [LoggingInterceptor, CooldownInterceptor])]
```

> 拦截器需要实现 `MessageEventInterceptor` trait。如果拦截器有状态（像 `CooldownInterceptor`），需要实现 `new()` 方法，宏会自动调用它来创建实例。无状态的拦截器（像 `LoggingInterceptor`）只需要是一个空结构体即可。

---

## 宏系统详解

QimenBot 的插件开发高度依赖过程宏（proc macro），理解它们的工作原理是写插件的关键。这一节会把每个宏拆开来讲，告诉你**它做了什么**、**怎么用**、**背后发生了什么**。

### `#[module]` — 模块声明宏

`#[module]` 标记在 `impl` 块上面，告诉框架"这是一个插件模块"。

#### 基本用法

```rust
#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    // ... 方法定义
}
```

#### 参数说明

| 参数 | 必填 | 默认值 | 说明 |
|------|------|--------|------|
| `id` | 是 | — | 模块唯一标识，用于配置文件中启用/禁用 |
| `version` | 否 | `"0.1.0"` | 模块版本号 |
| `name` | 否 | 结构体名（如 `MyPlugin`） | 显示名称 |
| `description` | 否 | `""` | 模块描述 |
| `system_plugins` | 否 | `[]` | 额外的 SystemPlugin 类型列表 |
| `interceptors` | 否 | `[]` | 拦截器类型列表 |

#### 它在编译期间做了什么？

当你写下：

```rust
#[module(id = "example-basic", version = "0.1.0",
         name = "Basic Commands",
         interceptors = [LoggingInterceptor])]
#[commands]
impl BasicModule {
    // ...
}
```

宏会在**编译期**自动帮你生成以下代码（你不需要手写）：

```rust
// 1. 自动创建一个空结构体（你不需要自己写 struct BasicModule;）
pub struct BasicModule;

// 2. 在结构体上生成隐藏常量，存储模块元信息
impl BasicModule {
    pub const __QIMEN_MODULE_ID: &'static str = "example-basic";
    pub const __QIMEN_MODULE_VERSION: &'static str = "0.1.0";
    pub const __QIMEN_MODULE_NAME: &'static str = "Basic Commands";
    pub const __QIMEN_MODULE_DESCRIPTION: &'static str = "";

    // 3. 生成拦截器创建函数
    pub fn __qimen_interceptors() -> Vec<Arc<dyn MessageEventInterceptor>> {
        vec![
            // 无状态拦截器：直接用结构体字面量创建
            Arc::new(LoggingInterceptor) as Arc<dyn MessageEventInterceptor>,
        ]
    }
}

// 4. 实现 Module trait（由 #[commands] 宏补全）
```

> 所以你在代码里**只写 `impl BasicModule`，不写 `struct BasicModule;`**，是因为 `#[module]` 宏帮你自动生成了结构体定义。

---

### `#[commands]` — 命令/事件扫描宏

`#[commands]` 紧跟在 `#[module]` 下面，标记在 `impl` 块上。它会扫描 `impl` 块里所有带 `#[command]`、`#[notice]`、`#[request]`、`#[meta]` 标注的方法，自动生成 `CommandPlugin` 和 `SystemPlugin` 实现。

```rust
#[module(id = "my-plugin", version = "0.1.0")]
#[commands]    // ◄── 这个宏做实际的代码生成
impl MyPlugin {
    #[command("...")]
    async fn some_cmd(&self) -> Message { ... }

    #[notice(GroupPoke)]
    async fn on_poke(&self) -> Message { ... }
}
```

`#[commands]` 宏做了这些事：

1. **保留你写的所有方法**（去掉 `#[command]` 等宏标注）
2. 如果有 `#[command]` 方法 → 生成一个隐藏的 `CommandPlugin` 实现，包含命令路由表
3. 如果有 `#[notice]`/`#[request]`/`#[meta]` 方法 → 生成一个隐藏的 `SystemPlugin` 实现
4. 生成 `Module` trait 实现，把上面的 CommandPlugin、SystemPlugin、拦截器组装起来

> `#[system]` 是 `#[commands]` 的别名，功能完全一样。

---

### `#[command]` — 命令定义宏

标记在 `impl` 块内的方法上，把这个方法注册为一个聊天命令。

#### 完整参数说明

```rust
#[command(
    "Echo back the given text",     // 第一个字符串 = 命令描述（必填）
    name = "echo",                  // 命令名（可选，默认从函数名推导）
    aliases = ["e", "复读"],         // 别名列表（可选）
    examples = ["/echo hello"],     // 使用示例，展示在 /help 中（可选）
    category = "examples",          // 分类（可选，默认 "general"）
    role = "admin",                 // 权限要求（可选，默认 Anyone）
    hidden,                         // 在帮助列表中隐藏（可选，无需赋值）
)]
async fn echo(&self, args: Vec<String>) -> Message { ... }
```

| 参数 | 必填 | 默认值 | 说明 |
|------|------|--------|------|
| 第一个字符串 | 是 | — | 命令描述，显示在 `/help` 中 |
| `name` | 否 | 函数名（`_` → `-`） | 命令触发名 |
| `aliases` | 否 | `[]` | 别名列表 |
| `examples` | 否 | `[]` | 使用示例 |
| `category` | 否 | `"general"` | 命令分类 |
| `role` | 否 | `Anyone` | `"admin"` 或 `"owner"` |
| `hidden` | 否 | `false` | 加上此标志则 `/help` 中不显示 |

#### 命令名自动推导规则

**最重要的一点**：如果你没写 `name = "xxx"`，宏会自动拿**函数名**当命令名，并且把**下划线 `_` 替换成连字符 `-`**。

源码逻辑（`crates/qimen-plugin-derive/src/lib.rs:471`）：

```rust
let name = content
    .name
    .unwrap_or_else(|| method_name.replace('_', "-"));
//                      ^^^^^^^^^^^ 函数名    ^^^^^^^^^ 下划线变连字符
```

实际效果：

| 函数名 | 推导出的命令名 | 用户输入 |
|--------|---------------|---------|
| `ping` | `"ping"` | `/ping` |
| `echo` | `"echo"` | `/echo hello` |
| `group_info` | `"group-info"` | `/group-info` |
| `reply_quote` | `"reply-quote"` | `/reply-quote` |
| `my_cool_cmd` | `"my-cool-cmd"` | `/my-cool-cmd` |

如果你想让命令名和函数名不同，手动指定 `name`：

```rust
#[command("Say hi", name = "hi")]
async fn my_internal_function_name(&self) -> Message {
    // 用户发 /hi 触发，跟函数名无关
    Message::text("hello!")
}
```

#### 方法签名的灵活组合

宏会自动检测你的函数签名里有哪些参数，决定怎么调用你的方法：

**1. 无参数** — 最简单，不需要任何上下文：

```rust
#[command("Reply with pong")]
async fn ping(&self) -> Message {
    Message::text("pong!")
}
```

**2. 只有 `args`** — 框架自动把命令后的文字按空格拆分传入：

```rust
#[command("Echo text")]
async fn echo(&self, args: Vec<String>) -> Message {
    // 用户发 "/echo hello world" → args = ["hello", "world"]
    Message::text(args.join(" "))
}
```

**3. 只有 `ctx`** — 获取完整的命令上下文（发送者、群、机器人等）：

```rust
#[command("Show your info")]
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
    let sender = ctx.sender_id().unwrap_or("?");
    CommandPluginSignal::Reply(Message::text(format!("You are {sender}")))
}
```

**4. `ctx` + `args`** — 两个都要（ctx 必须在前）：

```rust
#[command("Ban a user", role = "admin")]
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
    let group_id = ctx.group_id_i64().unwrap();
    let user_id: i64 = args[0].parse().unwrap();
    // ...
}
```

> 宏的判断逻辑是：看参数个数 + 第一个参数的类型名是否包含 `PluginContext`。

#### 返回值自动转换

命令方法的返回值会自动转为 `CommandPluginSignal`：

| 你返回的类型 | 框架自动转为 | 效果 |
|-------------|------------|------|
| `Message` | `CommandPluginSignal::Reply(msg)` | 回复消息 |
| `String` | `Reply(Message::text(s))` | 回复文本 |
| `&str` | `Reply(Message::text(s))` | 回复文本 |
| `CommandPluginSignal` | 直接使用 | 完全控制 |
| `Result<T, E>` | Ok → 正常转换，Err → `Reply("Error: ...")` | 错误自动回复 |

所以这些写法**效果完全一样**：

```rust
// 写法 1：返回 Message
async fn ping(&self) -> Message {
    Message::text("pong!")
}

// 写法 2：返回 &str
async fn ping(&self) -> &str {
    "pong!"
}

// 写法 3：返回 CommandPluginSignal
async fn ping(&self) -> CommandPluginSignal {
    CommandPluginSignal::Reply(Message::text("pong!"))
}
```

---

### `#[notice]` / `#[request]` / `#[meta]` — 系统事件宏

标记在方法上，让这个方法响应特定的系统事件。

#### 用法

```rust
// 监听通知事件（可以同时监听多个类型）
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

// 监听请求事件
#[request(Friend)]
async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

// 监听元事件
#[meta(Heartbeat)]
async fn on_heartbeat(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }
```

#### 方法签名组合

和 `#[command]` 类似，系统事件方法也支持灵活的签名：

**1. 无参数** — 最简单：

```rust
#[notice(GroupPoke)]
async fn on_poke(&self) -> Message {
    Message::text("别戳了！")
}
```

**2. 只有 `ctx`** — 获取事件上下文：

```rust
#[notice(GroupPoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let target = ctx.event.get("target_id").and_then(|v| v.as_i64());
    // ...
}
```

**3. 只有 `route`** — 根据具体的路由类型做不同处理：

```rust
#[notice(GroupAdminSet, GroupAdminUnset)]
async fn on_admin(&self, route: &SystemNoticeRoute) -> Message {
    match route {
        SystemNoticeRoute::GroupAdminSet => Message::text("新管理员已设置"),
        SystemNoticeRoute::GroupAdminUnset => Message::text("管理员已取消"),
        _ => Message::text(""),
    }
}
```

**4. `ctx` + `route`** — 两个都要（ctx 在前）：

```rust
#[request(Friend, GroupAdd)]
async fn on_request(&self, ctx: &SystemPluginContext<'_>, route: &SystemRequestRoute) -> SystemPluginSignal {
    // 可以根据 route 区分是好友请求还是加群请求
    // ...
}
```

#### 返回值自动转换

和命令一样，系统事件方法的返回值也支持自动转换：

| 你返回的类型 | 框架自动转为 |
|-------------|------------|
| `Message` | `SystemPluginSignal::Reply(msg)` |
| `String` / `&str` | `Reply(Message::text(s))` |
| `SystemPluginSignal` | 直接使用 |
| `Result<T, E>` | Ok → 正常转换，Err → `Reply("Error: ...")` |

---

### 宏展开完整示例

为了帮你理解全貌，这是一个简单插件和宏展开后的**对比**：

#### 你写的代码

```rust
#[module(id = "demo", version = "0.1.0", interceptors = [LoggingInterceptor])]
#[commands]
impl DemoPlugin {
    #[command("Say pong", aliases = ["p"])]
    async fn ping(&self) -> &str {
        "pong!"
    }

    #[notice(GroupPoke)]
    async fn on_poke(&self) -> Message {
        Message::text("别戳了！")
    }
}
```

#### 宏帮你自动生成的代码（简化版）

```rust
// ① #[module] 生成：结构体 + 隐藏常量
pub struct DemoPlugin;

impl DemoPlugin {
    pub const __QIMEN_MODULE_ID: &'static str = "demo";
    pub const __QIMEN_MODULE_VERSION: &'static str = "0.1.0";
    pub const __QIMEN_MODULE_NAME: &'static str = "DemoPlugin";
    pub const __QIMEN_MODULE_DESCRIPTION: &'static str = "";

    pub fn __qimen_interceptors() -> Vec<Arc<dyn MessageEventInterceptor>> {
        vec![Arc::new(LoggingInterceptor)]
    }
}

// ② 你的原始方法保留（去掉宏标注）
impl DemoPlugin {
    async fn ping(&self) -> &str {
        "pong!"
    }
    async fn on_poke(&self) -> Message {
        Message::text("别戳了！")
    }
}

// ③ #[commands] 生成：CommandPlugin 实现
struct __QimenCmdPlugin_DemoPlugin;

impl CommandPlugin for __QimenCmdPlugin_DemoPlugin {
    fn commands(&self) -> Vec<CommandDefinition> {
        vec![
            CommandDefinition::new("ping", "Say pong")
                .aliases(&["p"])
                .category("general"),
        ]
    }

    async fn on_command(&self, ctx: &CommandPluginContext<'_>, invocation: &CommandInvocation)
        -> Option<CommandPluginSignal>
    {
        match invocation.definition.name {
            "ping" => {
                let inst = DemoPlugin;
                // 你的返回值 &str 被 IntoCommandSignal 自动转为 Reply
                Some(IntoCommandSignal::into_signal(inst.ping().await))
            }
            _ => Some(CommandPluginSignal::Continue),
        }
    }
}

// ④ #[commands] 生成：SystemPlugin 实现
struct __QimenSysPlugin_DemoPlugin;

impl SystemPlugin for __QimenSysPlugin_DemoPlugin {
    async fn on_notice(&self, ctx: &SystemPluginContext<'_>, route: &SystemNoticeRoute)
        -> Option<SystemPluginSignal>
    {
        match route {
            SystemNoticeRoute::GroupPoke => {
                let inst = DemoPlugin;
                Some(IntoSystemSignal::into_signal(inst.on_poke().await))
            }
            _ => None,
        }
    }
}

// ⑤ #[commands] 生成：Module trait 实现
impl Module for DemoPlugin {
    fn id(&self) -> &'static str { Self::__QIMEN_MODULE_ID }
    async fn on_load(&self) -> Result<()> { Ok(()) }

    fn command_plugins(&self) -> Vec<Arc<dyn CommandPlugin>> {
        vec![Arc::new(__QimenCmdPlugin_DemoPlugin)]
    }
    fn system_plugins(&self) -> Vec<Arc<dyn SystemPlugin>> {
        vec![Arc::new(__QimenSysPlugin_DemoPlugin)]
    }
    fn interceptors(&self) -> Vec<Arc<dyn MessageEventInterceptor>> {
        Self::__qimen_interceptors()
    }
}
```

> 所以你只写了 **~15 行**，宏帮你生成了 **~80 行**的胶水代码。这就是过程宏的价值 — 你只关注业务逻辑，框架对接代码全自动生成。

---

## 核心概念详解

### 什么是 Module？

`Module` 是 QimenBot 插件系统的**最小注册单元**。一个 Module 可以包含：

- **命令插件**（CommandPlugin）：处理用户发送的 `/命令`
- **系统插件**（SystemPlugin）：处理通知、请求、元事件
- **拦截器**（MessageEventInterceptor）：在消息处理前后执行

用 `#[module]` 宏标注在 `impl` 块上即可自动生成 `Module` 实现：

```rust
#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("...")]
    async fn my_cmd(&self) -> Message { ... }

    #[notice(GroupPoke)]
    async fn on_poke(&self) -> Message { ... }
}
```

`#[commands]` 宏会扫描 `impl` 块中的 `#[command]`、`#[notice]`、`#[request]`、`#[meta]` 标注，自动生成对应的 `CommandPlugin` 和 `SystemPlugin` 实现。

---

### 命令插件的工作流程

```
用户发送 "/echo hello"
        │
        ▼
  ┌─────────────┐
  │  拦截器链     │ ◄── pre_handle() 逐个调用，任意一个返回 false 则中止
  │  (Interceptors) │
  └──────┬──────┘
         │ 全部返回 true
         ▼
  ┌─────────────┐
  │  命令匹配     │ ◄── 框架根据命令名和别名找到对应的 handler
  │  (Routing)    │
  └──────┬──────┘
         │ 找到 echo 命令
         ▼
  ┌─────────────┐
  │  执行 handler │ ◄── echo(&self, args: Vec<String>) 被调用
  │  (Execute)    │     args = ["hello"]
  └──────┬──────┘
         │ 返回信号
         ▼
  ┌─────────────┐
  │  处理信号     │ ◄── Reply → 发送回复
  │  (Signal)     │     Continue → 交给下一个插件
  └──────┬──────┘     Block → 发送回复并终止
         │
         ▼
  ┌─────────────┐
  │  拦截器链     │ ◄── after_completion() 逐个调用
  │  (Cleanup)    │
  └─────────────┘
```

---

### CommandPluginSignal 返回信号

命令处理函数可以返回 `Message`（自动包装为 `Reply`）或 `CommandPluginSignal`：

| 信号 | 效果 | 典型场景 |
|------|------|---------|
| `Reply(Message)` | 发送回复，然后**继续**让后续插件处理 | 大多数命令 |
| `Continue` | 不回复，让后续插件处理 | 不在群聊时跳过 |
| `Block(Message)` | 发送回复，**阻止**后续插件处理此消息 | 独占式命令 |
| `Ignore` | 不回复，**阻止**后续插件处理 | 静默拦截 |

> 快捷返回：直接返回 `Message`、`String` 或 `&str` 会自动转为 `Reply`。

---

### SystemPluginSignal 返回信号

系统事件处理函数返回 `SystemPluginSignal`：

| 信号 | 效果 | 典型场景 |
|------|------|---------|
| `Continue` | 继续让后续插件处理 | 仅做日志记录时 |
| `Reply(Message)` | 发送消息到事件来源 | 戳一戳回复 |
| `ApproveFriend { flag, remark }` | 同意好友请求 | 自动加好友 |
| `RejectFriend { flag, reason }` | 拒绝好友请求 | 过滤垃圾请求 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 | 自动入群 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 | — |
| `Block(Message)` | 回复并阻止后续插件 | — |
| `Ignore` | 静默阻止后续插件 | — |

---

### MessageBuilder 消息构建器

`MessageBuilder` 用链式调用拼接各种消息段：

```rust
Message::builder()
    .text("文字")                              // 纯文本
    .at("123456")                              // @某人
    .at_all()                                  // @全体成员
    .image("https://example.com/pic.png")      // 图片
    .face("21")                                // QQ 表情
    .record("https://example.com/audio.mp3")   // 语音
    .video("https://example.com/video.mp4")    // 视频
    .share("https://example.com", "标题")      // 分享卡片
    .reply("12345")                            // 引用回复
    .markdown("**粗体** _斜体_")               // Markdown
    .keyboard(kb)                              // 交互按钮键盘
    .build()                                   // 构建完成
```

---

### 拦截器的运行机制

```
消息到达
    │
    ▼
Interceptor1.pre_handle()  →  true（放行）
    │
    ▼
Interceptor2.pre_handle()  →  true（放行）
    │
    ▼
  [插件处理消息]
    │
    ▼
Interceptor2.after_completion()   ◄── 注意：完成回调是逆序的
    │
    ▼
Interceptor1.after_completion()
```

如果任何一个拦截器的 `pre_handle()` 返回 `false`，消息就**不会被任何插件处理**。

---

## 如何在 Official Host 中启用

在配置文件中添加想要启用的模块 ID：

```toml
[official_host]
plugin_modules = [
    "example-basic",     # 基础命令（ping、echo、whoami 等）
    "example-message",   # 消息演示（rich、parse、card 等）
    "example-events",    # 事件处理（戳一戳、入群欢迎等）
]
```

> `"example-plugin"` 是 `"example-basic"` 的别名，向后兼容旧配置。

启用后，Official Host 会在加载前校验插件的 `api_version`，同时遵循 `config/plugin-state.toml` 中持久化的启用/禁用状态。

---

## 以此为模板创建你自己的插件

1. **复制目录**：

   ```bash
   cp -r plugins/qimen-plugin-example plugins/qimen-plugin-myplugin
   ```

2. **修改 `Cargo.toml`**：把 `name` 改成你自己的名字。

3. **修改 `#[module(...)]`**：设置你自己的 `id` 和 `version`。

4. **注册到 workspace**：在根目录 `Cargo.toml` 的 `[workspace] members` 中添加你的插件路径。

5. **注册到 Official Host**：在 `crates/qimen-official-host/src/lib.rs` 中添加你的模块的导入和注册逻辑。

6. **启用插件**：在配置文件的 `plugin_modules` 中添加你的模块 ID。

**最小可运行的插件**只需要这些代码：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    #[command("Say hello", examples = ["/hello"])]
    async fn hello(&self) -> Message {
        Message::text("Hello, world!")
    }
}
```

`Cargo.toml` 最少需要这些依赖：

```toml
[dependencies]
async-trait.workspace = true
qimen-error = { path = "../../crates/qimen-error" }
qimen-message = { path = "../../crates/qimen-message" }
qimen-plugin-api = { path = "../../crates/qimen-plugin-api" }
qimen-plugin-derive = { path = "../../crates/qimen-plugin-derive" }
```
