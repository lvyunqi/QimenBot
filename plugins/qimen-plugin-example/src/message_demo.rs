// ── 消息构建与提取示例 / Message Building & Extraction Examples ──
//
// 展示 MessageBuilder 链式构建和 Message 提取方法。
// Demonstrates MessageBuilder chaining and Message extraction methods.

use qimen_message::{
    Segment,
    keyboard::{ButtonAction, ButtonPermission, ButtonStyle, KeyboardBuilder},
};
use qimen_plugin_api::prelude::*;
use serde_json::json;

#[module(
    id = "example-message",
    version = "0.1.0",
    name = "Message Demo / 消息演示",
    description = "Message building and extraction examples / 消息构建与提取示例"
)]
#[commands]
impl MessageDemoModule {
    // ── /rich ────────────────────────────────────────────────────────────
    // 展示 MessageBuilder 链式构建富媒体消息
    // Demonstrates MessageBuilder chained rich-media message building

    #[command("Build a rich message / 构建富媒体消息",
              examples = ["/rich"], category = "examples")]
    async fn rich(&self, ctx: &CommandPluginContext<'_>) -> Message {
        let sender = ctx.sender_id().unwrap_or("0");

        Message::builder()
            .text("Hello / 你好 ")
            .at(sender) // @发送者 / @sender
            .text("\n")
            .face("21") // 表情 / emoji face
            .text(" Have a look / 看看这个:\n")
            .image("https://httpbin.org/image/png") // 图片 / image
            .share(
                // 分享链接 / share link
                "https://github.com",
                "GitHub",
            )
            .build()
    }

    // ── /parse ───────────────────────────────────────────────────────────
    // 展示 Message 的各种提取方法
    // Demonstrates Message extraction/introspection methods

    #[command("Parse the current message / 解析当前消息",
              examples = ["/parse"], category = "examples")]
    async fn parse(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
        let msg = match ctx.message() {
            Some(m) => m,
            None => {
                return CommandPluginSignal::Reply(Message::text(
                    "No message to parse / 无消息可解析",
                ));
            }
        };

        let at_list = msg.at_list();
        let image_urls = msg.image_urls();
        let has_reply = msg.has_reply();
        let reply_id = msg.reply_id().unwrap_or("none");
        let plain = msg.plain_text();

        let report = format!(
            "── Message Analysis / 消息分析 ──\n\
             Plain text / 纯文本: {plain}\n\
             @list: {at_list:?}\n\
             Image URLs / 图片链接: {image_urls:?}\n\
             Has reply / 含引用: {has_reply}\n\
             Reply ID / 引用ID: {reply_id}\n\
             Has image / 含图片: {}\n\
             Has @all / 含@全体: {}",
            msg.has_image(),
            msg.has_at_all(),
        );

        CommandPluginSignal::Reply(Message::text(report))
    }

    // ── /card ────────────────────────────────────────────────────────────
    // 展示分享卡片消息
    // Demonstrates share card message

    #[command("Send a share card / 发送分享卡片",
              examples = ["/card"], category = "examples")]
    async fn card(&self) -> Message {
        Message::builder()
            .share(
                "https://github.com/anthropics/claude-code",
                "Claude Code - GitHub",
            )
            .build()
    }

    // ── /reply-quote ─────────────────────────────────────────────────────
    // 展示引用回复：MessageBuilder.reply(message_id) + text
    // Demonstrates quote-reply: MessageBuilder.reply(message_id) + text

    #[command("Quote-reply the current message / 引用回复当前消息",
              aliases = ["rq"],
              examples = ["/reply-quote"], category = "examples")]
    async fn reply_quote(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
        let message_id = match ctx.event.message_id() {
            Some(id) => id,
            None => {
                return CommandPluginSignal::Reply(Message::text(
                    "No message_id found / 未找到消息ID",
                ));
            }
        };

        // 使用 reply() 设置引用，再追加文本
        // Use reply() to set the quote, then append text
        let msg = Message::builder()
            .reply(message_id.to_string())
            .text("This is a quote reply / 这是一条引用回复")
            .build();

        CommandPluginSignal::Reply(msg)
    }

    // ── /keyboard ────────────────────────────────────────────────────────
    // 展示 KeyboardBuilder 交互按钮
    // Demonstrates KeyboardBuilder interactive buttons

    #[command("Send interactive keyboard / 发送交互键盘",
              aliases = ["kb"],
              examples = ["/keyboard"], category = "examples")]
    async fn keyboard(&self) -> Message {
        let kb = KeyboardBuilder::new()
            // 第一行：命令按钮 / Row 1: command buttons
            .command_button("Ping", "/ping")
            .command_button("Whoami", "/whoami")
            .row()
            // 第二行：跳转按钮 / Row 2: jump button
            .jump_button("GitHub", "https://github.com")
            .button("Help / 帮助", ButtonAction::Command, "/help")
            .row()
            .build();

        Message::builder()
            .text("Interactive keyboard / 交互键盘:")
            .keyboard(kb)
            .build()
    }

    // ── /qq-md ──────────────────────────────────────────────────────────
    // 官方 Bot Markdown 内容消息示例，对齐 botpy demo_at_reply_markdown.py
    // QQ official Markdown content demo, adapted from botpy demo_at_reply_markdown.py

    #[command("Send QQ official Markdown content / 发送官方 Markdown 内容",
              aliases = ["md"],
              examples = ["/qq-md"],
              category = "qq-official")]
    async fn qq_md(&self) -> Message {
        Message::builder()
            .markdown(
                "# 标题\n\
                 ## 简介很开心\n\
                 内容\n\n\
                 - 这条消息会在官方 Bot 适配器中映射为 `markdown.content`\n\
                 - QQ 群/C2C 会自动使用 `msg_type = 2`",
            )
            .build()
    }

    // ── /qq-md-template [template_id] ───────────────────────────────────
    // 官方 Bot Markdown 模板消息示例
    // QQ official Markdown template payload demo

    #[command("Send QQ official Markdown template / 发送官方 Markdown 模板",
              aliases = ["mdtpl"],
              examples = ["/qq-md-template", "/qq-md-template 65"],
              category = "qq-official")]
    async fn qq_md_template(&self, args: Vec<String>) -> Message {
        let template_id = args.first().map(String::as_str).unwrap_or("65");
        let markdown = Segment::new("markdown")
            .with("custom_template_id", json!(template_id))
            .with(
                "params",
                json!([
                    { "key": "title", "values": ["标题"] },
                    {
                        "key": "content",
                        "values": [
                            "为了成为一名合格的巫师，请务必阅读频道公告",
                            "藏馆黑色魔法书"
                        ]
                    }
                ]),
            );

        Message::from_segments(vec![markdown])
    }

    // ── /qq-keyboard ────────────────────────────────────────────────────
    // 官方 Bot Markdown + 自定义键盘示例，对齐 botpy demo_at_reply_keyboard.py
    // QQ official Markdown + self-defined keyboard demo

    #[command("Send QQ official Markdown with keyboard / 发送官方 Markdown + 键盘",
              aliases = ["qkb"],
              examples = ["/qq-keyboard"],
              category = "qq-official")]
    async fn qq_keyboard(&self) -> Message {
        let keyboard = KeyboardBuilder::new()
            .command_button("搜索", "/搜索")
            .style(ButtonStyle::Blue)
            .permission(ButtonPermission::All)
            .row()
            .command_button("Ping", "/ping")
            .button("帮助", ButtonAction::Command, "/help")
            .row()
            .jump_button("GitHub", "https://github.com/lvyunqi/QimenBot")
            .build();

        Message::builder()
            .markdown("# 标题\n## 简介\n内容\n\n点击下方按钮测试官方 Keyboard payload。")
            .keyboard(keyboard)
            .build()
    }

    // ── /qq-keyboard-template [keyboard_id] ─────────────────────────────
    // 官方 Bot Markdown + 模板键盘示例
    // QQ official Markdown + template keyboard payload demo

    #[command("Send QQ official template keyboard / 发送官方模板键盘",
              aliases = ["qkbtpl"],
              examples = ["/qq-keyboard-template", "/qq-keyboard-template 62"],
              category = "qq-official")]
    async fn qq_keyboard_template(&self, args: Vec<String>) -> Message {
        let keyboard_id = args.first().map(String::as_str).unwrap_or("62");
        let markdown = Segment::markdown("# 123\n今天是个好天气");
        let keyboard = Segment::new("keyboard").with("id", json!(keyboard_id));

        Message::from_segments(vec![markdown, keyboard])
    }

    // ── /qq-ark ─────────────────────────────────────────────────────────
    // 官方 Bot Ark 消息示例，对齐 botpy demo_at_reply_ark.py
    // QQ official Ark payload demo

    #[command("Send QQ official Ark payload / 发送官方 Ark 消息",
              examples = ["/qq-ark"],
              category = "qq-official")]
    async fn qq_ark(&self) -> Message {
        let ark = Segment::new("ark").with("template_id", json!(37)).with(
            "kv",
            json!([
                { "key": "#METATITLE#", "value": "通知提醒" },
                { "key": "#PROMPT#", "value": "标题" },
                { "key": "#TITLE#", "value": "标题" },
                {
                    "key": "#METACOVER#",
                    "value": "https://vfiles.gtimg.cn/vupload/20211029/bf0ed01635493790634.jpg"
                }
            ]),
        );

        Message::from_segments(vec![ark])
    }

    // ── /qq-embed ───────────────────────────────────────────────────────
    // 官方 Bot Embed 消息示例，对齐 botpy demo_at_reply_embed.py
    // QQ official Embed payload demo

    #[command("Send QQ official Embed payload / 发送官方 Embed 消息",
              examples = ["/qq-embed"],
              category = "qq-official")]
    async fn qq_embed(&self) -> Message {
        let embed = Segment::new("embed")
            .with("title", json!("embed消息"))
            .with("prompt", json!("消息透传显示"))
            .with(
                "fields",
                json!([
                    { "name": "<@!1234>hello world" },
                    { "name": "<@!1234>hello world" }
                ]),
            );

        Message::from_segments(vec![embed])
    }

    // ── /qq-media <image|record|video|file> <url> ───────────────────────
    // 官方 Bot 群/C2C media upload 示例，对齐 botpy group/c2c file demo
    // QQ official group/C2C media upload demo

    #[command("Send QQ official media by URL / 发送官方 media URL",
              aliases = ["qmedia"],
              examples = ["/qq-media image https://httpbin.org/image/png",
                          "/qq-media record https://example.com/a.amr",
                          "/qq-media video https://example.com/a.mp4",
                          "/qq-media file https://example.com/a.zip"],
              category = "qq-official")]
    async fn qq_media(&self, args: Vec<String>) -> CommandPluginSignal {
        let media_type = args.first().map(String::as_str).unwrap_or("image");
        let url = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "https://httpbin.org/image/png".to_string());

        let segment = match media_type {
            "image" => Segment::image(url),
            "record" | "voice" | "audio" => Segment::record(url),
            "video" => Segment::video(url),
            "file" => Segment::new("file").with("url", json!(url)),
            _ => {
                return CommandPluginSignal::Reply(Message::text(
                    "Usage / 用法: /qq-media <image|record|video|file> <url>",
                ));
            }
        };

        CommandPluginSignal::Reply(Message::from_segments(vec![segment]))
    }
}
