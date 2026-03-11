# 事件处理流程

本页详细解释 QimenBot 从接收到一条消息到最终回复的完整处理链路。理解事件流对于调试和开发高质量插件至关重要。

## 事件类型

QimenBot 处理四种 OneBot 事件类型：

| 事件类型 | `post_type` | 说明 | 处理方式 |
|---------|-------------|------|---------|
| **消息事件** | `message` | 用户发送的聊天消息 | 命令匹配 → `CommandPlugin` |
| **通知事件** | `notice` | 群成员变动、撤回、戳一戳等 | 路由 → `SystemPlugin.on_notice()` |
| **请求事件** | `request` | 好友申请、群邀请等 | 路由 → `SystemPlugin.on_request()` |
| **元事件** | `meta_event` | 心跳、生命周期等 | 路由 → `SystemPlugin.on_meta()` |

## 消息事件处理链

消息事件经过多道"关卡"，每一步都可能终止处理：

### 第 1 步：消息去重

```rust
// 框架内部逻辑
if message_dedup.is_duplicate(event.message_id()) {
    return; // 丢弃重复消息
}
```

**作用：** 防止网络抖动导致的重复消息被处理两次。基于 `message_id` 去重，保留最近一段时间的 ID 记录。

### 第 2 步：群事件过滤

如果配置了群白名单或黑名单，不在允许范围内的群消息会被直接丢弃。

### 第 3 步：令牌桶限流

```toml
[bots.limiter]
enable = true
rate = 5.0       # 每秒 5 个令牌
capacity = 10    # 最多缓存 10 个令牌
```

**作用：** 防止某个 Bot 被刷屏攻击。如果令牌耗尽，消息会被丢弃。

### 第 4 步：拦截器链 `pre_handle`

所有注册的拦截器按优先级**顺序**执行 `pre_handle` 方法：

```rust
for interceptor in &interceptor_chain {
    if !interceptor.pre_handle(bot_id, &event).await {
        return; // 拦截器返回 false，终止处理
    }
}
```

**典型用途：**
- 日志记录
- 冷却时间控制
- 黑名单检查
- 关键词过滤

### 第 5 步：权限解析

根据 `owners` 和 `admins` 配置判断发送者的角色：

```
发送者 QQ 号在 owners 列表中 → Owner 角色
发送者 QQ 号在 admins 列表中 → Admin 角色
其他 → Anyone 角色
```

### 第 6 步：命令匹配

框架检测消息是否触发了命令：

```
消息文本: "/echo hello world"
            ↓ 匹配命令
命令名: "echo"
参数: ["hello", "world"]
```

命令触发方式：

| 触发方式 | 示例 | 场景 |
|---------|------|------|
| 斜杠前缀 | `/ping` | 群聊和私聊 |
| 直接输入 | `ping` | 仅私聊 |
| @提及 | `@Bot ping` | 群聊 |
| 回复消息 | (回复 Bot 消息) `ping` | 群聊和私聊 |

### 第 7 步：插件分发

匹配到的命令会被路由到对应的 `CommandPlugin`：

```rust
// 框架调用你的插件方法
let signal = plugin.on_command(&ctx, &invocation).await;
```

插件返回一个**信号（Signal）**，告诉框架如何处理：

| 信号 | 效果 |
|------|------|
| `Reply(message)` | 发送回复消息，继续处理下一个插件 |
| `Continue` | 不做任何操作，继续处理下一个插件 |
| `Block(message)` | 发送回复消息，**停止**后续所有插件 |
| `Ignore` | **停止**后续所有插件，不发消息 |

### 第 8 步：拦截器链 `after_completion`

所有拦截器按**逆序**执行 `after_completion` 方法：

```rust
for interceptor in interceptor_chain.iter().rev() {
    interceptor.after_completion(bot_id, &event).await;
}
```

**典型用途：**
- 统计处理时间
- 清理临时状态
- 日志记录

## 系统事件处理

系统事件（notice / request / meta）不经过命令匹配，而是根据**路由**直接分发到 `SystemPlugin`：

```
收到 notice 事件
  → 解析 notice_type + sub_type
  → 映射到 SystemNoticeRoute 枚举
  → 查找注册了该路由的 SystemPlugin
  → 调用 on_notice()
```

### 通知事件路由

| 路由名 | 说明 |
|--------|------|
| `GroupPoke` | 群聊戳一戳 |
| `PrivatePoke` | 私聊戳一戳 |
| `GroupIncreaseApprove` | 新成员通过审批入群 |
| `GroupIncreaseInvite` | 新成员被邀请入群 |
| `GroupDecreaseLeave` | 成员主动退群 |
| `GroupDecreaseKick` | 成员被踢出群 |
| `GroupRecall` | 消息被撤回 |
| `GroupBanBan` | 成员被禁言 |
| `GroupBanLiftBan` | 成员被解除禁言 |
| `FriendAdd` | 新好友已添加 |
| `GroupUpload` | 群文件上传 |
| ... | 更多路由见 [API 参考](/api/plugin-api#系统事件路由) |

### 请求事件路由

| 路由名 | 说明 |
|--------|------|
| `Friend` | 好友申请 |
| `GroupAdd` | 加群申请 |
| `GroupInvite` | 群邀请 |

### 元事件路由

| 路由名 | 说明 |
|--------|------|
| `Heartbeat` | 心跳包 |
| `LifecycleConnect` | 连接建立 |
| `LifecycleEnable` | 启用 |
| `LifecycleDisable` | 禁用 |

## 动态插件事件处理

动态插件的事件处理与静态插件类似，但有一些区别：

1. **同步执行** — 动态插件回调是同步的（`extern "C"` 函数）
2. **熔断器保护** — 连续 3 次异常后自动隔离 60 秒
3. **简化上下文** — 通过 `CommandRequest` / `NoticeRequest` 传递上下文

```
消息事件 → 命令匹配 → 动态插件?
    ↓ 是
检查熔断器状态
    ↓ 未隔离
调用 extern "C" callback(req) → CommandResponse
    ↓ 成功
重置失败计数
    ↓ 失败
递增失败计数 → 3次? → 隔离 60 秒
```
