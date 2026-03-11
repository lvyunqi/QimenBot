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

## 消息相关

### 发送消息

| 方法 | 说明 |
|------|------|
| `send_private_msg(user_id, message)` | 发送私聊消息 |
| `send_group_msg(group_id, message)` | 发送群消息 |
| `send_msg(message_type, id, message)` | 通用发送消息 |

```rust
// 发送私聊消息
let msg = Message::text("Hello!");
client.send_private_msg(123456, msg).await?;

// 发送群消息
let msg = Message::builder().text("Hello ").at("123456").build();
client.send_group_msg(654321, msg).await?;
```

### 消息操作

| 方法 | 说明 |
|------|------|
| `get_msg(message_id)` | 获取消息详情 |
| `delete_msg(message_id)` | 撤回消息 |
| `get_forward_msg(id)` | 获取合并转发内容 |

### 合并转发

| 方法 | 说明 |
|------|------|
| `send_group_forward_msg(group_id, messages)` | 发送群合并转发 |
| `send_private_forward_msg(user_id, messages)` | 发送私聊合并转发 |

### 消息历史

| 方法 | 说明 |
|------|------|
| `get_group_msg_history(group_id, message_seq, count)` | 获取群历史消息 |
| `get_friend_msg_history(user_id, message_seq, count)` | 获取好友历史消息 |

## 好友相关

| 方法 | 说明 |
|------|------|
| `get_friend_list()` | 获取好友列表 |
| `get_stranger_info(user_id, no_cache)` | 获取陌生人信息 |
| `delete_friend(user_id)` | 删除好友 |
| `delete_unidirectional_friend(user_id)` | 删除单向好友 |
| `send_like(user_id, times)` | 点赞 |
| `send_friend_poke(user_id)` | 戳一戳好友 |

```rust
// 获取好友列表
let friends = client.get_friend_list().await?;
for friend in friends {
    println!("{}: {}", friend.user_id, friend.nickname);
}
```

## 群相关

### 群信息

| 方法 | 说明 |
|------|------|
| `get_group_info(group_id, no_cache)` | 获取群信息 |
| `get_group_list()` | 获取群列表 |
| `get_group_member_info(group_id, user_id, no_cache)` | 获取群成员信息 |
| `get_group_member_list(group_id)` | 获取群成员列表 |
| `get_group_honor_info(group_id, honor_type)` | 获取群荣誉信息 |

```rust
// 获取群信息
let info = client.get_group_info(123456, false).await?;
println!("群名: {}  成员数: {}", info.group_name, info.member_count.unwrap_or(0));

// 获取群成员信息
let member = client.get_group_member_info(123456, 789012, false).await?;
println!("昵称: {}  角色: {:?}", member.nickname.unwrap_or_default(), member.role);
```

### 群管理

| 方法 | 说明 |
|------|------|
| `set_group_kick(group_id, user_id, reject_add_request)` | 踢出群成员 |
| `set_group_ban(group_id, user_id, duration)` | 禁言（duration=0 解除） |
| `set_group_whole_ban(group_id, enable)` | 全员禁言 |
| `set_group_anonymous_ban(group_id, anonymous_flag, duration)` | 匿名禁言 |
| `set_group_admin(group_id, user_id, enable)` | 设置/取消管理员 |
| `set_group_card(group_id, user_id, card)` | 设置群名片 |
| `set_group_name(group_id, group_name)` | 修改群名 |
| `set_group_leave(group_id, is_dismiss)` | 退群 / 解散群 |
| `set_group_special_title(group_id, user_id, special_title, duration)` | 设置专属头衔 |
| `set_group_anonymous(group_id, enable)` | 开关匿名 |
| `set_group_portrait(group_id, file, cache)` | 设置群头像 |
| `send_group_poke(group_id, user_id)` | 群内戳一戳 |
| `send_group_sign(group_id)` | 群打卡 |

```rust
// 禁言 60 秒
client.set_group_ban(group_id, user_id, 60).await?;

// 解除禁言
client.set_group_ban(group_id, user_id, 0).await?;

// 全员禁言
client.set_group_whole_ban(group_id, true).await?;
```

### 群公告 & 精华

| 方法 | 说明 |
|------|------|
| `send_group_notice(group_id, content)` | 发送群公告 |
| `get_group_at_all_remain(group_id)` | 获取 @全体 剩余次数 |
| `set_essence_msg(message_id)` | 设为精华消息 |
| `delete_essence_msg(message_id)` | 取消精华消息 |
| `get_essence_msg_list(group_id)` | 获取精华消息列表 |

## 文件相关

| 方法 | 说明 |
|------|------|
| `upload_group_file(group_id, file, name, folder)` | 上传群文件 |
| `upload_private_file(user_id, file, name)` | 上传私聊文件 |
| `get_group_root_files(group_id)` | 获取群根目录文件 |
| `get_group_files_by_folder(group_id, folder_id)` | 获取群文件夹文件 |
| `get_group_file_url(group_id, file_id, busid)` | 获取群文件下载链接 |
| `create_group_file_folder(group_id, name)` | 创建群文件夹 |
| `delete_group_folder(group_id, folder_id)` | 删除群文件夹 |
| `delete_group_file(group_id, file_id, busid)` | 删除群文件 |
| `download_file(url, thread_count, headers)` | 下载文件 |

## 请求处理

| 方法 | 说明 |
|------|------|
| `set_friend_add_request(flag, approve, remark)` | 处理好友请求 |
| `set_group_add_request(flag, sub_type, approve, reason)` | 处理群请求 |

```rust
// 同意好友请求
client.set_friend_add_request("flag_xxx", true, Some("备注")).await?;

// 拒绝加群申请
client.set_group_add_request("flag_xxx", "add", false, Some("不符合条件")).await?;
```

## 账号相关

| 方法 | 说明 |
|------|------|
| `get_login_info()` | 获取登录信息 |
| `get_status()` | 获取运行状态 |
| `get_version_info()` | 获取版本信息 |
| `can_send_image()` | 是否能发送图片 |
| `can_send_record()` | 是否能发送语音 |
| `get_online_clients(no_cache)` | 获取在线客户端 |
| `set_qq_profile(nickname, company, email, college, personal_note)` | 设置 QQ 资料 |
| `fetch_custom_face()` | 获取自定义表情 |

## 表情回应

| 方法 | 说明 |
|------|------|
| `set_msg_emoji_like(message_id, emoji_id, set)` | 消息表情点赞 |
| `set_group_reaction(group_id, message_id, code, is_add)` | 群消息表情回应 |

## 频道相关

| 方法 | 说明 |
|------|------|
| `send_guild_channel_msg(guild_id, channel_id, message)` | 发送频道消息 |
| `get_guild_list()` | 获取频道列表 |
| `get_guild_channel_list(guild_id, no_cache)` | 获取子频道列表 |
| `get_guild_member_list(guild_id, next_token)` | 获取频道成员列表 |
| `get_guild_member_profile(guild_id, user_id)` | 获取频道成员资料 |

## 其他

| 方法 | 说明 |
|------|------|
| `check_url_safely(url)` | URL 安全检查 |
| `ocr_image(image)` | 图片 OCR |
| `custom_action(action, params)` | 自定义 API 调用 |

```rust
// 自定义 API 调用
let params = serde_json::json!({ "key": "value" });
let response = client.custom_action("custom_api_name", params).await?;
```

## 返回类型

### LoginInfoResponse

```rust
pub struct LoginInfoResponse {
    pub user_id: i64,
    pub nickname: String,
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
    pub role: Option<String>,    // "owner" | "admin" | "member"
    pub title: Option<String>,
    pub join_time: Option<i64>,
    pub last_sent_time: Option<i64>,
    // ...
}
```

### SendMsgResponse

```rust
pub struct SendMsgResponse {
    pub message_id: i64,
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
