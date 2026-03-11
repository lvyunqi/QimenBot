# 事件处理

除了命令之外，QimenBot 还可以处理 OneBot 协议的**系统事件**：通知（notice）、请求（request）、元事件（meta）。

通过 `#[notice]`、`#[request]`、`#[meta]` 三个宏，你可以像写命令一样轻松地处理各种系统事件。

## 事件处理器宏

### `#[notice]` — 通知事件

通知事件包括戳一戳、入群/退群、撤回消息、禁言等。括号中的参数是**路由名**，一个处理器可以监听多个路由：

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self) -> &str {
    "被戳了！"
}
```

### `#[request]` — 请求事件

请求事件包括好友申请、加群申请、群邀请等：

```rust
#[request(Friend)]
async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    SystemPluginSignal::ApproveFriend {
        flag,
        remark: Some("自动同意".to_string()),
    }
}
```

### `#[meta]` — 元事件

元事件包括心跳、生命周期事件等：

```rust
#[meta(Heartbeat)]
async fn on_heartbeat(&self) -> SystemPluginSignal {
    SystemPluginSignal::Continue
}
```

## 方法签名

事件处理器支持以下四种方法签名，按需选用：

```rust
// 1. 最简形式 —— 不需要任何上下文
async fn name(&self) -> impl IntoSystemSignal

// 2. 需要访问事件 JSON 和运行时
async fn name(&self, ctx: &SystemPluginContext<'_>) -> impl IntoSystemSignal

// 3. 需要判断具体触发的路由
async fn name(&self, route: &SystemNoticeRoute) -> impl IntoSystemSignal

// 4. 同时需要上下文和路由
async fn name(&self, ctx: &SystemPluginContext<'_>, route: &SystemNoticeRoute) -> impl IntoSystemSignal
```

::: tip 什么时候需要 route 参数？
当你用一个处理器监听多个路由时（如 `#[notice(GroupPoke, PrivatePoke)]`），可以通过 `route` 参数判断当前触发的是哪个路由，从而做出不同处理。
:::

---

## SystemPluginContext

`SystemPluginContext` 是事件处理器的上下文，提供以下字段和方法：

| 字段/方法 | 类型 | 说明 |
|-----------|------|------|
| `ctx.bot_id` | `&str` | 当前 Bot 的 QQ 号 |
| `ctx.event` | `&serde_json::Value` | 原始事件 JSON，包含所有事件字段 |
| `ctx.runtime` | `&dyn RuntimeBotContext` | 运行时上下文 |
| `ctx.onebot_actions()` | `OneBotActionClient` | 获取 OneBot API 客户端，可调用发消息等 API |

### 读取事件字段

`ctx.event` 是一个 `serde_json::Value`，通过下标或 `.get()` 方法读取字段：

```rust
// 下标方式（字段不存在时返回 Value::Null）
let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

// .get() 方式（字段不存在时返回 None，更安全）
let flag = ctx.event.get("flag")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
```

### 调用 OneBot API

通过 `ctx.onebot_actions()` 获取 API 客户端，可以发送消息、处理请求等：

```rust
let client = ctx.onebot_actions();
let _ = client.send_group_msg(group_id, message).await;
```

---

## 通知事件详解（Notice）

### GroupPoke / PrivatePoke — 戳一戳

当有人在群聊或私聊中使用"戳一戳"时触发。

**路由名：** `GroupPoke`（群聊戳一戳）、`PrivatePoke`（私聊戳一戳）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号（仅 `GroupPoke` 有） |
| `user_id` | `i64` | 发起戳一戳的用户 QQ |
| `target_id` | `i64` | 被戳的用户 QQ |
| `self_id` | `i64` | Bot 自身 QQ 号 |

::: tip 判断是否戳了 Bot 自己
比较 `target_id == self_id` 即可判断是否是戳了 Bot 自己。`NormalizedEvent` 也提供了 `is_poke_self()` 便捷方法。
:::

**代码示例：**

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let target = ctx.event["target_id"].as_i64().unwrap_or(0);
    let self_id = ctx.event["self_id"].as_i64().unwrap_or(0);

    if target == self_id {
        SystemPluginSignal::Reply(Message::text("别戳我啦！"))
    } else {
        SystemPluginSignal::Continue
    }
}
```

---

### GroupIncreaseApprove / GroupIncreaseInvite — 新成员入群

当新成员通过审批或被邀请进入群聊时触发。

**路由名：** `GroupIncreaseApprove`（管理员审批通过）、`GroupIncreaseInvite`（被邀请入群）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 新入群成员的 QQ |
| `operator_id` | `i64` | 审批人/邀请人的 QQ |
| `sub_type` | `string` | `"approve"` 审批入群 / `"invite"` 邀请入群 |

**代码示例：**

```rust
#[notice(GroupIncreaseApprove, GroupIncreaseInvite)]
async fn on_member_join(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    let welcome = Message::builder()
        .text("欢迎 ")
        .at(user_id.to_string())
        .text(" 加入本群！请阅读群公告。")
        .build();

    let client = ctx.onebot_actions();
    let _ = client.send_group_msg(group_id, welcome).await;

    SystemPluginSignal::Continue
}
```

---

### GroupDecreaseLeave / GroupDecreaseKick / GroupDecreaseKickMe — 成员退群

当群成员退出群聊时触发。

**路由名：** `GroupDecreaseLeave`（主动退群）、`GroupDecreaseKick`（被踢出群）、`GroupDecreaseKickMe`（Bot 被踢出群）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 退出/被踢的成员 QQ |
| `operator_id` | `i64` | 执行踢人的管理员 QQ（主动退群时与 `user_id` 相同） |
| `sub_type` | `string` | `"leave"` 主动退群 / `"kick"` 被踢 / `"kick_me"` Bot 被踢 |

::: warning 注意 GroupDecreaseKickMe
当 Bot 被踢出群时会触发 `GroupDecreaseKickMe`。建议在此事件中记录日志或通知管理员，因为 Bot 此时已无法向该群发送消息。
:::

**代码示例：**

```rust
#[notice(GroupDecreaseLeave, GroupDecreaseKick)]
async fn on_member_leave(
    &self,
    ctx: &SystemPluginContext<'_>,
    route: &SystemNoticeRoute,
) -> SystemPluginSignal {
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    let text = match route {
        SystemNoticeRoute::GroupDecreaseLeave => {
            format!("成员 {user_id} 退出了群聊")
        }
        SystemNoticeRoute::GroupDecreaseKick => {
            let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);
            format!("成员 {user_id} 被管理员 {operator} 踢出了群聊")
        }
        _ => return SystemPluginSignal::Continue,
    };

    tracing::info!("[群 {group_id}] {text}");
    SystemPluginSignal::Continue
}
```

---

### GroupRecall — 群消息撤回

当群聊中有消息被撤回时触发。

**路由名：** `GroupRecall`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 被撤回消息的发送者 QQ |
| `operator_id` | `i64` | 执行撤回操作的用户 QQ（管理员可撤回他人消息） |
| `message_id` | `i64` | 被撤回的消息 ID |

**代码示例：**

```rust
#[notice(GroupRecall)]
async fn on_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);
    let user = ctx.event["user_id"].as_i64().unwrap_or(0);
    let msg_id = ctx.event["message_id"].as_i64().unwrap_or(0);

    if operator == user {
        tracing::info!("用户 {user} 撤回了消息 {msg_id}");
    } else {
        tracing::info!("管理员 {operator} 撤回了用户 {user} 的消息 {msg_id}");
    }

    SystemPluginSignal::Continue
}
```

---

### FriendRecall — 好友消息撤回

当好友撤回了发给你的消息时触发。

**路由名：** `FriendRecall`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 撤回消息的好友 QQ |
| `message_id` | `i64` | 被撤回的消息 ID |

**代码示例：**

```rust
#[notice(FriendRecall)]
async fn on_friend_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let msg_id = ctx.event["message_id"].as_i64().unwrap_or(0);

    tracing::info!("好友 {user_id} 撤回了消息 {msg_id}");
    SystemPluginSignal::Continue
}
```

---

### GroupBanBan / GroupBanLiftBan — 群禁言

当群成员被禁言或解除禁言时触发。

**路由名：** `GroupBanBan`（禁言）、`GroupBanLiftBan`（解除禁言）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 被禁言/解除禁言的成员 QQ |
| `operator_id` | `i64` | 执行操作的管理员 QQ |
| `duration` | `i64` | 禁言时长（秒）。`0` 表示解除禁言 |

**代码示例：**

```rust
#[notice(GroupBanBan, GroupBanLiftBan)]
async fn on_ban(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let duration = ctx.event["duration"].as_i64().unwrap_or(0);
    let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);

    if duration > 0 {
        let minutes = duration / 60;
        tracing::info!("[群 {group_id}] 管理员 {operator} 禁言了 {user_id}，时长 {minutes} 分钟");
    } else {
        tracing::info!("[群 {group_id}] 管理员 {operator} 解除了 {user_id} 的禁言");
    }

    SystemPluginSignal::Continue
}
```

---

### FriendAdd — 新好友添加

当有新好友添加成功时触发（注意：这是好友已添加完成的通知，不是好友申请）。

**路由名：** `FriendAdd`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 新好友的 QQ |

**代码示例：**

```rust
#[notice(FriendAdd)]
async fn on_friend_add(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    let welcome = Message::text("你好！很高兴认识你～");
    let client = ctx.onebot_actions();
    let _ = client.send_private_msg(user_id, welcome).await;

    SystemPluginSignal::Continue
}
```

---

### GroupUpload — 群文件上传

当有用户向群内上传文件时触发。

**路由名：** `GroupUpload`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 上传者 QQ |
| `file` | `object` | 文件信息对象 |
| `file.id` | `string` | 文件 ID |
| `file.name` | `string` | 文件名 |
| `file.size` | `i64` | 文件大小（字节） |
| `file.busid` | `i64` | 文件 busid |

**代码示例：**

```rust
#[notice(GroupUpload)]
async fn on_file_upload(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let file_name = ctx.event["file"]["name"].as_str().unwrap_or("未知文件");
    let file_size = ctx.event["file"]["size"].as_i64().unwrap_or(0);

    let size_mb = file_size as f64 / 1024.0 / 1024.0;
    tracing::info!("用户 {user_id} 上传了文件: {file_name} ({size_mb:.2} MB)");

    SystemPluginSignal::Continue
}
```

---

### GroupAdminSet / GroupAdminUnset — 管理员变更

当群成员被设为管理员或被取消管理员时触发。

**路由名：** `GroupAdminSet`（设为管理员）、`GroupAdminUnset`（取消管理员）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 被设置/取消管理员的成员 QQ |

**代码示例：**

```rust
#[notice(GroupAdminSet, GroupAdminUnset)]
async fn on_admin_change(
    &self,
    ctx: &SystemPluginContext<'_>,
    route: &SystemNoticeRoute,
) -> SystemPluginSignal {
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    match route {
        SystemNoticeRoute::GroupAdminSet => {
            tracing::info!("[群 {group_id}] {user_id} 被设为管理员");
        }
        SystemNoticeRoute::GroupAdminUnset => {
            tracing::info!("[群 {group_id}] {user_id} 被取消管理员");
        }
        _ => {}
    }

    SystemPluginSignal::Continue
}
```

---

### GroupCard — 群名片变更

当群成员的群名片发生变更时触发。

**路由名：** `GroupCard`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 修改名片的成员 QQ |
| `card_new` | `string` | 新名片 |
| `card_old` | `string` | 旧名片 |

**代码示例：**

```rust
#[notice(GroupCard)]
async fn on_card_change(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let old = ctx.event["card_old"].as_str().unwrap_or("");
    let new = ctx.event["card_new"].as_str().unwrap_or("");

    tracing::info!("用户 {user_id} 修改了群名片: \"{old}\" -> \"{new}\"");
    SystemPluginSignal::Continue
}
```

---

### EssenceAdd / EssenceDelete — 精华消息

当消息被设为精华或精华消息被移除时触发。

**路由名：** `EssenceAdd`（设为精华）、`EssenceDelete`（移除精华）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `message_id` | `i64` | 消息 ID |
| `sender_id` | `i64` | 消息发送者 QQ |
| `operator_id` | `i64` | 执行操作的管理员 QQ |

**代码示例：**

```rust
#[notice(EssenceAdd)]
async fn on_essence_add(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let sender = ctx.event["sender_id"].as_i64().unwrap_or(0);
    let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);

    let msg = Message::builder()
        .at(sender.to_string())
        .text(" 的消息被管理员设为精华啦！")
        .build();

    SystemPluginSignal::Reply(msg)
}
```

---

### NotifyLuckyKing — 运气王

当群红包产生运气王时触发。

**路由名：** `NotifyLuckyKing`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 红包发送者 QQ |
| `target_id` | `i64` | 运气王的 QQ |

**代码示例：**

```rust
#[notice(NotifyLuckyKing)]
async fn on_lucky_king(&self, ctx: &SystemPluginContext<'_>) -> Message {
    let target = ctx.event["target_id"].as_i64().unwrap_or(0);

    Message::builder()
        .at(target.to_string())
        .text(" 是本次红包的运气王！")
        .build()
}
```

---

### NotifyHonor — 群荣誉变更

当群成员获得群荣誉时触发。

**路由名：** `NotifyHonor`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 获得荣誉的成员 QQ |
| `honor_type` | `string` | 荣誉类型：`"talkative"` 龙王、`"performer"` 群聊之火、`"emotion"` 快乐源泉 |

**代码示例：**

```rust
#[notice(NotifyHonor)]
async fn on_honor(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let honor = ctx.event["honor_type"].as_str().unwrap_or("");

    let title = match honor {
        "talkative" => "龙王",
        "performer" => "群聊之火",
        "emotion" => "快乐源泉",
        _ => "未知荣誉",
    };

    let msg = Message::builder()
        .at(user_id.to_string())
        .text(&format!(" 获得了「{title}」称号！"))
        .build();

    SystemPluginSignal::Reply(msg)
}
```

---

### OfflineFile — 离线文件

当收到好友发送的离线文件时触发。

**路由名：** `OfflineFile`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 发送者 QQ |
| `file` | `object` | 文件信息对象 |
| `file.name` | `string` | 文件名 |
| `file.size` | `i64` | 文件大小（字节） |
| `file.url` | `string` | 文件下载链接 |

**代码示例：**

```rust
#[notice(OfflineFile)]
async fn on_offline_file(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let name = ctx.event["file"]["name"].as_str().unwrap_or("未知文件");

    tracing::info!("收到好友 {user_id} 的离线文件: {name}");
    SystemPluginSignal::Continue
}
```

---

### GroupReaction / MessageEmojiLike — 消息表情回应

当有用户对群消息进行表情回应时触发。

**路由名：** `GroupReaction`、`MessageEmojiLike`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `message_id` | `i64` | 被回应的消息 ID |
| `user_id` | `i64` | 发起回应的用户 QQ |
| `code` / `emoji_id` | `string` | 表情 ID |

**代码示例：**

```rust
#[notice(GroupReaction, MessageEmojiLike)]
async fn on_reaction(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let msg_id = ctx.event["message_id"].as_i64().unwrap_or(0);

    tracing::info!("用户 {user_id} 对消息 {msg_id} 添加了表情回应");
    SystemPluginSignal::Continue
}
```

---

## 请求事件详解（Request）

请求事件需要返回特定的 `SystemPluginSignal` 来告诉框架如何处理该请求。

::: warning 重要
请求事件必须通过 `ctx.event["flag"]` 获取 `flag` 标识，并在返回信号中传递该 `flag`，框架才能正确处理请求。
:::

### Friend — 好友申请

当收到好友添加请求时触发。

**路由名：** `Friend`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 申请人 QQ |
| `comment` | `string` | 验证消息（申请人填写的附加信息） |
| `flag` | `string` | 请求标识，处理时需要传回 |

**返回信号：**

| 信号 | 说明 |
|------|------|
| `ApproveFriend { flag, remark }` | 同意好友请求，`remark` 为可选备注名 |
| `RejectFriend { flag, reason }` | 拒绝好友请求，`reason` 为可选拒绝理由 |

**代码示例：**

```rust
#[request(Friend)]
async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    let comment = ctx.event["comment"].as_str().unwrap_or_default();
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    tracing::info!("收到好友请求: user={user_id}, comment={comment}");

    // 如果验证消息包含暗号，自动同意
    if comment.contains("芝麻开门") {
        SystemPluginSignal::ApproveFriend {
            flag,
            remark: Some("通过暗号验证".to_string()),
        }
    } else {
        SystemPluginSignal::RejectFriend {
            flag,
            reason: Some("请输入正确的暗号".to_string()),
        }
    }
}
```

---

### GroupAdd — 加群申请

当有用户申请加入群聊时触发。

**路由名：** `GroupAdd`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 申请人 QQ |
| `comment` | `string` | 验证消息 |
| `flag` | `string` | 请求标识，处理时需要传回 |
| `sub_type` | `string` | 固定为 `"add"` |

**返回信号：**

| 信号 | 说明 |
|------|------|
| `ApproveGroupInvite { flag, sub_type }` | 同意加群请求 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝加群请求 |

**代码示例：**

```rust
#[request(GroupAdd)]
async fn on_group_add(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    let sub_type = ctx.event["sub_type"].as_str().unwrap_or("add").to_string();
    let comment = ctx.event["comment"].as_str().unwrap_or_default();
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);

    tracing::info!("[群 {group_id}] 收到加群申请: user={user_id}, comment={comment}");

    // 自动同意所有加群申请
    SystemPluginSignal::ApproveGroupInvite { flag, sub_type }
}
```

---

### GroupInvite — 群邀请

当 Bot 被邀请加入某个群时触发。

**路由名：** `GroupInvite`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 被邀请进入的群号 |
| `user_id` | `i64` | 邀请人 QQ |
| `flag` | `string` | 请求标识，处理时需要传回 |
| `sub_type` | `string` | 固定为 `"invite"` |

**返回信号：**

| 信号 | 说明 |
|------|------|
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 |

**代码示例：**

```rust
#[request(GroupInvite)]
async fn on_group_invite(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    let sub_type = ctx.event["sub_type"].as_str().unwrap_or("invite").to_string();
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    tracing::info!("收到群邀请: 被 {user_id} 邀请加入群 {group_id}");

    SystemPluginSignal::ApproveGroupInvite { flag, sub_type }
}
```

---

## 元事件详解（Meta）

元事件用于监控 Bot 与 OneBot 实现的连接状态。

### Heartbeat — 心跳

OneBot 实现定期发送的心跳包，通常每 30 秒一次。

**路由名：** `Heartbeat`

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `status` | `object` | Bot 在线状态信息 |
| `interval` | `i64` | 心跳间隔（毫秒） |

**代码示例：**

```rust
#[meta(Heartbeat)]
async fn on_heartbeat(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let interval = ctx.event["interval"].as_i64().unwrap_or(0);
    tracing::trace!("心跳: 间隔 {interval}ms");
    SystemPluginSignal::Continue
}
```

---

### LifecycleConnect / LifecycleEnable / LifecycleDisable — 生命周期

OneBot 实现的生命周期事件。

**路由名：** `LifecycleConnect`（连接建立）、`LifecycleEnable`（启用）、`LifecycleDisable`（禁用）

**`ctx.event` 可用字段：**

| 字段 | 类型 | 说明 |
|------|------|------|
| `sub_type` | `string` | `"connect"` / `"enable"` / `"disable"` |

**代码示例：**

```rust
#[meta(LifecycleConnect)]
async fn on_connect(&self) -> SystemPluginSignal {
    tracing::info!("Bot 已连接到 OneBot 实现！");
    SystemPluginSignal::Continue
}
```

---

## SystemPluginSignal 信号一览

事件处理器通过返回 `SystemPluginSignal` 来告诉框架如何处理事件。

| 信号 | 说明 | 适用场景 |
|------|------|----------|
| `Continue` | 不做特殊处理，继续执行后续插件 | 仅记录日志、不需要回复的事件 |
| `Reply(Message)` | 回复消息并继续执行后续插件 | 自动回复、通知消息 |
| `ApproveFriend { flag, remark }` | 同意好友请求 | `#[request(Friend)]` |
| `RejectFriend { flag, reason }` | 拒绝好友请求 | `#[request(Friend)]` |
| `ApproveGroupInvite { flag, sub_type }` | 同意加群/群邀请 | `#[request(GroupAdd, GroupInvite)]` |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝加群/群邀请 | `#[request(GroupAdd, GroupInvite)]` |
| `Block(Message)` | 回复消息并**终止**后续插件执行 | 独占处理某事件 |
| `Ignore` | 不回复，直接**终止**后续插件执行 | 静默拦截 |

::: tip Continue vs Block vs Ignore
- `Continue` 和 `Reply` 会让后续插件继续处理同一事件。
- `Block` 和 `Ignore` 会阻止后续插件处理同一事件（插件链提前终止）。
- 如果你的插件是唯一处理某事件的插件，用哪个都可以；但在多插件协作时，选择正确的信号很重要。
:::

---

## 返回值自动转换

和命令处理器一样，事件处理器也实现了 `IntoSystemSignal` trait，支持多种返回值类型的自动转换：

| 返回类型 | 转换结果 |
|----------|----------|
| `&str` | `Reply(Message::text(...))` |
| `String` | `Reply(Message::text(...))` |
| `Message` | `Reply(message)` |
| `SystemPluginSignal` | 直接使用 |
| `Result<T, E>` | `Ok(v)` 走上述转换，`Err(e)` 转为错误消息回复 |

```rust
// 返回 &str —— 自动转为 Reply
#[notice(GroupPoke)]
async fn on_poke(&self) -> &str {
    "别戳我！"
}

// 返回 Message —— 自动转为 Reply
#[notice(GroupPoke)]
async fn on_poke(&self) -> Message {
    Message::builder().text("别戳我！").face(181).build()
}

// 返回 SystemPluginSignal —— 直接使用，可以实现更复杂的逻辑
#[notice(GroupPoke)]
async fn on_poke(&self) -> SystemPluginSignal {
    SystemPluginSignal::Continue // 不回复，静默处理
}
```

---

## 完整示例

下面是一个完整的事件处理插件示例，包含了通知、请求和元事件的处理：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "event-handler", version = "0.1.0",
         name = "事件处理插件",
         description = "处理各种系统事件")]
#[commands]
impl EventHandlerModule {
    // ── 通知事件 ──

    /// 戳一戳回复
    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let target = ctx.event["target_id"].as_i64().unwrap_or(0);
        let self_id = ctx.event["self_id"].as_i64().unwrap_or(0);

        if target == self_id {
            SystemPluginSignal::Reply(
                Message::builder().text("别戳我啦！").face(181).build()
            )
        } else {
            SystemPluginSignal::Continue
        }
    }

    /// 新成员欢迎
    #[notice(GroupIncreaseApprove, GroupIncreaseInvite)]
    async fn on_member_join(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
        let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

        let welcome = Message::builder()
            .text("欢迎 ")
            .at(user_id.to_string())
            .text(" 加入本群！请阅读群公告。")
            .build();

        let client = ctx.onebot_actions();
        let _ = client.send_group_msg(group_id, welcome).await;
        SystemPluginSignal::Continue
    }

    /// 撤回消息通知
    #[notice(GroupRecall)]
    async fn on_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);
        let user = ctx.event["user_id"].as_i64().unwrap_or(0);
        let msg_id = ctx.event["message_id"].as_i64().unwrap_or(0);

        if operator == user {
            tracing::info!("用户 {user} 撤回了消息 {msg_id}");
        } else {
            tracing::info!("管理员 {operator} 撤回了用户 {user} 的消息 {msg_id}");
        }

        SystemPluginSignal::Continue
    }

    /// 群禁言通知
    #[notice(GroupBanBan, GroupBanLiftBan)]
    async fn on_ban(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);
        let duration = ctx.event["duration"].as_i64().unwrap_or(0);

        if duration > 0 {
            tracing::info!("用户 {user_id} 被禁言 {} 分钟", duration / 60);
        } else {
            tracing::info!("用户 {user_id} 被解除禁言");
        }

        SystemPluginSignal::Continue
    }

    // ── 请求事件 ──

    /// 自动处理好友请求
    #[request(Friend)]
    async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
        let comment = ctx.event["comment"].as_str().unwrap_or_default();
        let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

        tracing::info!("收到好友请求: user={user_id}, comment={comment}");

        SystemPluginSignal::ApproveFriend {
            flag,
            remark: Some("自动同意".to_string()),
        }
    }

    /// 自动同意群邀请
    #[request(GroupInvite)]
    async fn on_group_invite(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
        let sub_type = ctx.event["sub_type"].as_str().unwrap_or("invite").to_string();

        SystemPluginSignal::ApproveGroupInvite { flag, sub_type }
    }

    // ── 元事件 ──

    /// 心跳日志
    #[meta(Heartbeat)]
    async fn on_heartbeat(&self) -> SystemPluginSignal {
        tracing::trace!("收到心跳包");
        SystemPluginSignal::Continue
    }

    /// 连接建立通知
    #[meta(LifecycleConnect)]
    async fn on_connect(&self) -> SystemPluginSignal {
        tracing::info!("Bot 已连接！");
        SystemPluginSignal::Continue
    }
}
```
