# 类型参考

本页面列出 QimenBot 框架中所有核心数据类型的字段定义与说明。

## 响应类型

以下类型为调用 OneBot API 后返回的响应结构体。

### LoginInfoResponse

调用 `get_login_info()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | Bot QQ号 |
| `nickname` | `String` | Bot 昵称 |

### SendMsgResponse

调用 `send_msg()` / `send_group_msg()` / `send_private_msg()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 发送的消息ID |

### GetMsgResponse

调用 `get_msg()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息ID |
| `real_id` | `Option<i64>` | 真实消息ID |
| `sender` | `Option<SenderInfo>` | 发送者信息，见 [SenderInfo](#senderinfo) |
| `time` | `Option<i64>` | 时间戳 |
| `message` | `Value` | 消息内容（JSON 格式） |
| `raw_message` | `Option<String>` | 原始 CQ 码 |

### GroupInfoResponse

调用 `get_group_info()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `group_name` | `String` | 群名 |
| `member_count` | `Option<i64>` | 成员数 |
| `max_member_count` | `Option<i64>` | 最大成员数 |

### GroupMemberInfoResponse

调用 `get_group_member_info()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `Option<i64>` | 群号 |
| `user_id` | `i64` | 用户 QQ号 |
| `nickname` | `Option<String>` | 昵称 |
| `card` | `Option<String>` | 群名片 |
| `sex` | `Option<String>` | 性别，`"male"` / `"female"` / `"unknown"` |
| `age` | `Option<i64>` | 年龄 |
| `area` | `Option<String>` | 地区 |
| `join_time` | `Option<i64>` | 入群时间戳 |
| `last_sent_time` | `Option<i64>` | 最后发言时间戳 |
| `level` | `Option<String>` | 等级 |
| `role` | `Option<String>` | 角色，`"owner"` / `"admin"` / `"member"` |
| `title` | `Option<String>` | 专属头衔 |

### FriendInfoResponse

调用 `get_friend_list()` 返回的列表元素类型。

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 用户 QQ号 |
| `nickname` | `String` | 昵称 |
| `remark` | `Option<String>` | 好友备注 |

### StrangerInfoResponse

调用 `get_stranger_info()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 用户 QQ号 |
| `nickname` | `String` | 昵称 |
| `sex` | `Option<String>` | 性别 |
| `age` | `Option<i64>` | 年龄 |

### CanSendResponse

调用 `can_send_image()` / `can_send_record()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `yes` | `bool` | 是否支持 |

### GetForwardMsgResponse

调用 `get_forward_msg()` 返回。

| 字段 | 类型 | 说明 |
|------|------|------|
| `message` | `Value` | 合并转发消息内容（JSON 格式） |

### SenderInfo

消息发送者的详细信息，内嵌于 `GetMsgResponse` 中。

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `Option<i64>` | 用户 QQ号 |
| `nickname` | `Option<String>` | 昵称 |
| `card` | `Option<String>` | 群名片 |
| `sex` | `Option<String>` | 性别 |
| `age` | `Option<i64>` | 年龄 |
| `area` | `Option<String>` | 地区 |
| `level` | `Option<String>` | 等级 |
| `role` | `Option<String>` | 角色 |
| `title` | `Option<String>` | 专属头衔 |

---

## 核心框架类型

### NormalizedEvent

统一事件对象，是所有事件处理器中最常使用的类型。通过 `ctx.event()` 获取。

::: tip
`NormalizedEvent` 提供了大量便捷方法来访问事件中的字段，无需手动解析 JSON。
:::

#### 身份信息方法

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `sender_id()` | `Option<String>` | 发送者 ID（字符串） |
| `sender_id_i64()` | `Option<i64>` | 发送者 ID（整数） |
| `sender_nickname()` | `Option<String>` | 发送者昵称 |
| `sender_role()` | `Option<String>` | 发送者角色 |
| `sender_card()` | `Option<String>` | 发送者群名片 |
| `sender_sex()` | `Option<String>` | 发送者性别 |
| `sender_age()` | `Option<i64>` | 发送者年龄 |
| `sender_level()` | `Option<String>` | 发送者等级 |
| `sender_title()` | `Option<String>` | 发送者专属头衔 |

#### 聊天信息方法

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `chat_id()` | `Option<String>` | 当前聊天 ID（群号或用户 ID） |
| `group_id()` | `Option<String>` | 群号（字符串） |
| `group_id_i64()` | `Option<i64>` | 群号（整数） |
| `is_group()` | `bool` | 是否为群聊消息 |
| `is_private()` | `bool` | 是否为私聊消息 |

#### 原始 JSON 字段方法

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `user_id()` | `Option<String>` | 用户 ID |
| `message_id()` | `Option<String>` | 消息 ID |
| `self_id()` | `Option<i64>` | Bot 自身 ID（整数） |
| `self_id_str()` | `Option<String>` | Bot 自身 ID（字符串） |
| `sub_type()` | `Option<String>` | 事件子类型 |
| `message_type()` | `Option<String>` | 消息类型 |
| `post_type()` | `Option<String>` | 上报类型 |
| `notice_type()` | `Option<String>` | 通知类型 |
| `request_type()` | `Option<String>` | 请求类型 |
| `operator_id()` | `Option<String>` | 操作者 ID |
| `target_id()` | `Option<String>` | 目标 ID |
| `comment()` | `Option<String>` | 附加评论信息 |
| `flag()` | `Option<String>` | 请求标识 |
| `duration()` | `Option<i64>` | 时长（秒） |

#### 便捷方法

| 方法 | 返回类型 | 说明 |
|------|----------|------|
| `plain_text()` | `String` | 提取消息纯文本内容 |
| `is_at_self()` | `bool` | 消息是否 @ 了 Bot |
| `is_group_admin_or_owner()` | `bool` | 发送者是否为群管理员或群主 |
| `is_poke_self()` | `bool` | 是否为戳一戳 Bot 的事件 |

---

### CommandPluginSignal

命令处理器的返回信号，决定命令执行后的行为。

| 变体 | 说明 |
|------|------|
| `Reply(Message)` | 回复一条消息 |
| `Continue` | 不做任何操作，继续后续流程 |
| `Block(Message)` | 回复消息并阻止后续处理器 |
| `Ignore` | 忽略本次事件 |

### SystemPluginSignal

系统事件处理器的返回信号。

| 变体 | 说明 |
|------|------|
| `Continue` | 继续后续流程 |
| `Reply(Message)` | 回复一条消息 |
| `ApproveFriend { flag, remark }` | 同意好友请求，`flag`: 请求标识，`remark`: 备注名 |
| `RejectFriend { flag, reason }` | 拒绝好友请求，`flag`: 请求标识，`reason`: 拒绝理由 |
| `ApproveGroupInvite { flag, sub_type }` | 同意群邀请/加群请求 |
| `RejectGroupInvite { flag, sub_type, reason }` | 拒绝群邀请/加群请求 |
| `Block(Message)` | 回复消息并阻止后续处理器 |
| `Ignore` | 忽略本次事件 |

---

### CommandRole

命令的权限等级。

| 变体 | 说明 |
|------|------|
| `Anyone` | 任何人可执行 |
| `Admin` | 仅群管理员及以上可执行 |
| `Owner` | 仅群主可执行 |

### CommandDefinition

命令定义结构体，用于注册命令时描述命令的元信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| `name` | `String` | 命令名称 |
| `description` | `String` | 命令描述 |
| `aliases` | `Vec<String>` | 命令别名列表 |
| `examples` | `Vec<String>` | 使用示例 |
| `category` | `String` | 命令分类 |
| `hidden` | `bool` | 是否隐藏（不显示在帮助中） |
| `required_role` | `CommandRole` | 所需权限等级 |
| `scope` | `CommandScope` | 作用域（默认 `All`） |
| `filter` | `MessageFilter` | 消息过滤器 |

### CommandScope

命令的作用域，声明命令在群聊/私聊中的可用性。

| 变体 | 说明 |
|------|------|
| `All` (默认) | 群聊和私聊均可触发 |
| `Group` | 仅在群聊中触发 |
| `Private` | 仅在私聊中触发 |

### MessageFilter

消息过滤器，用于精确匹配符合条件的消息。

| 字段 | 类型 | 说明 |
|------|------|------|
| `cmd` | `Option<String>` | 精确匹配命令名 |
| `starts_with` | `Option<String>` | 消息以指定文本开头 |
| `ends_with` | `Option<String>` | 消息以指定文本结尾 |
| `contains` | `Option<String>` | 消息包含指定文本 |
| `groups` | `Option<Vec<i64>>` | 限定群号列表 |
| `senders` | `Option<Vec<i64>>` | 限定发送者列表 |
| `at_mode` | `AtMode` | At 模式：`Need` / `NotNeed` / `Both` |
| `reply_filter` | `Option<ReplyFilter>` | 回复消息过滤 |
| `media_types` | `Option<Vec<String>>` | 媒体类型过滤 |
| `invert` | `bool` | 是否反转过滤结果 |

---

### PluginMetadata

插件元数据，描述插件的基本信息。

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | 插件唯一标识 |
| `name` | `String` | 插件名称 |
| `version` | `String` | 插件版本 |
| `description` | `String` | 插件描述 |
| `api_version` | `String` | 兼容的框架 API 版本 |
| `compatibility` | `String` | 兼容性说明 |

---

## 键盘类型

用于构建 QQ 机器人消息中的按钮键盘。

### Button

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | `String` | 按钮唯一标识 |
| `label` | `String` | 按钮文本 |
| `visited_label` | `String` | 点击后显示的文本 |
| `action_type` | `u8` | 动作类型，见 [ButtonAction](#buttonaction) |
| `action_data` | `String` | 动作数据（URL / 回调值 / 命令文本） |
| `style` | `u8` | 按钮样式，见 [ButtonStyle](#buttonstyle) |
| `permission_type` | `u8` | 权限类型，见 [ButtonPermission](#buttonpermission) |

### ButtonAction

按钮动作类型枚举。

| 值 | 名称 | 说明 |
|----|------|------|
| `0` | Jump | 跳转 URL |
| `1` | Callback | 回调 |
| `2` | Command | 发送命令 |

### ButtonStyle

按钮样式枚举。

| 值 | 名称 | 说明 |
|----|------|------|
| `0` | Grey | 灰色按钮 |
| `1` | Blue | 蓝色按钮 |

### ButtonPermission

按钮权限类型枚举。

| 值 | 名称 | 说明 |
|----|------|------|
| `0` | SpecifiedUsers | 指定用户可操作 |
| `1` | Manager | 仅管理员可操作 |
| `2` | All | 所有人可操作 |
| `3` | SpecifiedRoles | 指定角色可操作 |

---

## 协议类型

框架多协议支持相关的底层类型。

### ProtocolId

协议标识枚举。

| 变体 | 说明 |
|------|------|
| `OneBot11` | OneBot 11 协议 |
| `OneBot12` | OneBot 12 协议 |
| `Satori` | Satori 协议 |
| `Custom(String)` | 自定义协议 |

### TransportMode

传输模式枚举。

| 变体 | 说明 |
|------|------|
| `WsForward` | 正向 WebSocket |
| `WsReverse` | 反向 WebSocket |
| `HttpApi` | HTTP API |
| `HttpPost` | HTTP POST 上报 |
| `Webhook` | Webhook |
| `Custom(String)` | 自定义传输模式 |

### EventKind

事件类型枚举。

| 变体 | 说明 |
|------|------|
| `Message` | 消息事件 |
| `MessageSent` | Bot 发送消息事件 |
| `Notice` | 通知事件 |
| `Request` | 请求事件 |
| `Meta` | 元事件 |
| `Internal(String)` | 框架内部事件 |

### ActionStatus

API 调用状态枚举。

| 变体 | 说明 |
|------|------|
| `Ok` | 调用成功 |
| `Async` | 异步处理中 |
| `Failed` | 调用失败 |
