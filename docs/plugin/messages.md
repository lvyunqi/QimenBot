# 消息构建

QimenBot 支持发送各种类型的消息：文本、图片、@、表情、分享卡片、交互按钮等。

## 消息是什么？

一条消息由多个**消息段（Segment）**组成，每个段代表一种内容：

```
Message = [文本段] + [@段] + [表情段] + [图片段] + ...
```

这和 OneBot 11 协议的消息段格式完全对应。

## 三种创建方式

### 方式一：MessageBuilder（推荐）

链式调用，直观清晰：

```rust
let msg = Message::builder()
    .text("Hello ")
    .at("123456")
    .text(" 你好！")
    .face(1)
    .build();
```

### 方式二：快捷方法

```rust
// 纯文本
let msg = Message::text("Hello!");

// 从消息段构建
let msg = Message::from_segments(vec![
    Segment::text("Hello "),
    Segment::at("123456"),
]);
```

### 方式三：直接返回字符串

在命令处理器中，直接返回字符串会自动转为文本消息：

```rust
#[command("打招呼")]
async fn hello(&self) -> &str {
    "Hello!" // 自动变成 Message::text("Hello!")
}
```

## MessageBuilder 完整 API

### 文本与提及

| 方法 | 参数 | 说明 |
|------|------|------|
| `.text(text)` | `impl Into<String>` | 普通文本 |
| `.at(target)` | `impl Into<String>` | @某人（传 QQ 号） |
| `.at_all()` | — | @全体成员 |
| `.tts(text)` | `impl Into<String>` | TTS 语音合成文本 |

```rust
Message::builder()
    .text("你好 ")
    .at("123456")
    .text("\n")
    .text("大家好 ")
    .at_all()
    .build()
```

### 多媒体

| 方法 | 参数 | 说明 |
|------|------|------|
| `.image(file)` | `impl Into<String>` | 图片（URL 或本地路径） |
| `.flash_image(file)` | `impl Into<String>` | 闪照（阅后即焚） |
| `.image_with_opts(file, cache, proxy)` | `String, bool, bool` | 带选项的图片 |
| `.record(file)` | `impl Into<String>` | 语音消息 |
| `.record_magic(file)` | `impl Into<String>` | 变声语音 |
| `.video(file)` | `impl Into<String>` | 视频消息 |
| `.card_image(file)` | `impl Into<String>` | 装扮大图 |

```rust
// 发送网络图片
Message::builder()
    .text("看看这张图：\n")
    .image("https://example.com/photo.jpg")
    .build()

// 发送闪照
Message::builder()
    .flash_image("https://example.com/secret.jpg")
    .build()
```

### 表情与互动

| 方法 | 参数 | 说明 |
|------|------|------|
| `.face(id)` | `impl Into<String>` | QQ 表情（ID 编号） |
| `.rps()` | — | 猜拳 |
| `.dice()` | — | 骰子 |
| `.shake()` | — | 窗口抖动（私聊） |
| `.poke(type, id)` | `impl Into<String>` x2 | 戳一戳 |
| `.anonymous()` | — | 匿名模式 |

::: tip QQ 表情 ID
常见表情 ID：0=惊讶 1=撇嘴 2=色 4=得意 5=流泪 6=害羞 14=微笑 21=飞吻 ...
完整列表可参考 [OneBot 11 文档](https://github.com/botuniverse/onebot-11/blob/master/message/segment.md#qq-%E8%A1%A8%E6%83%85)。
:::

### 分享类

| 方法 | 参数 | 说明 |
|------|------|------|
| `.share(url, title)` | `String, String` | 链接分享卡片 |
| `.contact(type, id)` | `String, String` | 推荐联系人（`"qq"` 或 `"group"`） |
| `.location(lat, lon, title)` | `f64, f64, String` | 位置分享 |
| `.music(type, id)` | `String, String` | 音乐（`"163"` / `"qq"` + 歌曲ID） |
| `.music_custom(url, audio, title)` | `String, String, String` | 自定义音乐 |

```rust
// 分享链接
Message::builder()
    .share("https://github.com/lvyunqi/QimenBot", "QimenBot - Rust Bot 框架")
    .build()

// 分享位置
Message::builder()
    .location(39.9042, 116.4074, "北京天安门")
    .build()
```

### 引用与转发

| 方法 | 参数 | 说明 |
|------|------|------|
| `.reply(message_id)` | `impl Into<String>` | 引用回复某条消息 |
| `.forward(id)` | `impl Into<String>` | 合并转发 |
| `.node(user_id, nickname, content)` | `String, String, Message` | 转发节点 |

```rust
// 引用回复
let msg_id = ctx.event.message_id().unwrap_or(0);
Message::builder()
    .reply(msg_id)
    .text("收到你的消息了！")
    .build()
```

### 特殊格式

| 方法 | 参数 | 说明 |
|------|------|------|
| `.xml(data)` | `impl Into<String>` | XML 消息 |
| `.json_msg(data)` | `impl Into<String>` | JSON 消息 |
| `.markdown(content)` | `impl Into<String>` | Markdown（部分平台支持） |
| `.keyboard(kb)` | `Keyboard` | 交互按钮 |
| `.segment(segment)` | `Segment` | 添加任意自定义段 |

### 构建

| 方法 | 说明 |
|------|------|
| `.build()` | 完成构建，返回 `Message` |

## 消息解析

收到用户消息后，你可以用这些方法**提取内容**：

### 文本

```rust
let msg = ctx.message();
let text = msg.plain_text(); // 获取纯文本（忽略图片、@等）
```

### @提及

```rust
let at_list = msg.at_list();        // 所有被 @ 的 QQ 号
let mentioned = msg.has_at("123");   // 是否 @ 了某人
let at_all = msg.has_at_all();       // 是否 @全体
```

### 多媒体

```rust
msg.has_image();          // 是否有图片
msg.has_record();         // 是否有语音
msg.has_video();          // 是否有视频

let urls = msg.image_urls();   // 所有图片 URL
let voices = msg.record_urls();  // 所有语音 URL
let videos = msg.video_urls();   // 所有视频 URL
```

### 引用回复

```rust
if msg.has_reply() {
    let reply_id = msg.reply_id().unwrap_or_default();
    // reply_id 是被引用消息的 ID
}
```

## 交互按钮（Keyboard）

交互按钮让用户可以点击按钮来触发操作。

### 基本用法

```rust
use qimen_message::keyboard::*;

let kb = KeyboardBuilder::new()
    .command_button("帮助", "/help")       // 点击后发送 /help
    .jump_button("GitHub", "https://github.com/lvyunqi/QimenBot")  // 打开链接
    .row()                                  // 换行
    .callback_button("回调", "my_data")    // 触发回调
    .build();

let msg = Message::builder()
    .text("请选择操作：")
    .keyboard(kb)
    .build();
```

### 按钮类型

| 类型 | 方法 | 点击效果 |
|------|------|---------|
| 命令按钮 | `command_button(label, cmd)` | 将命令发送到输入框 |
| 跳转按钮 | `jump_button(label, url)` | 打开 URL |
| 回调按钮 | `callback_button(label, data)` | 触发回调事件 |

### 样式和权限

```rust
let kb = KeyboardBuilder::new()
    .command_button("蓝色按钮", "/help")
    .style(ButtonStyle::Blue)              // 蓝色样式
    .permission(ButtonPermission::All)     // 所有人可点
    .command_button("管理员专用", "/admin")
    .style(ButtonStyle::Grey)
    .permission(ButtonPermission::Manager) // 仅管理员
    .build();
```

| 样式 | 说明 |
|------|------|
| `ButtonStyle::Grey` | 灰色（默认） |
| `ButtonStyle::Blue` | 蓝色（推荐用于主要操作） |

| 权限 | 说明 |
|------|------|
| `ButtonPermission::All` | 所有人可点 |
| `ButtonPermission::Manager` | 仅管理员 |
| `ButtonPermission::SpecifiedUsers` | 指定用户 |
| `ButtonPermission::SpecifiedRoles` | 指定角色 |

::: warning 兼容性提醒
Keyboard 功能依赖具体的 OneBot 实现和聊天平台。不是所有平台和 OneBot 实现都支持交互按钮。
:::

## 实战示例

### 构建一条富媒体回复

```rust
#[command("富媒体消息演示")]
async fn rich(&self, ctx: &CommandPluginContext<'_>) -> Message {
    Message::builder()
        .text("这是一条富媒体消息：\n")
        .text("  @你: ")
        .at(ctx.sender_id())
        .text("\n  表情: ")
        .face("1")
        .face("2")
        .face("3")
        .text("\n  分享: ")
        .share("https://github.com/lvyunqi/QimenBot", "QimenBot")
        .build()
}
```

### 解析用户消息

```rust
#[command("分析消息内容")]
async fn parse(&self, ctx: &CommandPluginContext<'_>) -> String {
    let msg = ctx.message();
    let mut info = vec![];

    let at_list = msg.at_list();
    if !at_list.is_empty() {
        info.push(format!("@了: {}", at_list.join(", ")));
    }

    let images = msg.image_urls();
    if !images.is_empty() {
        info.push(format!("包含 {} 张图片", images.len()));
    }

    if msg.has_reply() {
        info.push(format!("回复了消息 ID: {}",
            msg.reply_id().unwrap_or_default()));
    }

    let text = msg.plain_text();
    if !text.is_empty() {
        info.push(format!("文本内容: {text}"));
    }

    if info.is_empty() {
        "消息中没有可解析的内容".to_string()
    } else {
        info.join("\n")
    }
}
```

### 引用回复

```rust
#[command("引用回复当前消息", aliases = ["rq"])]
async fn reply_quote(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
    let Some(msg_id) = ctx.event.message_id() else {
        return CommandPluginSignal::Reply(Message::text("无法获取消息 ID"));
    };

    let reply = Message::builder()
        .reply(msg_id)
        .text("这是对你的消息的引用回复！")
        .build();

    CommandPluginSignal::Reply(reply)
}
```
