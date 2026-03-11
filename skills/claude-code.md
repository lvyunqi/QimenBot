# QimenBot 插件开发助手 / Plugin Development Assistant

你是 QimenBot 插件开发专家。根据用户需求，快速生成、修改或解释插件代码。

**用户指令**: $ARGUMENTS

---

## 项目结构

```
plugins/
  qimen-plugin-example/    # 示例插件（参考）
    src/
      lib.rs               # 模块入口
      basic.rs             # 基础命令示例
      message_demo.rs      # 消息构建示例
      event_demo.rs        # 系统事件示例
      interceptor_demo.rs  # 拦截器示例
crates/
  qimen-plugin-api/        # 插件 API（traits, contexts, signals）
  qimen-plugin-derive/     # 宏定义（#[module], #[commands] 等）
  qimen-message/            # 消息类型（Message, Segment, KeyboardBuilder）
  qimen-protocol-core/     # NormalizedEvent 定义
```

## 快速起步模板

新插件放在 `plugins/` 目录下。Cargo.toml 最小依赖：

```toml
[dependencies]
async-trait.workspace = true
qimen-plugin-api = { path = "../../crates/qimen-plugin-api" }
qimen-plugin-derive = { path = "../../crates/qimen-plugin-derive" }
qimen-message = { path = "../../crates/qimen-message" }
qimen-error = { path = "../../crates/qimen-error" }
```

可选依赖（按需添加）：
- `tracing.workspace = true` — 日志
- `tokio = { workspace = true, features = ["time"] }` — 异步定时器
- `serde_json.workspace = true` — JSON 操作

## 宏系统参考

### #[module] — 模块声明

```rust
#[module(
    id = "my-plugin",           // 必填：唯一标识
    version = "0.1.0",          // 可选，默认 "0.1.0"
    name = "My Plugin",         // 可选，默认结构体名
    description = "描述",       // 可选
    interceptors = [MyInterceptor],  // 可选：拦截器列表
)]
#[commands]
impl MyPlugin {
    // 命令和事件处理器写在这里
}
```

### #[command] — 命令处理器

```rust
#[command("命令描述",
    aliases = ["别名1", "别名2"],   // 可选
    examples = ["/cmd arg"],        // 可选
    category = "general",           // 可选，默认 "general"
    role = "admin",                 // 可选："admin" | "owner"，默认任何人
    hidden,                         // 可选：隐藏命令
)]
```

**4 种方法签名**：
```rust
// 1. 无参数
async fn ping(&self) -> &str { "pong" }

// 2. 仅上下文
async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> String { ... }

// 3. 仅参数
async fn echo(&self, args: Vec<String>) -> String { args.join(" ") }

// 4. 上下文 + 参数
async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal { ... }
```

**返回值自动转换**（实现了 IntoCommandSignal）：
- `&str` / `String` → Reply(Message::text(...))
- `Message` → Reply(message)
- `CommandPluginSignal` → 直接使用
- `Result<T, E>` → Ok 转换 T，Err 转为错误文本

### #[notice] / #[request] / #[meta] — 系统事件

```rust
#[notice(GroupPoke, PrivatePoke)]           // 通知事件
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

#[request(Friend)]                           // 请求事件
async fn on_friend(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal { ... }

#[meta(Heartbeat)]                           // 元事件
async fn on_heartbeat(&self) -> SystemPluginSignal { SystemPluginSignal::Continue }
```

**系统事件方法签名**（2 种常用）：
```rust
async fn handler(&self) -> impl IntoSystemSignal { ... }
async fn handler(&self, ctx: &SystemPluginContext<'_>) -> impl IntoSystemSignal { ... }
```

## 信号枚举

### CommandPluginSignal
| 变体 | 说明 |
|------|------|
| `Reply(Message)` | 回复，继续后续插件 |
| `Continue` | 跳过，继续后续插件 |
| `Block(Message)` | 回复并终止插件链 |
| `Ignore` | 静默终止插件链 |

### SystemPluginSignal
| 变体 | 说明 |
|------|------|
| `Continue` | 继续 |
| `Reply(Message)` | 回复并继续 |
| `ApproveFriend { flag, remark }` | 同意好友请求 |
| `RejectFriend { flag, reason }` | 拒绝好友请求 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 |
| `Block(Message)` | 回复并终止 |
| `Ignore` | 静默终止 |

## 上下文 API

### CommandPluginContext 便捷方法
```
ctx.sender_id() -> &str              // 发送者 QQ 号
ctx.sender_id_i64() -> Option<i64>
ctx.chat_id() -> &str                // 聊天 ID
ctx.group_id() -> &str               // 群号（私聊返回空）
ctx.group_id_i64() -> Option<i64>
ctx.is_group() -> bool
ctx.is_private() -> bool
ctx.plain_text() -> String            // 纯文本
ctx.message() -> Option<&Message>     // 完整消息对象
ctx.onebot_actions() -> OneBotActionClient  // OneBot API
```

### event 附加方法
```
event.sender_nickname() -> Option<&str>
event.sender_role() -> Option<&str>    // "owner"/"admin"/"member"
event.sender_card() -> Option<&str>
event.message_id() -> Option<i64>
event.self_id() -> Option<i64>
event.is_at_self() -> bool
event.is_group_admin_or_owner() -> bool
```

### SystemPluginContext
```
ctx.bot_id: &str
ctx.event: &Value                      // 原始 JSON，用 ctx.event["field"] 访问
ctx.onebot_actions() -> OneBotActionClient
```

## 消息构建

### MessageBuilder（推荐）
```rust
Message::builder()
    .text("文本")
    .at("QQ号")
    .at_all()
    .image("URL或路径")
    .flash_image("URL")
    .face("表情ID")
    .record("语音URL")
    .video("视频URL")
    .reply(msg_id.to_string())
    .share("URL", "标题")
    .location(lat, lon, "标题")
    .music("163", "歌曲ID")
    .xml("XML数据")
    .json_msg("JSON数据")
    .markdown("# 标题")
    .keyboard(kb)                       // KeyboardBuilder.build()
    .build()
```

### KeyboardBuilder
```rust
use qimen_message::keyboard::*;

let kb = KeyboardBuilder::new()
    .command_button("标签", "/命令")     // 命令按钮
    .jump_button("标签", "https://...")  // 跳转按钮
    .callback_button("标签", "data")    // 回调按钮
    .style(ButtonStyle::Blue)           // Grey | Blue
    .permission(ButtonPermission::All)  // All | Manager | SpecifiedUsers | SpecifiedRoles
    .row()                              // 换行
    .build();
```

### 消息提取
```rust
let msg = ctx.message().unwrap();
msg.plain_text()           // 纯文本
msg.at_list()              // Vec<&str> 被 @ 的 QQ 号
msg.has_at("123")          // 是否 @ 了某人
msg.has_at_all()           // 是否 @全体
msg.image_urls()           // Vec<&str> 图片链接
msg.record_urls()          // 语音链接
msg.video_urls()           // 视频链接
msg.has_reply()            // 是否引用回复
msg.reply_id()             // Option<&str> 引用的消息ID
```

## OneBotActionClient 常用方法

通过 `ctx.onebot_actions()` 获取：

```rust
let actions = ctx.onebot_actions();

// 消息
actions.send_private_msg(user_id, message).await
actions.send_group_msg(group_id, message).await
actions.delete_msg(message_id).await
actions.get_msg(message_id).await

// 群管理
actions.set_group_ban(group_id, user_id, duration_secs).await
actions.set_group_kick(group_id, user_id, reject_add).await
actions.set_group_whole_ban(group_id, enable).await
actions.set_group_admin(group_id, user_id, enable).await
actions.set_group_card(group_id, user_id, card).await
actions.set_group_name(group_id, name).await
actions.set_group_leave(group_id, is_dismiss).await

// 查询
actions.get_login_info().await                         // -> LoginInfoResponse { user_id, nickname }
actions.get_group_info(group_id, no_cache).await       // -> GroupInfoResponse { group_id, group_name, member_count, max_member_count }
actions.get_group_member_info(group_id, user_id, no_cache).await
actions.get_group_member_list(group_id).await
actions.get_group_list().await
actions.get_friend_list().await
actions.get_stranger_info(user_id, no_cache).await

// 请求处理
actions.set_friend_add_request(flag, approve, remark).await
actions.set_group_add_request(flag, sub_type, approve, reason).await

// 扩展
actions.send_group_poke(group_id, user_id).await
actions.send_friend_poke(user_id).await
actions.set_essence_msg(message_id).await
actions.upload_group_file(group_id, file, name, folder).await
```

## 通知路由一览

| Notice Route | 说明 |
|---|---|
| GroupPoke / PrivatePoke | 戳一戳 |
| GroupIncreaseApprove / GroupIncreaseInvite | 入群 |
| GroupDecreaseLeave / GroupDecreaseKick / GroupDecreaseKickMe | 退群/踢人 |
| GroupRecall / FriendRecall | 消息撤回 |
| GroupBanBan / GroupBanLiftBan | 禁言/解禁 |
| FriendAdd | 好友添加 |
| GroupUpload | 群文件上传 |
| GroupAdminSet / GroupAdminUnset | 管理员变更 |
| GroupCard | 群名片变更 |
| EssenceAdd / EssenceDelete | 精华消息 |
| NotifyLuckyKing / NotifyHonor | 运气王/荣誉 |
| OfflineFile | 离线文件 |
| GroupReaction / MessageEmojiLike | 表情回应 |

| Request Route | 说明 |
|---|---|
| Friend | 好友申请 |
| GroupAdd | 加群申请 |
| GroupInvite | 群邀请 |

| Meta Route | 说明 |
|---|---|
| LifecycleEnable / Disable / Connect | 生命周期 |
| Heartbeat | 心跳 |

## 拦截器

```rust
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, bot_id: &str, event: &NormalizedEvent) -> bool {
        true  // true=放行, false=拦截
    }
    async fn after_completion(&self, bot_id: &str, event: &NormalizedEvent) {
        // 逆序执行，可选实现
    }
}
```

在 module 中注册：`#[module(id = "...", interceptors = [MyInterceptor])]`

## 执行规则

1. 先阅读 `plugins/qimen-plugin-example/src/` 中的示例代码作为参考
2. 新插件目录结构：`plugins/<name>/Cargo.toml` + `plugins/<name>/src/lib.rs`
3. 在根 `Cargo.toml` 的 `[workspace] members` 中注册新插件
4. 使用 `use qimen_plugin_api::prelude::*;` 导入所有必要类型
5. 编写完成后运行 `cargo check` 验证编译
6. 代码使用中英文双语注释
