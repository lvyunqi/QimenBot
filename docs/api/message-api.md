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

### 构造方法

| 方法 | 说明 |
|------|------|
| `Message::new()` | 创建空消息 |
| `Message::text(text)` | 创建纯文本消息 |
| `Message::builder()` | 创建 MessageBuilder |
| `Message::from_segments(segments)` | 从消息段列表创建 |
| `Message::from_onebot_value(value)` | 从 OneBot JSON 解析 |
| `Message::from_cq_string(input)` | 从 CQ 码字符串解析 |

### 内容检测方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `plain_text()` | `String` | 提取所有文本段的纯文本 |
| `has_at_all()` | `bool` | 是否包含 @全体 |
| `has_image()` | `bool` | 是否包含图片 |
| `has_record()` | `bool` | 是否包含语音 |
| `has_video()` | `bool` | 是否包含视频 |
| `has_reply()` | `bool` | 是否引用了消息 |
| `has_at(user_id)` | `bool` | 是否 @了指定用户 |

### 内容提取方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `at_list()` | `Vec<String>` | 所有被 @ 的用户 ID |
| `reply_id()` | `Option<String>` | 引用的消息 ID |
| `image_urls()` | `Vec<String>` | 所有图片 URL |
| `record_urls()` | `Vec<String>` | 所有语音 URL |
| `video_urls()` | `Vec<String>` | 所有视频 URL |

### 序列化方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `to_onebot_value()` | `Value` | 转换为 OneBot JSON 格式 |

### 其他方法

| 方法 | 说明 |
|------|------|
| `push(segment)` | 添加一个消息段 |

## MessageBuilder

链式消息构建器。

```rust
let msg = Message::builder()
    .text("Hello ")
    .at("123456")
    .face(1)
    .build();
```

### 完整方法列表

#### 文本

| 方法 | 参数 | 说明 |
|------|------|------|
| `text(text)` | `&str` | 文本段 |
| `tts(text)` | `&str` | TTS 语音文本 |

#### 提及

| 方法 | 参数 | 说明 |
|------|------|------|
| `at(target)` | `&str` | @某人（QQ 号） |
| `at_all()` | — | @全体成员 |

#### 多媒体

| 方法 | 参数 | 说明 |
|------|------|------|
| `image(file)` | `&str` | 图片（URL 或本地路径） |
| `flash_image(file)` | `&str` | 闪照 |
| `image_with_opts(file, cache, proxy)` | `&str, bool, bool` | 带选项的图片 |
| `record(file)` | `&str` | 语音 |
| `record_magic(file)` | `&str` | 变声语音 |
| `video(file)` | `&str` | 视频 |
| `card_image(file)` | `&str` | 装扮卡片图片 |

#### 表情 & 互动

| 方法 | 参数 | 说明 |
|------|------|------|
| `face(id)` | `i32` | QQ 表情 |
| `rps()` | — | 猜拳 |
| `dice()` | — | 骰子 |
| `shake()` | — | 窗口抖动 |
| `poke(poke_type, id)` | `i32, i32` | 戳一戳 |
| `anonymous()` | — | 匿名 |

#### 分享

| 方法 | 参数 | 说明 |
|------|------|------|
| `share(url, title)` | `&str, &str` | 链接分享 |
| `contact(type, id)` | `&str, &str` | 推荐联系人 |
| `location(lat, lon, title)` | `f64, f64, &str` | 位置 |
| `music(type, id)` | `&str, &str` | 音乐 |
| `music_custom(url, audio, title)` | `&str, &str, &str` | 自定义音乐 |

#### 特殊

| 方法 | 参数 | 说明 |
|------|------|------|
| `reply(message_id)` | `i64` | 引用回复 |
| `forward(id)` | `&str` | 合并转发 |
| `node(user_id, nickname, content)` | `i64, &str, &str` | 转发节点 |
| `xml(data)` | `&str` | XML 消息 |
| `json_msg(data)` | `&str` | JSON 消息 |
| `markdown(content)` | `&str` | Markdown |
| `keyboard(kb)` | `Keyboard` | 交互按钮 |
| `segment(segment)` | `Segment` | 添加任意段 |
| `build()` | — | 构建 Message |

## Segment

单个消息段。

```rust
pub struct Segment {
    pub kind: String,           // 段类型
    pub data: Map<String, Value>, // 段数据
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
| 其他... | 与 MessageBuilder 对应 |

### 检测方法

| 方法 | 返回类型 | 说明 |
|------|---------|------|
| `is_text()` | `bool` | 是否文本段 |
| `is_at()` | `bool` | 是否 @段 |
| `get_text()` | `Option<&str>` | 获取文本内容 |
| `at_target()` | `Option<&str>` | 获取 @目标 |
| `data_str(key)` | `Option<&str>` | 获取 data 中的字符串字段 |

### 修改方法

| 方法 | 说明 |
|------|------|
| `with(key, value)` | 设置 data 字段（链式） |

### 序列化

| 方法 | 说明 |
|------|------|
| `from_onebot_value(value)` | 从 OneBot JSON 解析 |
| `from_cq_code(input)` | 从 CQ 码解析 |
| `to_onebot_value()` | 转为 OneBot JSON |

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
| `command_button(label, command)` | `&str, &str` | 命令按钮 |
| `jump_button(label, url)` | `&str, &str` | 跳转按钮 |
| `callback_button(label, data)` | `&str, &str` | 回调按钮 |
| `style(style)` | `ButtonStyle` | 设置最近按钮的样式 |
| `permission(perm)` | `ButtonPermission` | 设置最近按钮的权限 |
| `row()` | — | 换行 |
| `build()` | — | 构建 Keyboard |

### ButtonAction

```rust
pub enum ButtonAction {
    Jump = 0,      // 打开 URL
    Callback = 1,  // 触发回调
    Command = 2,   // 发送命令
}
```

### ButtonStyle

```rust
pub enum ButtonStyle {
    Grey = 0,  // 灰色
    Blue = 1,  // 蓝色
}
```

### ButtonPermission

```rust
pub enum ButtonPermission {
    SpecifiedUsers = 0,  // 指定用户
    Manager = 1,         // 管理员
    All = 2,             // 所有人
    SpecifiedRoles = 3,  // 指定角色
}
```
