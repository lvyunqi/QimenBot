# 消息构建

QimenBot 提供了强大的消息构建 API，支持文本、图片、@、表情、分享卡片、交互按钮等多种消息类型。

## 消息模型

每条消息由多个**消息段（Segment）**组成：

```
Message
  ├── Segment { kind: "text",  data: { text: "Hello " } }
  ├── Segment { kind: "at",    data: { qq: "123456" } }
  ├── Segment { kind: "face",  data: { id: "1" } }
  └── Segment { kind: "image", data: { file: "https://..." } }
```

这与 OneBot 11 协议的消息段格式完全兼容。

## 创建消息

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

### 方式二：Message 快捷方法

```rust
// 纯文本
let msg = Message::text("Hello!");

// 从消息段构建
let msg = Message::from_segments(vec![
    Segment::text("Hello "),
    Segment::at("123456"),
]);
```

### 方式三：字符串自动转换

在命令处理器中，直接返回字符串会自动转为文本消息：

```rust
#[command("打招呼")]
async fn hello(&self) -> &str {
    "Hello!" // 自动转为 Message::text("Hello!")
}
```

## MessageBuilder 完整 API

### 文本类

```rust
Message::builder()
    .text("普通文本")      // 文本段
    .tts("语音合成文本")    // TTS 语音文本
    .build()
```

### 提及类

```rust
Message::builder()
    .at("123456")       // @某人
    .at_all()           // @全体成员
    .build()
```

### 多媒体类

```rust
Message::builder()
    .image("https://example.com/img.png")           // 图片（URL 或本地路径）
    .flash_image("https://example.com/flash.png")   // 闪照
    .image_with_opts("url", true, true)              // 图片（带缓存和代理选项）
    .record("https://example.com/audio.mp3")         // 语音
    .record_magic("https://example.com/audio.mp3")   // 变声语音
    .video("https://example.com/video.mp4")          // 视频
    .card_image("https://example.com/card.png")      // 装扮卡片图片
    .build()
```

### 表情类

```rust
Message::builder()
    .face(1)            // QQ 表情（ID）
    .rps()              // 猜拳
    .dice()             // 骰子
    .shake()            // 窗口抖动
    .poke(1, 1)         // 戳一戳（类型, ID）
    .build()
```

### 分享类

```rust
Message::builder()
    .share("https://example.com", "网站标题")          // 链接分享
    .contact("qq", "123456")                           // 推荐好友/群
    .location(39.9, 116.4, "天安门")                    // 位置分享
    .music("163", "12345")                             // 音乐（平台, ID）
    .music_custom("http://play.url", "http://audio.url", "歌曲名") // 自定义音乐
    .build()
```

### 特殊类

```rust
Message::builder()
    .reply(12345678)                  // 引用回复（消息 ID）
    .forward("abcdef")               // 合并转发（转发 ID）
    .node(123456, "昵称", "内容")      // 合并转发节点
    .xml("<xml>...</xml>")            // XML 消息
    .json_msg(r#"{"key":"value"}"#)   // JSON 消息
    .markdown("**加粗文本**")          // Markdown（部分平台支持）
    .anonymous()                      // 匿名消息
    .build()
```

### 交互按钮

```rust
use qimen_message::keyboard::*;

let kb = KeyboardBuilder::new()
    .command_button("执行命令", "/help")
    .jump_button("打开链接", "https://example.com")
    .row()  // 换行
    .callback_button("回调", "callback_data")
    .build();

let msg = Message::builder()
    .text("请选择操作：")
    .keyboard(kb)
    .build();
```

## 消息解析

`Message` 对象也提供了丰富的**内容提取**方法：

### 文本提取

```rust
let msg: &Message = ctx.message();

// 获取纯文本（合并所有 text 段）
let text = msg.plain_text();
```

### @提及检测

```rust
// 获取所有被 @ 的用户 ID
let at_list: Vec<String> = msg.at_list();

// 是否 @ 了某个人
let mentioned = msg.has_at("123456");

// 是否 @全体
let at_all = msg.has_at_all();
```

### 多媒体提取

```rust
// 提取所有图片 URL
let image_urls: Vec<String> = msg.image_urls();

// 提取所有语音 URL
let record_urls: Vec<String> = msg.record_urls();

// 提取所有视频 URL
let video_urls: Vec<String> = msg.video_urls();

// 检测是否包含某种媒体
msg.has_image();   // 是否有图片
msg.has_record();  // 是否有语音
msg.has_video();   // 是否有视频
```

### 回复检测

```rust
// 是否引用了其他消息
msg.has_reply();

// 获取引用的消息 ID
if let Some(reply_id) = msg.reply_id() {
    println!("回复了消息: {}", reply_id);
}
```

## 实战示例

### 构建富媒体回复

```rust
#[command("富媒体消息演示")]
async fn rich(&self, ctx: &CommandPluginContext<'_>) -> Message {
    Message::builder()
        .text("这是一条富媒体消息：\n")
        .text("  - @你: ")
        .at(ctx.sender_id())
        .text("\n  - 表情: ")
        .face(1)
        .face(2)
        .face(3)
        .text("\n  - 分享: ")
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

## KeyboardBuilder 详解

交互按钮（Keyboard）让用户可以通过点击按钮来触发操作。

### 按钮类型

| 类型 | 方法 | 说明 |
|------|------|------|
| 命令按钮 | `command_button(label, command)` | 点击后发送命令到输入框 |
| 跳转按钮 | `jump_button(label, url)` | 点击后打开 URL |
| 回调按钮 | `callback_button(label, data)` | 点击后触发回调 |

### 按钮样式

```rust
use qimen_message::keyboard::*;

let kb = KeyboardBuilder::new()
    .command_button("蓝色按钮", "/help")
    .style(ButtonStyle::Blue)
    .command_button("灰色按钮", "/ping")
    .style(ButtonStyle::Grey)
    .build();
```

### 按钮权限

```rust
let kb = KeyboardBuilder::new()
    .command_button("所有人可点", "/help")
    .permission(ButtonPermission::All)
    .command_button("仅管理员", "/admin")
    .permission(ButtonPermission::Manager)
    .build();
```

### 多行按钮

```rust
let kb = KeyboardBuilder::new()
    // 第一行
    .command_button("帮助", "/help")
    .command_button("状态", "/status")
    .row()  // 换行
    // 第二行
    .jump_button("GitHub", "https://github.com/lvyunqi/QimenBot")
    .build();
```

::: tip 注意
Keyboard 功能依赖于具体的 OneBot 实现和聊天平台的支持。不是所有平台都支持交互按钮。
:::
