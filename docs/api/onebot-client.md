# OneBot API 客户端

`OneBotActionClient` 封装了 OneBot 11 协议的全部 API 操作，通过上下文对象获取：

```rust
let client = ctx.onebot_actions();
let info = client.get_login_info().await?;
```

## 获取方式

```rust
// 在命令处理器中
async fn my_cmd(&self, ctx: &CommandPluginContext<'_>) {
    let client = ctx.onebot_actions();
}

// 在系统事件处理器中
async fn on_notice(&self, ctx: &SystemPluginContext<'_>) {
    let client = ctx.onebot_actions();
}
```

::: tip 提示
所有方法均为异步方法，需要使用 `.await` 调用。返回 `Result` 类型时，可以使用 `?` 操作符进行错误传播。
:::

---

## 消息相关

### 发送私聊消息

> `client.send_private_msg(user_id, message)`

发送私聊消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 目标用户 QQ 号 |
| `message` | `Message` | 消息内容 |

**返回值** `SendMsgResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

```rust
let msg = Message::text("你好！");
let resp = client.send_private_msg(123456, msg).await?;
println!("消息已发送，ID: {}", resp.message_id);
```

---

### 发送群消息

> `client.send_group_msg(group_id, message)`

发送群消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `message` | `Message` | 消息内容 |

**返回值** `SendMsgResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

```rust
let msg = Message::builder()
    .text("Hello ")
    .at("123456")
    .text(" 欢迎加入！")
    .build();
let resp = client.send_group_msg(654321, msg).await?;
```

---

### 通用发送消息

> `client.send_msg(message_type, id, message)`

通用消息发送接口，根据 `message_type` 自动选择私聊或群聊。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_type` | `&str` | 消息类型：`"private"` 或 `"group"` |
| `id` | `i64` | 目标 ID（用户 QQ 号或群号） |
| `message` | `Message` | 消息内容 |

**返回值** `SendMsgResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

```rust
// 根据上下文动态选择发送目标
let msg_type = if ctx.is_group() { "group" } else { "private" };
let id = if ctx.is_group() { ctx.group_id_i64().unwrap() } else { ctx.sender_id_i64().unwrap() };
client.send_msg(msg_type, id, Message::text("通用发送")).await?;
```

---

### 获取消息

> `client.get_msg(message_id)`

获取消息详情。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

**返回值** `GetMsgResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |
| `real_id` | `Option<i64>` | 消息真实 ID |
| `sender` | `Option<SenderInfo>` | 发送者信息 |
| `time` | `Option<i64>` | 发送时间（时间戳） |
| `message` | `Value` | 消息内容（JSON 格式） |
| `raw_message` | `Option<String>` | 原始消息字符串 |

其中 `SenderInfo` 结构如下：

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `Option<i64>` | 发送者 QQ 号 |
| `nickname` | `Option<String>` | 昵称 |
| `card` | `Option<String>` | 群名片 |
| `sex` | `Option<String>` | 性别 |
| `age` | `Option<i64>` | 年龄 |
| `area` | `Option<String>` | 地区 |
| `level` | `Option<String>` | 等级 |
| `role` | `Option<String>` | 角色：`"owner"` / `"admin"` / `"member"` |
| `title` | `Option<String>` | 专属头衔 |

```rust
let msg_detail = client.get_msg(12345).await?;
if let Some(sender) = &msg_detail.sender {
    println!("发送者: {:?}", sender.nickname);
}
```

---

### 撤回消息

> `client.delete_msg(message_id)`

撤回消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 要撤回的消息 ID |

```rust
client.delete_msg(12345).await?;
```

---

### 获取合并转发消息

> `client.get_forward_msg(id)`

获取合并转发消息的内容。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `id` | `impl Into<String>` | 合并转发 ID |

**返回值** `GetForwardMsgResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `message` | `Value` | 合并转发消息内容（JSON 格式） |

---

### 发送群合并转发

> `client.send_group_forward_msg(group_id, messages)`

发送群合并转发消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `messages` | `Value` | 合并转发节点列表（JSON 格式） |

**返回值** `Value`

```rust
use serde_json::json;

let messages = json!([
    {
        "type": "node",
        "data": {
            "name": "Bot",
            "uin": "123456",
            "content": [{"type": "text", "data": {"text": "第一条消息"}}]
        }
    },
    {
        "type": "node",
        "data": {
            "name": "Bot",
            "uin": "123456",
            "content": [{"type": "text", "data": {"text": "第二条消息"}}]
        }
    }
]);
client.send_group_forward_msg(654321, messages).await?;
```

---

### 发送私聊合并转发

> `client.send_private_forward_msg(user_id, messages)`

发送私聊合并转发消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 目标用户 QQ 号 |
| `messages` | `Value` | 合并转发节点列表（JSON 格式） |

**返回值** `Value`

---

### 获取群历史消息

> `client.get_group_msg_history(group_id, message_seq, count)`

获取群历史消息记录。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `message_seq` | `Option<i64>` | 起始消息序号，`None` 表示从最新开始 |
| `count` | `i32` | 获取数量 |

**返回值** `Value`

```rust
// 获取群最近 20 条消息
let history = client.get_group_msg_history(654321, None, 20).await?;
```

---

### 获取好友历史消息

> `client.get_friend_msg_history(user_id, message_seq, count)`

获取好友历史消息记录。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 好友 QQ 号 |
| `message_seq` | `Option<i64>` | 起始消息序号，`None` 表示从最新开始 |
| `count` | `i32` | 获取数量 |

**返回值** `Value`

---

## 好友相关

### 获取好友列表

> `client.get_friend_list()`

获取好友列表。

**返回值** `Vec<FriendInfoResponse>`

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 好友 QQ 号 |
| `nickname` | `String` | 好友昵称 |
| `remark` | `Option<String>` | 好友备注 |

```rust
let friends = client.get_friend_list().await?;
for friend in friends {
    println!("{} ({})", friend.nickname, friend.user_id);
    if let Some(remark) = &friend.remark {
        println!("  备注: {}", remark);
    }
}
```

---

### 获取陌生人信息

> `client.get_stranger_info(user_id, no_cache)`

获取陌生人信息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 目标 QQ 号 |
| `no_cache` | `bool` | 是否不使用缓存 |

**返回值** `StrangerInfoResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | QQ 号 |
| `nickname` | `String` | 昵称 |
| `sex` | `Option<String>` | 性别：`"male"` / `"female"` / `"unknown"` |
| `age` | `Option<i64>` | 年龄 |

---

### 删除好友

> `client.delete_friend(user_id)`

删除好友。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 好友 QQ 号 |

---

### 删除单向好友

> `client.delete_unidirectional_friend(user_id)`

删除单向好友（对方已删除你但仍在你列表中的好友）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 好友 QQ 号 |

---

### 点赞

> `client.send_like(user_id, times)`

给好友点赞。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 目标 QQ 号 |
| `times` | `i32` | 点赞次数（每天最多 10 次） |

```rust
client.send_like(123456, 10).await?;
```

---

### 戳一戳好友

> `client.send_friend_poke(user_id)`

向好友发送戳一戳。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 好友 QQ 号 |

---

## 群信息

### 获取群信息

> `client.get_group_info(group_id, no_cache)`

获取群信息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `no_cache` | `bool` | 是否不使用缓存 |

**返回值** `GroupInfoResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `group_name` | `String` | 群名称 |
| `member_count` | `Option<i64>` | 当前成员数 |
| `max_member_count` | `Option<i64>` | 最大成员数 |

```rust
let info = client.get_group_info(654321, false).await?;
println!("群名: {}  成员: {}/{}",
    info.group_name,
    info.member_count.unwrap_or(0),
    info.max_member_count.unwrap_or(0)
);
```

---

### 获取群列表

> `client.get_group_list()`

获取已加入的群列表。

**返回值** `Vec<GroupInfoResponse>`

各字段同 `get_group_info` 返回值。

```rust
let groups = client.get_group_list().await?;
for g in groups {
    println!("[{}] {}", g.group_id, g.group_name);
}
```

---

### 获取群成员信息

> `client.get_group_member_info(group_id, user_id, no_cache)`

获取群成员详细信息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 成员 QQ 号 |
| `no_cache` | `bool` | 是否不使用缓存 |

**返回值** `GroupMemberInfoResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `group_id` | `Option<i64>` | 群号 |
| `user_id` | `i64` | QQ 号 |
| `nickname` | `Option<String>` | QQ 昵称 |
| `card` | `Option<String>` | 群名片 |
| `sex` | `Option<String>` | 性别 |
| `age` | `Option<i64>` | 年龄 |
| `role` | `Option<String>` | 角色：`"owner"` / `"admin"` / `"member"` |
| `title` | `Option<String>` | 专属头衔 |
| `join_time` | `Option<i64>` | 入群时间（时间戳） |
| `last_sent_time` | `Option<i64>` | 最后发言时间（时间戳） |
| `level` | `Option<String>` | 群等级 |
| `area` | `Option<String>` | 地区 |

```rust
let member = client.get_group_member_info(654321, 123456, false).await?;
println!("昵称: {}  角色: {:?}",
    member.nickname.unwrap_or_default(),
    member.role
);
```

---

### 获取群成员列表

> `client.get_group_member_list(group_id)`

获取群全部成员列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |

**返回值** `Vec<GroupMemberInfoResponse>`

各字段同 `get_group_member_info` 返回值。

```rust
let members = client.get_group_member_list(654321).await?;
let admins: Vec<_> = members.iter()
    .filter(|m| m.role.as_deref() == Some("admin") || m.role.as_deref() == Some("owner"))
    .collect();
println!("管理员数量: {}", admins.len());
```

---

## 群管理

### 踢出群成员

> `client.set_group_kick(group_id, user_id, reject_add_request)`

将成员踢出群聊。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 要踢出的成员 QQ 号 |
| `reject_add_request` | `bool` | 是否拒绝此人再次加群 |

```rust
// 踢出并拒绝再次加群
client.set_group_kick(654321, 123456, true).await?;
```

---

### 禁言

> `client.set_group_ban(group_id, user_id, duration)`

对群成员进行禁言操作。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 目标成员 QQ 号 |
| `duration` | `i64` | 禁言时长（秒），`0` 表示解除禁言 |

```rust
// 禁言 60 秒
client.set_group_ban(654321, 123456, 60).await?;

// 解除禁言
client.set_group_ban(654321, 123456, 0).await?;
```

---

### 全员禁言

> `client.set_group_whole_ban(group_id, enable)`

开启或关闭全员禁言。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `enable` | `bool` | `true` 开启全员禁言，`false` 关闭 |

```rust
// 开启全员禁言
client.set_group_whole_ban(654321, true).await?;

// 关闭全员禁言
client.set_group_whole_ban(654321, false).await?;
```

---

### 匿名用户禁言

> `client.set_group_anonymous_ban(group_id, anonymous_flag, duration)`

对匿名用户进行禁言。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `anonymous_flag` | `&str` | 匿名用户的 flag（从事件中获取） |
| `duration` | `i64` | 禁言时长（秒） |

---

### 设置管理员

> `client.set_group_admin(group_id, user_id, enable)`

设置或取消群管理员。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 目标成员 QQ 号 |
| `enable` | `bool` | `true` 设为管理员，`false` 取消管理员 |

---

### 设置群名片

> `client.set_group_card(group_id, user_id, card)`

设置群成员名片（群备注）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 目标成员 QQ 号 |
| `card` | `&str` | 新名片内容，空字符串表示删除名片 |

```rust
client.set_group_card(654321, 123456, "新名片").await?;
```

---

### 修改群名

> `client.set_group_name(group_id, group_name)`

修改群名称。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `group_name` | `&str` | 新群名 |

---

### 退群 / 解散群

> `client.set_group_leave(group_id, is_dismiss)`

退出群聊。若为群主且 `is_dismiss` 为 `true`，则解散该群。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `is_dismiss` | `bool` | 是否解散群（仅群主有效） |

---

### 设置专属头衔

> `client.set_group_special_title(group_id, user_id, special_title, duration)`

设置群成员专属头衔（仅群主可用）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 目标成员 QQ 号 |
| `special_title` | `&str` | 专属头衔内容，空字符串表示删除 |
| `duration` | `i64` | 有效期（秒），`-1` 表示永久 |

```rust
client.set_group_special_title(654321, 123456, "活跃达人", -1).await?;
```

---

### 开关匿名

> `client.set_group_anonymous(group_id, enable)`

开启或关闭群匿名功能。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `enable` | `bool` | `true` 开启匿名，`false` 关闭 |

---

### 设置群头像

> `client.set_group_portrait(group_id, file, cache)`

设置群头像。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `file` | `&str` | 图片文件路径或 URL |
| `cache` | `i32` | 是否使用缓存，`1` 使用，`0` 不使用 |

---

### 群内戳一戳

> `client.send_group_poke(group_id, user_id)`

在群内发送戳一戳。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `user_id` | `i64` | 目标成员 QQ 号 |

---

### 群打卡

> `client.send_group_sign(group_id)`

在群内进行打卡签到。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |

---

## 群公告 & 精华

### 发送群公告

> `client.send_group_notice(group_id, content)`

发布群公告。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `content` | `&str` | 公告内容 |

```rust
client.send_group_notice(654321, "本群规则更新，请查阅").await?;
```

---

### 获取 @全体 剩余次数

> `client.get_group_at_all_remain(group_id)`

获取群内 @全体成员 的剩余次数。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |

**返回值** `Value`

---

### 设为精华消息

> `client.set_essence_msg(message_id)`

将消息设为精华消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

---

### 取消精华消息

> `client.delete_essence_msg(message_id)`

取消精华消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 消息 ID |

---

### 获取精华消息列表

> `client.get_essence_msg_list(group_id)`

获取群精华消息列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |

**返回值** `Value`

---

## 文件相关

### 上传群文件

> `client.upload_group_file(group_id, file, name, folder)`

上传文件到群文件。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `file` | `&str` | 本地文件路径或 URL |
| `name` | `&str` | 文件显示名称 |
| `folder` | `Option<&str>` | 目标文件夹 ID，`None` 表示上传到根目录 |

```rust
// 上传到根目录
client.upload_group_file(654321, "/tmp/report.pdf", "周报.pdf", None).await?;

// 上传到指定文件夹
client.upload_group_file(654321, "/tmp/data.xlsx", "数据.xlsx", Some("folder_id")).await?;
```

---

### 上传私聊文件

> `client.upload_private_file(user_id, file, name)`

发送私聊文件。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | 目标用户 QQ 号 |
| `file` | `&str` | 本地文件路径或 URL |
| `name` | `&str` | 文件显示名称 |

---

### 获取群根目录文件

> `client.get_group_root_files(group_id)`

获取群文件根目录下的文件列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |

**返回值** `Value`

---

### 获取文件夹内文件

> `client.get_group_files_by_folder(group_id, folder_id)`

获取群文件夹内的文件列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `folder_id` | `&str` | 文件夹 ID |

**返回值** `Value`

---

### 获取群文件下载链接

> `client.get_group_file_url(group_id, file_id, busid)`

获取群文件的下载链接。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `file_id` | `&str` | 文件 ID |
| `busid` | `i32` | 文件业务 ID |

**返回值** `Value`

---

### 创建群文件夹

> `client.create_group_file_folder(group_id, name)`

在群文件中创建文件夹。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `name` | `&str` | 文件夹名称 |

---

### 删除群文件夹

> `client.delete_group_folder(group_id, folder_id)`

删除群文件夹。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `folder_id` | `&str` | 文件夹 ID |

---

### 删除群文件

> `client.delete_group_file(group_id, file_id, busid)`

删除群文件。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `file_id` | `&str` | 文件 ID |
| `busid` | `i32` | 文件业务 ID |

---

### 下载文件

> `client.download_file(url, thread_count, headers)`

下载文件到本地缓存目录。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `url` | `&str` | 下载链接 |
| `thread_count` | `i32` | 下载线程数 |
| `headers` | `&str` | 自定义请求头 |

**返回值** `Value`

```rust
let result = client.download_file("https://example.com/file.zip", 4, "").await?;
```

---

## 请求处理

### 处理好友请求

> `client.set_friend_add_request(flag, approve, remark)`

处理加好友请求。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `flag` | `&str` | 请求标识（从请求事件中获取） |
| `approve` | `bool` | 是否同意 |
| `remark` | `Option<&str>` | 好友备注（仅同意时有效） |

```rust
// 同意好友请求并添加备注
client.set_friend_add_request("flag_xxx", true, Some("备注名")).await?;

// 拒绝好友请求
client.set_friend_add_request("flag_xxx", false, None).await?;
```

---

### 处理群请求

> `client.set_group_add_request(flag, sub_type, approve, reason)`

处理加群请求或群邀请。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `flag` | `&str` | 请求标识（从请求事件中获取） |
| `sub_type` | `&str` | 请求子类型：`"add"` 加群请求 / `"invite"` 邀请入群 |
| `approve` | `bool` | 是否同意 |
| `reason` | `Option<&str>` | 拒绝理由（仅拒绝时有效） |

```rust
// 同意加群申请
client.set_group_add_request("flag_xxx", "add", true, None).await?;

// 拒绝并附带理由
client.set_group_add_request("flag_xxx", "add", false, Some("不符合入群条件")).await?;
```

---

## 账号相关

### 获取登录信息

> `client.get_login_info()`

获取当前登录的 Bot 账号信息。

**返回值** `LoginInfoResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `user_id` | `i64` | Bot QQ 号 |
| `nickname` | `String` | Bot 昵称 |

```rust
let info = client.get_login_info().await?;
println!("Bot: {} ({})", info.nickname, info.user_id);
```

---

### 获取运行状态

> `client.get_status()`

获取 OneBot 实现的运行状态。

**返回值** `Value`

---

### 获取版本信息

> `client.get_version_info()`

获取 OneBot 实现的版本信息。

**返回值** `Value`

```rust
let ver = client.get_version_info().await?;
println!("版本信息: {}", ver);
```

---

### 是否能发送图片

> `client.can_send_image()`

检查当前 OneBot 实现是否支持发送图片。

**返回值** `CanSendResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `yes` | `bool` | 是否支持 |

---

### 是否能发送语音

> `client.can_send_record()`

检查当前 OneBot 实现是否支持发送语音。

**返回值** `CanSendResponse`

| 字段 | 类型 | 说明 |
|------|------|------|
| `yes` | `bool` | 是否支持 |

---

### 获取在线客户端

> `client.get_online_clients(no_cache)`

获取当前账号在线的客户端列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `no_cache` | `bool` | 是否不使用缓存 |

**返回值** `Value`

---

### 设置 QQ 资料

> `client.set_qq_profile(nickname, company, email, college, personal_note)`

设置 Bot 的 QQ 个人资料。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `nickname` | `&str` | 昵称 |
| `company` | `&str` | 公司 |
| `email` | `&str` | 邮箱 |
| `college` | `&str` | 学校 |
| `personal_note` | `&str` | 个人说明 |

```rust
client.set_qq_profile("MyBot", "", "bot@example.com", "", "一个聊天机器人").await?;
```

---

### 获取自定义表情

> `client.fetch_custom_face()`

获取账号的自定义表情列表。

**返回值** `Value`

---

## 表情回应

### 消息表情点赞

> `client.set_msg_emoji_like(message_id, emoji_id, set)`

对消息进行表情回应（点赞/取消点赞）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `message_id` | `i64` | 目标消息 ID |
| `emoji_id` | `&str` | 表情 ID |
| `set` | `bool` | `true` 添加回应，`false` 取消回应 |

```rust
// 给消息添加表情回应
client.set_msg_emoji_like(12345, "128077", true).await?;
```

---

### 群消息表情回应

> `client.set_group_reaction(group_id, message_id, code, is_add)`

在群消息上添加或取消表情回应。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `group_id` | `i64` | 群号 |
| `message_id` | `i64` | 消息 ID |
| `code` | `&str` | 表情 code |
| `is_add` | `bool` | `true` 添加，`false` 取消 |

---

## 频道相关

::: tip 提示
频道相关 API 用于 QQ 频道（Guild）操作，与普通群聊不同。频道 ID 和用户 ID 均为字符串类型。
:::

### 发送频道消息

> `client.send_guild_channel_msg(guild_id, channel_id, message)`

向频道子频道发送消息。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `guild_id` | `&str` | 频道 ID |
| `channel_id` | `&str` | 子频道 ID |
| `message` | `Message` | 消息内容 |

**返回值** `Value`

```rust
let msg = Message::text("频道消息测试");
client.send_guild_channel_msg("guild_123", "channel_456", msg).await?;
```

---

### 获取频道列表

> `client.get_guild_list()`

获取已加入的频道列表。

**返回值** `Value`

---

### 获取子频道列表

> `client.get_guild_channel_list(guild_id, no_cache)`

获取频道下的子频道列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `guild_id` | `&str` | 频道 ID |
| `no_cache` | `bool` | 是否不使用缓存 |

**返回值** `Value`

---

### 获取频道成员列表

> `client.get_guild_member_list(guild_id, next_token)`

分页获取频道成员列表。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `guild_id` | `&str` | 频道 ID |
| `next_token` | `&str` | 翻页 token，首次请求传空字符串 |

**返回值** `Value`

---

### 获取频道成员资料

> `client.get_guild_member_profile(guild_id, user_id)`

获取频道成员的详细资料。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `guild_id` | `&str` | 频道 ID |
| `user_id` | `&str` | 成员 ID |

**返回值** `Value`

---

## 其他

### URL 安全检查

> `client.check_url_safely(url)`

检查 URL 是否安全。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `url` | `&str` | 要检查的 URL |

**返回值** `Value`

---

### 图片 OCR

> `client.ocr_image(image)`

对图片进行文字识别（OCR）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `image` | `&str` | 图片 ID 或 URL |

**返回值** `Value`

```rust
let result = client.ocr_image("image_id_xxx").await?;
println!("识别结果: {}", result);
```

---

### 自定义 API 调用

> `client.custom_action(action, params)`

调用任意 OneBot API（用于扩展 API 或尚未封装的接口）。

**参数**

| 参数 | 类型 | 说明 |
|------|------|------|
| `action` | `&str` | API 名称（action 字段） |
| `params` | `Value` | 请求参数（JSON 格式） |

**返回值** `NormalizedActionResponse`

```rust
use serde_json::json;

// 调用自定义或扩展 API
let params = json!({ "group_id": 654321, "key": "value" });
let response = client.custom_action("some_extended_api", params).await?;
println!("响应状态: {:?}", response);
```

::: tip 提示
当 OneBot 实现提供了标准协议之外的扩展 API 时，可以使用 `custom_action` 进行调用，无需等待框架适配。
:::

---

## 返回类型汇总

### LoginInfoResponse

```rust
pub struct LoginInfoResponse {
    pub user_id: i64,
    pub nickname: String,
}
```

### SendMsgResponse

```rust
pub struct SendMsgResponse {
    pub message_id: i64,
}
```

### GetMsgResponse

```rust
pub struct GetMsgResponse {
    pub message_id: i64,
    pub real_id: Option<i64>,
    pub sender: Option<SenderInfo>,
    pub time: Option<i64>,
    pub message: Value,
    pub raw_message: Option<String>,
}
```

### GetForwardMsgResponse

```rust
pub struct GetForwardMsgResponse {
    pub message: Value,
}
```

### GroupInfoResponse

```rust
pub struct GroupInfoResponse {
    pub group_id: i64,
    pub group_name: String,
    pub member_count: Option<i64>,
    pub max_member_count: Option<i64>,
}
```

### GroupMemberInfoResponse

```rust
pub struct GroupMemberInfoResponse {
    pub group_id: Option<i64>,
    pub user_id: i64,
    pub nickname: Option<String>,
    pub card: Option<String>,
    pub sex: Option<String>,
    pub age: Option<i64>,
    pub role: Option<String>,        // "owner" | "admin" | "member"
    pub title: Option<String>,
    pub join_time: Option<i64>,
    pub last_sent_time: Option<i64>,
    pub level: Option<String>,
    pub area: Option<String>,
}
```

### FriendInfoResponse

```rust
pub struct FriendInfoResponse {
    pub user_id: i64,
    pub nickname: String,
    pub remark: Option<String>,
}
```

### StrangerInfoResponse

```rust
pub struct StrangerInfoResponse {
    pub user_id: i64,
    pub nickname: String,
    pub sex: Option<String>,
    pub age: Option<i64>,
}
```

### CanSendResponse

```rust
pub struct CanSendResponse {
    pub yes: bool,
}
```

### SenderInfo

```rust
pub struct SenderInfo {
    pub user_id: Option<i64>,
    pub nickname: Option<String>,
    pub card: Option<String>,
    pub sex: Option<String>,
    pub age: Option<i64>,
    pub area: Option<String>,
    pub level: Option<String>,
    pub role: Option<String>,
    pub title: Option<String>,
}
```
