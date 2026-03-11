# 消息 API 参考

本页列出 `qimen-message` crate 提供的所有消息相关类型和方法。

## Message

消息对象，由多个消息段组成。

```rust
pub struct Message {
    pub segments: Vec<Segment>,
    pub raw_text: Option<String>,
    pub raw_segments: Option<Vec<Segment>>,
}
```

### 创建消息

| 方法 | 说明 |
|------|------|
| `Message::new()` | 创建空消息 |
| `Message::text(text)` | 创建纯文本消息 |
| `Message::builder()` | 创建 MessageBuilder（推荐） |
| `Message::from_segments(segments)` | 从消息段列表创建 |
| `Message::from_onebot_value(value)` | 从 OneBot JSON 解析 |
| `Message::from_cq_string(input)` | 从 CQ 码字符串解析 |

### 内容检测

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `plain_text()` | `String` | 合并所有文本段的纯文本 |
| `has_at_all()` | `bool` | 是否包含 @全体 |
| `has_at(user_id)` | `bool` | 是否 @了指定用户 |
| `has_image()` | `bool` | 是否包含图片 |
| `has_record()` | `bool` | 是否包含语音 |
| `has_video()` | `bool` | 是否包含视频 |
| `has_reply()` | `bool` | 是否引用了消息 |

### 内容提取

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `at_list()` | `Vec<&str>` | 所有被 @的用户 ID（不含 "all"） |
| `reply_id()` | `Option<&str>` | 引用的消息 ID |
| `image_urls()` | `Vec<&str>` | 所有图片 URL |
| `record_urls()` | `Vec<&str>` | 所有语音 URL |
| `video_urls()` | `Vec<&str>` | 所有视频 URL |

### 序列化

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `to_onebot_value()` | `Value` | 转换为 OneBot JSON 格式 |
| `push(segment)` | — | 添加一个消息段 |

## MessageBuilder

链式消息构建器。通过 `Message::builder()` 创建。

### 文本与提及

| 方法 | 参数 | 说明 |
|------|------|------|
| `.text(text)` | `impl Into<String>` | 文本段 |
| `.tts(text)` | `impl Into<String>` | TTS 语音文本 |
| `.at(target)` | `impl Into<String>` | @某人（QQ 号） |
| `.at_all()` | — | @全体成员 |

### 多媒体

| 方法 | 参数 | 说明 |
|------|------|------|
| `.image(file)` | `impl Into<String>` | 图片（URL 或本地路径） |
| `.flash_image(file)` | `impl Into<String>` | 闪照（阅后即焚） |
| `.image_with_opts(file, cache, proxy)` | `String, bool, bool` | 带缓存/代理选项的图片 |
| `.record(file)` | `impl Into<String>` | 语音消息 |
| `.record_magic(file)` | `impl Into<String>` | 变声语音 |
| `.video(file)` | `impl Into<String>` | 视频消息 |
| `.card_image(file)` | `impl Into<String>` | 装扮卡片大图 |

### 表情与互动

| 方法 | 参数 | 说明 |
|------|------|------|
| `.face(id)` | `impl Into<String>` | QQ 表情（ID） |
| `.rps()` | — | 猜拳 |
| `.dice()` | — | 骰子 |
| `.shake()` | — | 窗口抖动 |
| `.poke(type, id)` | `impl Into<String>` x2 | 戳一戳 |
| `.anonymous()` | — | 匿名模式 |

### 分享

| 方法 | 参数 | 说明 |
|------|------|------|
| `.share(url, title)` | `impl Into<String>` x2 | 链接分享卡片 |
| `.contact(type, id)` | `impl Into<String>` x2 | 推荐联系人 |
| `.location(lat, lon, title)` | `f64, f64, impl Into<String>` | 位置分享 |
| `.music(type, id)` | `impl Into<String>` x2 | 音乐（平台+ID） |
| `.music_custom(url, audio, title)` | `impl Into<String>` x3 | 自定义音乐 |

### 引用与转发

| 方法 | 参数 | 说明 |
|------|------|------|
| `.reply(message_id)` | `impl Into<String>` | 引用回复 |
| `.forward(id)` | `impl Into<String>` | 合并转发 |
| `.node(user_id, nickname, content)` | `String, String, Message` | 转发节点 |

### 格式化

| 方法 | 参数 | 说明 |
|------|------|------|
| `.xml(data)` | `impl Into<String>` | XML 消息 |
| `.json_msg(data)` | `impl Into<String>` | JSON 消息 |
| `.markdown(content)` | `impl Into<String>` | Markdown |
| `.keyboard(kb)` | `Keyboard` | 交互按钮 |

### 通用

| 方法 | 参数 | 说明 |
|------|------|------|
| `.segment(segment)` | `Segment` | 添加任意段 |
| `.build()` | — | 构建 `Message` |

## Segment

单个消息段。

```rust
pub struct Segment {
    pub kind: String,              // 段类型（如 "text"、"at"、"image"）
    pub data: Map<String, Value>,  // 段数据
}
```

### 构造方法

| 方法 | 说明 |
|------|------|
| `Segment::new(kind)` | 创建指定类型的空段 |
| `Segment::text(text)` | 文本段 |
| `Segment::at(target)` | @段 |
| `Segment::reply(message_id)` | 引用回复段 |
| `Segment::image(file)` | 图片段 |
| `Segment::face(id)` | 表情段 |
| `Segment::record(file)` | 语音段 |
| `Segment::video(file)` | 视频段 |
| `Segment::share(url, title)` | 分享段 |
| `Segment::poke(type, id)` | 戳一戳段 |
| `Segment::contact(type, id)` | 联系人段 |
| `Segment::location(lat, lon, title)` | 位置段 |
| `Segment::music(type, id)` | 音乐段 |
| `Segment::music_custom(url, audio, title)` | 自定义音乐段 |
| `Segment::forward(id)` | 转发段 |
| `Segment::node(user_id, nickname, content)` | 转发节点段 |
| `Segment::xml(data)` | XML 段 |
| `Segment::json_msg(data)` | JSON 段 |
| `Segment::tts(text)` | TTS 段 |
| `Segment::card_image(file)` | 卡片图片段 |
| `Segment::markdown(content)` | Markdown 段 |
| `Segment::rps()` | 猜拳段 |
| `Segment::dice()` | 骰子段 |
| `Segment::shake()` | 抖动段 |
| `Segment::anonymous()` | 匿名段 |
| `Segment::flash_image(file)` | 闪照段 |
| `Segment::image_with_opts(file, cache, proxy)` | 带选项图片段 |
| `Segment::record_magic(file)` | 变声语音段 |

### 检测方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `is_text()` | `bool` | 是否文本段 |
| `is_at()` | `bool` | 是否 @段 |
| `get_text()` | `Option<&str>` | 获取文本内容 |
| `at_target()` | `Option<String>` | 获取 @目标（兼容字符串和数字格式） |
| `data_str(key)` | `Option<&str>` | 获取 data 中的字符串字段 |
| `data_lossless(key)` | `Option<String>` | 获取 data 中的字段（无损转字符串） |

### 修改与序列化

| 方法 | 说明 |
|------|------|
| `.with(key, value)` | 设置 data 字段（链式调用） |
| `from_onebot_value(value)` | 从 OneBot JSON 解析 |
| `from_cq_code(input)` | 从 CQ 码解析 |
| `to_onebot_value()` | 转为 OneBot JSON |

## CQ 码

CQ 码是 OneBot 的传统消息格式。QimenBot 支持双向转换。

| 函数 | 说明 |
|------|------|
| `parse_cq_string(input)` | 将 CQ 码字符串解析为 `Message` |
| `to_cq_string(message)` | 将 `Message` 序列化为 CQ 码字符串 |
| `cq_escape(text)` | 转义特殊字符：`& [ ]` → `&amp; &#91; &#93;` |
| `cq_unescape(text)` | 反转义 |

::: tip CQ 码示例
```
你好[CQ:at,qq=123456]，这是一条[CQ:face,id=1]消息
```
等价于：文本("你好") + @(123456) + 文本("，这是一条") + 表情(1) + 文本("消息")
:::

## KeyboardBuilder

交互按钮构建器。

```rust
use qimen_message::keyboard::*;

let kb = KeyboardBuilder::new()
    .command_button("帮助", "/help")
    .jump_button("GitHub", "https://github.com")
    .row()
    .callback_button("回调", "data")
    .build();
```

### 方法

| 方法 | 参数 | 说明 |
|------|------|------|
| `new()` | — | 创建构建器 |
| `button(label, action, data)` | `&str, ButtonAction, &str` | 通用按钮 |
| `command_button(label, cmd)` | `&str, &str` | 命令按钮（点击发送命令） |
| `jump_button(label, url)` | `&str, &str` | 跳转按钮（点击打开URL） |
| `callback_button(label, data)` | `&str, &str` | 回调按钮 |
| `style(style)` | `ButtonStyle` | 设置最近按钮的样式 |
| `permission(perm)` | `ButtonPermission` | 设置最近按钮的权限 |
| `row()` | — | 换行，开始新的一行 |
| `build()` | — | 构建 `Keyboard` |

### ButtonAction

| 变体 | 值 | 说明 |
|------|---|------|
| `Jump` | 0 | 打开 URL |
| `Callback` | 1 | 触发回调 |
| `Command` | 2 | 发送命令 |

### ButtonStyle

| 变体 | 值 | 说明 |
|------|---|------|
| `Grey` | 0 | 灰色（默认） |
| `Blue` | 1 | 蓝色 |

### ButtonPermission

| 变体 | 值 | 说明 |
|------|---|------|
| `SpecifiedUsers` | 0 | 指定用户可点 |
| `Manager` | 1 | 仅管理员 |
| `All` | 2 | 所有人 |
| `SpecifiedRoles` | 3 | 指定角色 |
