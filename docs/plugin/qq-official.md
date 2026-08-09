# 官方 QQ Bot 插件适配

普通文本命令可以同时运行在 OneBot 11 和官方 QQ Bot 上。真正需要注意的是 ID 类型、消息场景、@ 提及、回复窗口和官方消息格式。

基本原则只有一条：插件先使用 QimenBot 的通用事件和 `Message` API，只有确实需要平台字段时才读取 `qqbot_payload`。

## 启用示例插件

仓库自带两个静态示例模块：

| 模块 | 用途 |
|------|------|
| `example-basic` | `/ping`、`/echo`、`/whoami` 等基础命令 |
| `example-message` | Markdown、Keyboard、Ark、Embed 和媒体测试 |

在 `config/base.toml` 中启用：

```toml
[official_host]
builtin_modules = ["command", "admin"]
plugin_modules = ["example-basic", "example-message"]
```

启动报告中出现这两个 ID 后，先发送 `/ping`。基础文本回复正常，再测试富消息。

## 最小命令

下面的命令不依赖协议专属字段：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "hello")]
#[commands]
impl HelloPlugin {
    #[command("Reply with a greeting", examples = ["/hello"])]
    async fn hello(&self) -> &'static str {
        "你好，我在。"
    }
}
```

命令返回 `&str`、`String` 或 `Message` 时，运行时会根据来信场景选择群、C2C、频道或 DMS 的发送接口。

## ID 一律按字符串处理

官方 QQ Bot 不向插件提供传统 QQ 号和 QQ 群号。事件中的 ID 由开放平台分配，通常是字符串：

| 名称 | 常见位置 | 含义 |
|------|----------|------|
| `member_openid` | QQ 群消息发送者 | 当前群成员的开放 ID |
| `user_openid` | C2C 发送者 | 当前单聊用户的开放 ID |
| `group_openid` | QQ 群消息 | 当前开放群 ID |
| `guild_id` | 频道事件 | 频道或频道私信上下文 |
| `channel_id` | 频道消息 | 子频道 ID |

使用上下文的字符串方法：

```rust
#[command("Show current IDs", examples = ["/whereami"])]
async fn whereami(&self, ctx: &CommandPluginContext<'_>) -> String {
    let sender = ctx.sender_id().unwrap_or("unknown");
    let chat = ctx.chat_id().unwrap_or("unknown");
    let message_id = ctx
        .event
        .message_id_str()
        .unwrap_or_else(|| "unknown".to_string());

    format!("sender={sender}\nchat={chat}\nmessage={message_id}")
}
```

常用方法：

| 方法 | 返回内容 |
|------|----------|
| `ctx.sender_id()` | 发送者的字符串 ID |
| `ctx.chat_id()` | 当前会话的字符串 ID |
| `ctx.group_id()` | QQ 群消息中的 `group_openid`，其他场景通常为空 |
| `ctx.event.message_id_str()` | 不损失内容的字符串消息 ID |
| `ctx.event.self_id_str()` | 有平台自身 ID 时按字符串读取 |

不要把官方 ID 解析成 `i64`。以下方法主要用于 OneBot 数字 ID：

```rust
ctx.sender_id_i64();
ctx.group_id_i64();
ctx.event.message_id();
```

持久化用户数据时，建议至少使用 `(protocol, account_id, sender_id)` 作为联合键。`account_id` 从 `ctx.event.raw_json["qimen_context"]["account_id"]` 读取，对应管理员配置的稳定 `[[bots]].account_id`；`bot_instance` 只是可调整的部署别名，不应作为长期账号主键。缺少稳定账号时应要求管理员补配置，不要把多个 Bot 的状态归入 `unknown`。不同机器人应用收到的 OpenID 不保证互通，也不要拿 OpenID 当作真实 QQ 号展示给用户。

## 识别会话场景

QimenBot 将官方消息映射为四种稳定场景：

| 官方场景 | Gateway 事件 | `message_type()` | `sender_id()` | `chat_id()` | 回复接口 |
|----------|--------------|------------------|---------------|-------------|----------|
| QQ 群 @ / 全量消息 | `GROUP_AT_MESSAGE_CREATE`、`GROUP_MESSAGE_CREATE` | `group` | `member_openid` | `group_openid` | `/v2/groups/{group_openid}/messages` |
| QQ 单聊 C2C | `C2C_MESSAGE_CREATE` | `private` | `user_openid` | `user_openid` | `/v2/users/{openid}/messages` |
| 频道消息 | `AT_MESSAGE_CREATE`、`MESSAGE_CREATE` | `channel` | 频道用户 ID | `channel_id` | `/channels/{channel_id}/messages` |
| 频道私信 | `DIRECT_MESSAGE_CREATE` | `channel_private` | 频道用户 ID | `guild_id` | `/dms/{guild_id}/messages` |

只区分群聊和私聊时：

```rust
let scope = if ctx.is_group() {
    "group"
} else if ctx.is_private() {
    "private"
} else {
    "channel"
};
```

需要区分所有场景时：

```rust
let scene = match ctx.event.message_type() {
    Some("group") => "QQ 群",
    Some("private") => "QQ 单聊",
    Some("channel") => "频道",
    Some("channel_private") => "频道私信",
    _ => "其他",
};
```

频道私信的 `chat_id()` 是 `guild_id`，这是官方 DMS 发送接口所需的目标，不要把它误当成发送者 ID。

## 正确判断 @ 机器人

不要只用事件名判断群消息是否 @ 了机器人。获得全量群消息权限后，平台可能把带 @ 的消息也作为 `GROUP_MESSAGE_CREATE` 下发，并在 `mentions[].is_you` 中标记当前机器人。

统一判断方式：

```rust
if ctx.event.is_at_self() {
    // 这条消息明确指向当前机器人
}
```

适配器会处理三种来源：

- `GROUP_AT_MESSAGE_CREATE` 和 `AT_MESSAGE_CREATE` 事件本身。
- 当前格式 `<qqbot-at-user id="..." />`。
- 旧格式 `<@id>`、`<@!id>` 以及 `mentions[].is_you`。

提及和文字会按原顺序转成消息段。命令前指向机器人的 @ 会被移出纯文本命令内容，因此 `@机器人 /ping` 可以匹配 `/ping`。

读取消息段：

```rust
if let Some(message) = ctx.message() {
    let text = message.plain_text();
    let mentioned_ids = message.at_list();
    let images = message.image_urls();

    tracing::debug!(?text, ?mentioned_ids, ?images);
}
```

附件会按 `content_type` 转成 `image`、`record`、`video` 或 `file` 段。官方未被通用模型覆盖的 `msg_elements` 会保留为 `qqbot_element` 段。

## 回复消息

纯文本回复：

```rust
#[command("Ping", examples = ["/ping"])]
async fn ping(&self) -> &'static str {
    "pong"
}
```

组合消息：

```rust
#[command("Help", examples = ["/help-me"])]
async fn help_me(&self) -> Message {
    Message::builder()
        .text("可用命令：\n")
        .text("/ping - 测试连通\n")
        .text("/whereami - 查看当前 ID")
        .build()
}
```

插件返回回复时，运行时会自动：

1. 根据 `chat.kind` 选择正确的 OpenAPI endpoint。
2. 优先携带原消息的字符串 `msg_id`，没有消息 ID 时才使用 `event_id`。
3. 为群和 C2C 同一来信的多次回复分配递增 `msg_seq`。
4. 避免同时发送互斥的 `msg_id`、`event_id` 和 `is_wakeup=true`。

官方被动回复限制：

| 场景 | 回复窗口 | 同一来信最多回复 |
|------|----------|------------------|
| QQ 群 | 5 分钟 | 5 条 |
| C2C | 60 分钟 | 4 条 |

这些是 QQ 平台限制，不是 QimenBot 的限流器配置。插件处理长任务时应先快速回复，再使用已获准的主动消息能力发送后续结果。

## Markdown 和 Keyboard

Markdown 内容消息：

```rust
#[command("Show Markdown", examples = ["/menu"])]
async fn menu(&self) -> Message {
    Message::builder()
        .markdown("# 菜单\n- /ping 测试连通\n- /whereami 查看会话")
        .build()
}
```

带按钮的消息：

```rust
use qimen_message::keyboard::{ButtonPermission, ButtonStyle, KeyboardBuilder};

#[command("Show keyboard", examples = ["/buttons"])]
async fn buttons(&self) -> Message {
    let keyboard = KeyboardBuilder::new()
        .command_button("Ping", "/ping")
        .style(ButtonStyle::Blue)
        .permission(ButtonPermission::All)
        .row()
        .command_button("查看身份", "/whereami")
        .build();

    Message::builder()
        .markdown("# 测试菜单\n请选择一个命令。")
        .keyboard(keyboard)
        .build()
}
```

群和 C2C 的 Markdown 会使用 `msg_type = 2`，并且不会同时发送普通 `content`。模板 Markdown 和模板 Keyboard 要使用开放平台中有效且获准使用的模板 ID。

## 富消息支持矩阵

不同场景不能共用所有官方消息结构：

| 内容 | QQ 群 | C2C | 频道 / DMS | 说明 |
|------|-------|-----|------------|------|
| 文本 | 支持，`msg_type=0` | 支持，`msg_type=0` | 支持 | 最先用于连通测试 |
| Markdown | 支持，`msg_type=2` | 支持，`msg_type=2` | 支持 | 模板能力取决于平台权限 |
| Keyboard | 支持 | 支持 | 支持 | 通常与 Markdown 一起发送 |
| 图片 URL | 先上传媒体 | 先上传媒体 | 使用频道 `image` 字段 | URL 必须能由 QQ 服务器访问 |
| 本地图片 / Base64 | 分片上传 | 分片上传 | `multipart/form-data` 的 `file_image` | 频道和 DMS 只接受图片 |
| 本地语音、视频、文件 | 分片上传 | 分片上传 | 不支持 | 群/C2C 使用 `msg_type=7` |
| Card | 支持，`msg_type=8` | 不支持 | 当前不发送群专用 Card | 以官方能力开放为准 |
| Input notify | 不支持 | 支持，`msg_type=6` | 不支持 | C2C 输入状态通知 |
| Ark | 不支持 | 不支持 | 支持 | 不能与 Embed 同时发送 |
| Embed | 不支持 | 不支持 | 支持 | 不能与 Ark 同时发送 |

平台是否允许某个机器人实际发送，还取决于机器人类型、审核状态和消息模板配置。框架支持构造 payload，不代表开放平台自动授予权限。

## 群和 C2C 媒体

使用普通消息段即可触发上传：

```rust
#[command("Send image", examples = ["/photo"])]
async fn photo(&self) -> Message {
    Message::builder()
        .image("https://example.com/photo.png")
        .build()
}
```

公网 URL 的执行顺序为：

```text
图片/语音/视频/文件 URL
  -> POST /v2/groups/{group_openid}/files
     或 POST /v2/users/{openid}/files
  -> 读取响应中的 file_info
  -> POST /messages，msg_type=7，media.file_info=...
```

URL 必须使用 `http://` 或 `https://`，并且能由 QQ 服务器直接访问。`localhost`、内网地址、`file://` 和需要登录 Cookie 的下载地址不能用于这条路径。

插件在内存中生成的媒体不需要先上传到自己的服务器。静态插件可以返回 `base64://...` 消息段；动态插件直接使用 builder：

```rust
CommandResponse::builder()
    .image_base64(&png_base64)
    .build()
```

群和 C2C 收到 Base64 段后，宿主执行完整的官方本地文件流程：

```text
Base64 解码和格式校验
  -> POST /upload_prepare（文件大小、文件名、MD5、SHA1、前 10002432 字节 MD5）
  <- upload_id、分片大小、预签名 URL、并发和重试参数
  -> PUT 每个预签名 URL（不携带 QQ Authorization）
  -> POST /upload_part_finish（逐片确认）
  -> POST /files { upload_id, file_type, file_name, srv_send_msg }
  <- file_info
  -> POST /messages { msg_type: 7, media: { file_info } }
```

被动回复和 API 0.4+ 主动发送共用这条执行路径。插件不需要、也无法读取宿主持有的 AppSecret 或 access token。

### 本地媒体格式和大小

Base64 会同时占用编码字符串和解码后的内存，因此宿主对内联数据采用比官方 200 MB 硬限制更保守的上限：

| 类型 | 内联上限 | 宿主校验 |
|------|----------|----------|
| 图片 | 20 MB | PNG、JPEG、GIF、WebP、BMP；实际平台兼容性以当前官方接口为准，PNG/JPEG 最稳妥 |
| 视频 | 30 MB | MP4 |
| 语音 | 20 MB | SILK |
| 文件 | 32 MB | 不限制文件格式 |

超过内联上限时应改用公网 URL。官方接口对单个文件还有 200 MB 硬限制；URL 方式也不能绕过平台限制。

`image_base64()` 等方法接受纯 Base64，也接受已经带有 `base64://` 或 `data:<mime>;base64,` 前缀的内容。Base64 无效、媒体格式不匹配或解码后为空时，请求会在访问 OpenAPI 前失败。宿主不会读取消息段中的本地文件路径，动态插件应先自行读取文件并编码，不能传 `C:\...`、`/tmp/...` 或 `file://...`。

官方单条媒体消息只有一个 `media` / `file_image` 位置。一个 `Message` 中应只放一个图片、视频、语音或文件段；需要发送多项时拆成多条消息，并遵守被动回复次数或主动消息额度。

## 频道和 DMS 本地图片

频道与频道私信不使用群/C2C 的 `/files` 接口。本地图片会直接随消息发送为 `multipart/form-data`：普通字段仍使用 `content`、`msg_id` 或 `event_id`，文件字段名固定为 `file_image`。

```rust
let response = CommandResponse::builder()
    .text("生成结果")
    .image_base64(&png_base64)
    .build();
```

同一段代码可以用于群、C2C、频道和 DMS，宿主按当前会话选择分片上传或 multipart。频道接口不接受本地视频、语音和普通文件。DMS 的本地图片上传方式与官方 botpy 保持一致，但开放平台能力可能因机器人类型和审核状态不同，发布插件前应在真实 DMS 会话中验证。

## 被动回复与主动发送

两种发送方式的语义不同：

| 类型 | 触发方式 | 目标来源 | 官方约束 |
|------|----------|----------|----------|
| 被动回复 | 命令处理器返回 `Message` | 当前事件自动提供 | 携带 `msg_id`，受回复窗口和次数限制 |
| 主动发送 | `BotApi` / `SendBuilder` 或宿主动作 | 插件显式提供 openid / channel ID | 需要对应主动消息权限和平台频控配额 |

动态插件可使用统一主动发送接口：

```rust
use abi_stable_host_api::BotApi;

BotApi::for_bot("qq-official")
    .send_group_msg("目标 group_openid", "定时通知");
```

生产配置建议通过 `account_id` 选择稳定账号，而不是依赖可变的实例名；官方 Bot 可以把稳定的应用 AppID 用作 `account_id`。完整写法见[动态插件实时主动推送](/advanced/dynamic-proactive-send-v04)。

主动发送没有原始 `msg_id`，不要伪造一个过期消息 ID。需要使用 `event_id` 或 `is_wakeup=true` 的低层场景必须遵守三者互斥规则：`msg_id`、`event_id`、`is_wakeup=true` 同一请求只能选择一种触发依据。

## 读取官方原始字段

通用方法不够用时，可以读取扩展字段：

```rust
let event_type = ctx
    .event
    .extensions
    .get("event_type")
    .and_then(serde_json::Value::as_str)
    .unwrap_or("unknown");

let payload = ctx.event.raw_json.get("qqbot_payload");
```

常用位置：

| 位置 | 内容 |
|------|------|
| `event.extensions["event_type"]` | 原始 Gateway 事件名 |
| `event.extensions["event_id"]` | Gateway 事件 ID |
| `event.extensions["sequence"]` | Gateway dispatch 序号 |
| `event.extensions["msg_idx"]` | 全量消息投递索引，可用时存在 |
| `event.extensions["message_scene"]` | 官方消息场景对象 |
| `event.raw_json["qqbot_payload"]` | 原始 Gateway `d` 对象 |

`raw_json` 还提供 `post_type`、`message_type`、`message_id`、`user_id`、`group_id` 等归一化字段，便于通用插件读取。原始对象的结构仍由 QQ 开放平台定义；访问深层字段时必须处理字段缺失和类型变化。

## 互动事件

按钮和快捷菜单可能产生 `INTERACTION_CREATE`。适配器将它归一化为 `notice_type = "interaction_create"`，原始数据保存在 `qqbot_payload`。

对需要确认的互动类型，运行时会自动调用：

```text
PUT /interactions/{interaction_id}
{"code": 0}
```

插件不需要再发一次 ACK。ACK 只负责结束客户端的等待状态，不等同于回复一条聊天消息。

## OneBot API 不是官方 OpenAPI

`ctx.onebot_actions()` 只适用于 OneBot 11：

```rust
let result = ctx
    .onebot_actions()
    .set_group_ban(group_id, user_id, 60)
    .await;
```

官方 QQ Bot 没有传统数字群号和 QQ 号，也不提供完全相同的群管理动作。兼容多协议的插件应先检查数字 ID 是否存在：

```rust
#[command("Read OneBot group info", scope = "group")]
async fn group_info(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
    let Some(group_id) = ctx.group_id_i64() else {
        return CommandPluginSignal::Reply(Message::text(
            "当前协议不支持 OneBot 群信息接口。",
        ));
    };

    match ctx.onebot_actions().get_group_info(group_id, false).await {
        Ok(info) => CommandPluginSignal::Reply(Message::text(info.group_name)),
        Err(err) => CommandPluginSignal::Reply(Message::text(format!("获取失败：{err}"))),
    }
}
```

不要把 OneBot 的 `set_group_ban`、`get_group_member_list`、群文件等方法解释成官方 QQ Bot 能力。需要官方特有动作时，应在 QQ 适配器和通用动作层增加明确支持。

## 权限角色

`role = "admin"` 和 `role = "owner"` 仍可使用，但配置中必须填事件实际返回的字符串 ID：

```toml
[[bots]]
id = "qq-official"
protocol = "qq-official"

owners = ["这里填写 sender_id() 返回的字符串"]
admins = []
```

群事件中的 `member_role` 也会参与管理员判断。最稳妥的配置方法是先用 `/whoami` 查看当前 `sender_id()`，再把完整字符串写入配置。

## 完整示例

```rust
use qimen_plugin_api::prelude::*;

#[module(
    id = "conversation-info",
    version = "0.1.0",
    name = "Conversation Info"
)]
#[commands]
impl ConversationInfoPlugin {
    #[command("Show current conversation", examples = ["/whereami"])]
    async fn whereami(&self, ctx: &CommandPluginContext<'_>) -> Message {
        let scene = ctx.event.message_type().unwrap_or("unknown");
        let sender = ctx.sender_id().unwrap_or("unknown");
        let chat = ctx.chat_id().unwrap_or("unknown");
        let message_id = ctx
            .event
            .message_id_str()
            .unwrap_or_else(|| "unknown".to_string());
        let directed = ctx.event.is_at_self();

        Message::builder()
            .text(format!("场景：{scene}\n"))
            .text(format!("发送者：{sender}\n"))
            .text(format!("会话：{chat}\n"))
            .text(format!("消息：{message_id}\n"))
            .text(format!("@当前机器人：{directed}"))
            .build()
    }
}
```

这个例子不依赖数字 QQ 号，不直接调用 OneBot API，也不根据原始事件名猜测会话类型，可以同时用于 OneBot 11 和官方 QQ Bot 的文本消息。
