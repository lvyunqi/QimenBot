# 事件处理

除了命令之外，QimenBot 还可以处理 OneBot 协议的**系统事件**：通知（notice）、请求（request）、元事件（meta）。

## 事件处理器宏

### `#[notice]` — 通知事件

通知事件包括戳一戳、入群/退群、撤回消息等：

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self) -> &str {
    "被戳了！"
}
```

括号中的参数是**路由名**，一个处理器可以监听多个路由。

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

## 完整路由列表

### 通知路由（Notice）

| 路由名 | 说明 | 典型用途 |
|--------|------|---------|
| `GroupPoke` | 群聊戳一戳 | 自动回复 |
| `PrivatePoke` | 私聊戳一戳 | 自动回复 |
| `GroupIncreaseApprove` | 新成员通过审批入群 | 欢迎消息 |
| `GroupIncreaseInvite` | 新成员被邀请入群 | 欢迎消息 |
| `GroupDecreaseLeave` | 成员主动退群 | 日志记录 |
| `GroupDecreaseKick` | 成员被踢出群 | 日志记录 |
| `GroupDecreaseKickMe` | Bot 被踢出群 | 告警通知 |
| `GroupRecall` | 群消息被撤回 | 反撤回 |
| `FriendRecall` | 好友消息被撤回 | 日志记录 |
| `GroupBanBan` | 成员被禁言 | 日志记录 |
| `GroupBanLiftBan` | 成员被解除禁言 | 日志记录 |
| `FriendAdd` | 新好友已添加 | 欢迎消息 |
| `GroupUpload` | 群文件上传 | 文件处理 |
| `GroupAdminSet` | 成员被设为管理员 | 通知 |
| `GroupAdminUnset` | 成员被取消管理员 | 通知 |
| `GroupCard` | 群名片变更 | 日志记录 |
| `EssenceAdd` | 消息被设为精华 | 通知 |
| `EssenceDelete` | 精华消息被移除 | 通知 |
| `NotifyLuckyKing` | 运气王 | 趣味回复 |
| `NotifyHonor` | 荣誉变更 | 通知 |
| `OfflineFile` | 离线文件 | 文件处理 |
| `GroupReaction` | 群消息表情回应 | 互动 |
| `MessageEmojiLike` | 消息表情点赞 | 互动 |

### 请求路由（Request）

| 路由名 | 说明 | 典型用途 |
|--------|------|---------|
| `Friend` | 好友申请 | 自动审批 |
| `GroupAdd` | 用户申请加群 | 自动审批 |
| `GroupInvite` | 被邀请加入某群 | 自动审批 |

### 元事件路由（Meta）

| 路由名 | 说明 | 典型用途 |
|--------|------|---------|
| `Heartbeat` | 心跳包（通常每 30 秒一次） | 健康检测 |
| `LifecycleConnect` | 连接建立 | 启动通知 |
| `LifecycleEnable` | OneBot 启用 | 日志记录 |
| `LifecycleDisable` | OneBot 禁用 | 日志记录 |

## SystemPluginContext

系统事件处理器可以接收 `SystemPluginContext` 来访问事件上下文：

```rust
#[notice(GroupRecall)]
async fn on_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    // ctx.bot_id  — 当前 Bot ID
    // ctx.event   — 原始事件 JSON（serde_json::Value）
    // ctx.runtime — 运行时上下文

    let operator = ctx.event["operator_id"].as_i64().unwrap_or(0);
    let user = ctx.event["user_id"].as_i64().unwrap_or(0);
    let msg_id = ctx.event["message_id"].as_i64().unwrap_or(0);

    tracing::info!(
        "消息被撤回: operator={operator}, user={user}, msg_id={msg_id}"
    );

    SystemPluginSignal::Continue
}
```

### 使用 OneBot API

通过 `ctx.onebot_actions()` 可以调用 OneBot API：

```rust
#[notice(GroupIncreaseApprove, GroupIncreaseInvite)]
async fn on_member_join(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    // 发送欢迎消息
    let welcome = Message::builder()
        .text("欢迎 ")
        .at(&user_id.to_string())
        .text(" 加入本群！")
        .face(1)
        .build();

    let client = ctx.onebot_actions();
    let _ = client.send_group_msg(group_id, welcome).await;

    SystemPluginSignal::Continue
}
```

## SystemPluginSignal

系统事件处理器返回 `SystemPluginSignal` 来告诉框架如何处理：

| 信号 | 说明 |
|------|------|
| `Continue` | 不做特殊处理，继续下一个插件 |
| `Reply(Message)` | 回复消息并继续 |
| `ApproveFriend { flag, remark }` | 同意好友请求 |
| `RejectFriend { flag, reason }` | 拒绝好友请求 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请 |
| `Block(Message)` | 回复消息并终止后续插件 |
| `Ignore` | 静默终止后续插件 |

### 好友请求处理

```rust
#[request(Friend)]
async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    let comment = ctx.event["comment"].as_str().unwrap_or_default();
    let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

    tracing::info!("收到好友请求: user={user_id}, comment={comment}");

    // 自动同意
    SystemPluginSignal::ApproveFriend {
        flag,
        remark: Some("通过插件自动同意".to_string()),
    }
}
```

### 群邀请处理

```rust
#[request(GroupInvite)]
async fn on_group_invite(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
    let sub_type = ctx.event["sub_type"].as_str().unwrap_or("invite").to_string();
    let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);

    tracing::info!("收到群邀请: group={group_id}");

    SystemPluginSignal::ApproveGroupInvite {
        flag,
        sub_type,
    }
}
```

## 返回值自动转换

和命令处理器一样，系统事件处理器也支持返回值自动转换：

```rust
// 返回字符串 → Reply(Message::text(...))
#[notice(GroupPoke)]
async fn on_poke(&self) -> &str {
    "别戳我！"
}

// 返回 Message → Reply(msg)
#[notice(GroupPoke)]
async fn on_poke(&self) -> Message {
    Message::builder().text("别戳我！").face(1).build()
}

// 返回 SystemPluginSignal → 直接使用
#[notice(GroupPoke)]
async fn on_poke(&self) -> SystemPluginSignal {
    SystemPluginSignal::Continue // 不回复
}
```

## 戳一戳自检测

`NormalizedEvent` 提供了 `is_poke_self()` 便捷方法，可以判断是否是戳了 Bot 自己：

```rust
#[notice(GroupPoke, PrivatePoke)]
async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
    // 需要将 Value 转为 NormalizedEvent 使用便捷方法
    // 或者手动判断 target_id == self_id
    let target = ctx.event["target_id"].as_i64().unwrap_or(0);
    let self_id = ctx.event["self_id"].as_i64().unwrap_or(0);

    if target == self_id {
        SystemPluginSignal::Reply(Message::text("别戳我啦！"))
    } else {
        SystemPluginSignal::Continue
    }
}
```

## 完整示例

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "example-events", version = "0.1.0")]
#[commands]
impl EventDemoModule {
    // ── 通知事件 ──

    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self) -> Message {
        Message::builder()
            .text("被戳了！")
            .face(181)
            .build()
    }

    #[notice(GroupIncreaseApprove, GroupIncreaseInvite)]
    async fn on_member_join(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let group_id = ctx.event["group_id"].as_i64().unwrap_or(0);
        let user_id = ctx.event["user_id"].as_i64().unwrap_or(0);

        let welcome = Message::builder()
            .text("欢迎 ")
            .at(&user_id.to_string())
            .text(" 加入本群！请阅读群公告。")
            .build();

        let client = ctx.onebot_actions();
        let _ = client.send_group_msg(group_id, welcome).await;
        SystemPluginSignal::Continue
    }

    #[notice(GroupRecall)]
    async fn on_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let user = ctx.event["user_id"].as_i64().unwrap_or(0);
        tracing::info!("用户 {user} 撤回了一条消息");
        SystemPluginSignal::Continue
    }

    // ── 请求事件 ──

    #[request(Friend)]
    async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
        SystemPluginSignal::ApproveFriend {
            flag,
            remark: Some("自动同意".to_string()),
        }
    }

    #[request(GroupInvite)]
    async fn on_group_invite(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event["flag"].as_str().unwrap_or_default().to_string();
        let sub_type = ctx.event["sub_type"].as_str().unwrap_or("invite").to_string();
        SystemPluginSignal::ApproveGroupInvite { flag, sub_type }
    }

    // ── 元事件 ──

    #[meta(Heartbeat)]
    async fn on_heartbeat(&self) -> SystemPluginSignal {
        tracing::debug!("收到心跳包");
        SystemPluginSignal::Continue
    }
}
```
