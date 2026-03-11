// ── 消息构建与提取示例 / Message Building & Extraction Examples ──
//
// 展示 MessageBuilder 链式构建和 Message 提取方法。
// Demonstrates MessageBuilder chaining and Message extraction methods.

use qimen_message::keyboard::{KeyboardBuilder, ButtonAction};
use qimen_plugin_api::prelude::*;

#[module(id = "example-message", version = "0.1.0",
         name = "Message Demo / 消息演示",
         description = "Message building and extraction examples / 消息构建与提取示例")]
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
            .at(sender)               // @发送者 / @sender
            .text("\n")
            .face("21")               // 表情 / emoji face
            .text(" Have a look / 看看这个:\n")
            .image("https://httpbin.org/image/png") // 图片 / image
            .share(                    // 分享链接 / share link
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
            .share("https://github.com/anthropics/claude-code", "Claude Code - GitHub")
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
}
