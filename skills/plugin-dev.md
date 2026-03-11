# QimenBot 插件开发参考 / Plugin Development Reference

> 通用参考文档，适用于任何 AI 编程工具。可直接喂给 AI 作为上下文。

## 项目概览

QimenBot 是基于 Rust 构建的高性能 Bot 框架，支持 OneBot 11 协议，使用宏驱动插件开发。

**关键路径**：
- 插件 API：`crates/qimen-plugin-api/src/lib.rs`
- 宏定义：`crates/qimen-plugin-derive/src/lib.rs`
- 消息类型：`crates/qimen-message/src/lib.rs`
- 示例插件：`plugins/qimen-plugin-example/src/`（完整参考）

**新插件结构**：`plugins/<name>/Cargo.toml` + `plugins/<name>/src/lib.rs`，在根 `Cargo.toml` workspace members 注册。

最小 Cargo.toml 依赖：
```toml
[dependencies]
async-trait.workspace = true
qimen-plugin-api = { path = "../../crates/qimen-plugin-api" }
qimen-plugin-derive = { path = "../../crates/qimen-plugin-derive" }
qimen-message = { path = "../../crates/qimen-message" }
qimen-error = { path = "../../crates/qimen-error" }
```

---

## 宏系统

### #[module] — 模块声明

```rust
use qimen_plugin_api::prelude::*;

#[module(
    id = "my-plugin",           // 必填：唯一标识
    version = "0.1.0",          // 可选，默认 "0.1.0"
    name = "My Plugin",         // 可选
    description = "描述",       // 可选
    interceptors = [MyInterceptor],  // 可选
)]
#[commands]
impl MyPlugin {
    // 命令和事件处理器写在这里
}
```

### #[command] — 命令处理器

```rust
#[command("命令描述",              // 必填
    aliases = ["别名1", "别名2"],  // 可选
    examples = ["/cmd arg"],       // 可选
    category = "general",          // 可选，默认 "general"
    role = "admin",                // 可选："admin" | "owner"，默认任何人
    hidden,                        // 可选：隐藏命令
)]
```

**4 种方法签名**：
```rust
async fn ping(&self) -> &str { "pong" }
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String { ... }
async fn echo(&self, args: Vec<String>) -> Message { ... }
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal { ... }
```

**返回值自动转换**：`&str`/`String` → Reply(text)，`Message` → Reply(msg)，`CommandPluginSignal` → 直接使用，`Result<T,E>` → Ok 转 T / Err 转错误消息。

### #[notice] / #[request] / #[meta] — 系统事件

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

#[request(Friend)]
async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or("").to_string();
    SystemPluginSignal::ApproveFriend { flag, remark: None }
}

#[meta(Heartbeat)]
async fn on_heartbeat(&self) -> SystemPluginSignal { SystemPluginSignal::Continue }
```

---

## 信号枚举

### CommandPluginSignal
| 变体 | 说明 |
|------|------|
| `Reply(Message)` | 回复，继续后续插件 |
| `Continue` | 跳过，继续 |
| `Block(Message)` | 回复并终止插件链 |
| `Ignore` | 静默终止 |

### SystemPluginSignal
| 变体 | 说明 |
|------|------|
| `Continue` | 继续 |
| `Reply(Message)` | 回复并继续 |
| `ApproveFriend { flag, remark }` | 同意好友 |
| `RejectFriend { flag, reason }` | 拒绝好友 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 |
| `Block(Message)` | 回复并终止 |
| `Ignore` | 静默终止 |

---

## 上下文 API

### CommandPluginContext
```
ctx.sender_id() -> &str              // QQ 号
ctx.sender_id_i64() -> Option<i64>
ctx.chat_id() -> &str
ctx.group_id() -> &str               // 私聊返回空
ctx.group_id_i64() -> Option<i64>
ctx.is_group() -> bool
ctx.is_private() -> bool
ctx.plain_text() -> String
ctx.message() -> Option<&Message>
ctx.onebot_actions() -> OneBotActionClient
```

### event 附加方法
```
event.sender_nickname() -> Option<&str>
event.sender_role() -> Option<&str>      // "owner"/"admin"/"member"
event.sender_card() -> Option<&str>
event.message_id() -> Option<i64>
event.self_id() -> Option<i64>
event.is_at_self() -> bool
event.is_group_admin_or_owner() -> bool
```

### SystemPluginContext
```
ctx.bot_id: &str
ctx.event: &Value    // 原始 JSON，用 ctx.event["field"] 访问
ctx.onebot_actions() -> OneBotActionClient
```

---

## 消息构建

### MessageBuilder
```rust
Message::builder()
    .text("文本").at("QQ号").at_all()
    .image("URL").flash_image("URL").face("1")
    .record("URL").video("URL")
    .reply(msg_id.to_string())
    .share("URL", "标题").location(lat, lon, "标题")
    .music("163", "歌曲ID")
    .xml("data").json_msg("data").markdown("# 标题")
    .keyboard(kb)
    .build()
```

### KeyboardBuilder
```rust
use qimen_message::keyboard::*;
let kb = KeyboardBuilder::new()
    .command_button("标签", "/命令")
    .jump_button("标签", "https://...")
    .callback_button("标签", "data")
    .style(ButtonStyle::Blue)
    .permission(ButtonPermission::All)
    .row()
    .build();
```

### 消息提取
```rust
let msg = ctx.message().unwrap();
msg.plain_text()      msg.at_list()        msg.has_at("123")
msg.has_at_all()      msg.image_urls()     msg.record_urls()
msg.video_urls()      msg.has_reply()      msg.reply_id()
```

---

## OneBotActionClient

```rust
let actions = ctx.onebot_actions();

// 消息
actions.send_group_msg(group_id, message).await
actions.send_private_msg(user_id, message).await
actions.delete_msg(message_id).await
actions.get_msg(message_id).await

// 群管理
actions.set_group_ban(group_id, user_id, duration).await
actions.set_group_kick(group_id, user_id, reject_add).await
actions.set_group_whole_ban(group_id, enable).await
actions.set_group_admin(group_id, user_id, enable).await
actions.set_group_card(group_id, user_id, card).await
actions.set_group_name(group_id, name).await

// 查询
actions.get_login_info().await
actions.get_group_info(group_id, no_cache).await
actions.get_group_member_info(group_id, user_id, no_cache).await
actions.get_group_member_list(group_id).await
actions.get_group_list().await
actions.get_friend_list().await

// 请求
actions.set_friend_add_request(flag, approve, remark).await
actions.set_group_add_request(flag, sub_type, approve, reason).await

// 扩展
actions.send_group_poke(group_id, user_id).await
actions.set_essence_msg(message_id).await
actions.upload_group_file(group_id, file, name, folder).await
```

---

## 通知/请求/元事件路由

**Notice**: GroupPoke, PrivatePoke, GroupIncreaseApprove, GroupIncreaseInvite, GroupDecreaseLeave, GroupDecreaseKick, GroupDecreaseKickMe, GroupRecall, FriendRecall, GroupBanBan, GroupBanLiftBan, FriendAdd, GroupUpload, GroupAdminSet, GroupAdminUnset, GroupCard, EssenceAdd, EssenceDelete, NotifyLuckyKing, NotifyHonor, OfflineFile, GroupReaction, MessageEmojiLike

**Request**: Friend, GroupAdd, GroupInvite

**Meta**: LifecycleEnable, LifecycleDisable, LifecycleConnect, Heartbeat

---

## 拦截器

```rust
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        true  // true=放行, false=拦截
    }
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {}
}
// 注册：#[module(id = "...", interceptors = [MyInterceptor])]
```

---

## 编码规范

- 中英文双语注释
- 使用 `tracing` 日志，不用 `println!`
- 编写后 `cargo check --workspace` 验证
