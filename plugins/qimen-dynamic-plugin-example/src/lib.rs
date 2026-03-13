//! QimenBot 动态插件完整示例（过程宏写法）
//!
//! 本示例使用 `#[dynamic_plugin]` 过程宏声明插件，代码量比手动 FFI 大幅减少。
//! 宏自动生成 `qimen_plugin_descriptor()` 和所有 `extern "C" fn` 导出。
//!
//! ## 编译 / Build
//!
//! ```bash
//! cd plugins/qimen-dynamic-plugin-example
//! cargo build --release
//! ```
//!
//! ## 安装 / Install
//!
//! ```bash
//! cp target/release/libqimen_dynamic_plugin_example.{so,dylib,dll} ../../plugins/bin/
//! ```

use std::sync::atomic::{AtomicBool, Ordering};

use abi_stable_host_api::{
    BotApi, CommandRequest, CommandResponse, DynamicActionResponse, InterceptorRequest,
    InterceptorResponse, NoticeRequest, NoticeResponse, PluginInitConfig, PluginInitResult,
    SendBuilder,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

// ─── 全局状态 / Global State ─────────────────────────────────────────────────

/// 标记插件是否已初始化 / Whether the plugin has been initialized
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ═════════════════════════════════════════════════════════════════════════════
// 插件定义（宏自动生成 descriptor + extern "C" fn）
// ═════════════════════════════════════════════════════════════════════════════

#[dynamic_plugin(id = "dynamic-example", version = "0.1.0")]
mod example {
    use super::*;

    // ── 生命周期钩子 ──────────────────────────────────────────────────────

    /// 插件加载后由宿主调用。配置从 `config/plugins/dynamic-example.toml` 自动加载。
    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult {
        let plugin_id = config.plugin_id.as_str();
        let config_json = config.config_json.as_str();
        let plugin_dir = config.plugin_dir.as_str();
        let data_dir = config.data_dir.as_str();

        eprintln!(
            "[dynamic-example] init: id={}, config={}, plugin_dir={}, data_dir={}",
            plugin_id,
            if config_json.is_empty() { "<none>" } else { config_json },
            plugin_dir,
            data_dir,
        );

        INITIALIZED.store(true, Ordering::Relaxed);
        PluginInitResult::ok()
    }

    /// 插件卸载前由宿主调用。
    #[shutdown]
    fn on_shutdown() {
        eprintln!("[dynamic-example] shutdown");
        INITIALIZED.store(false, Ordering::Relaxed);
    }

    // ── 命令回调 ──────────────────────────────────────────────────────────

    /// 打招呼 — 演示 ReplyBuilder + sender_nickname + message_id 引用
    #[command(name = "greet", description = "打招呼 / Greet the sender", aliases = "hi,hello,你好", category = "示例")]
    fn greet(req: &CommandRequest) -> CommandResponse {
        let sender = req.sender_id.as_str();
        let nickname = req.sender_nickname.as_str();
        let args = req.args.as_str().trim();

        let display = if nickname.is_empty() { sender } else { nickname };

        let greeting = if args.is_empty() {
            format!(" 你好 {display}！欢迎使用 QimenBot 动态插件示例~")
        } else {
            format!(" {display} 说：{args}")
        };

        let mut builder = CommandResponse::builder();

        let msg_id = req.message_id.as_str();
        if !msg_id.is_empty() {
            builder = builder.reply(msg_id);
        }

        builder
            .at(sender)
            .text(&greeting)
            .face(1)
            .build()
    }

    /// 显示时间 — 演示 CommandResponse::text() + timestamp 字段
    #[command(name = "time", description = "显示时间 / Show current time", aliases = "时间", category = "示例")]
    fn time(req: &CommandRequest) -> CommandResponse {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let event_ts = req.timestamp;

        let msg = if event_ts > 0 {
            let latency = now.saturating_sub(event_ts as u64);
            format!(
                "⏰ 时间信息\n\
                 ├ 服务器时间: {now}\n\
                 ├ 事件时间戳: {event_ts}\n\
                 └ 消息延迟: {latency}s"
            )
        } else {
            format!("⏰ 服务器时间: {now}")
        };

        CommandResponse::text(&msg)
    }

    /// 复读消息 — 演示参数解析 + 空参检查 + ReplyBuilder
    #[command(name = "echo", description = "复读消息 / Echo back the message", aliases = "复读,say", category = "示例")]
    fn echo(req: &CommandRequest) -> CommandResponse {
        let args = req.args.as_str().trim();

        if args.is_empty() {
            return CommandResponse::text("用法：echo <内容>\nUsage: echo <content>");
        }

        let mut builder = CommandResponse::builder();

        let msg_id = req.message_id.as_str();
        if !msg_id.is_empty() {
            builder = builder.reply(msg_id);
        }

        builder.text(args).build()
    }

    /// 请求详情（仅管理员） — 演示 CommandRequest 所有字段
    #[command(name = "info", description = "显示请求详情 / Show request details", aliases = "debug,调试", category = "示例", role = "admin")]
    fn info(req: &CommandRequest) -> CommandResponse {
        let initialized = INITIALIZED.load(Ordering::Relaxed);

        let info = format!(
            "📋 Request Info\n\
             ├ command_name: {}\n\
             ├ args: {:?}\n\
             ├ sender_id: {}\n\
             ├ sender_nickname: {}\n\
             ├ group_id: {}\n\
             ├ message_id: {}\n\
             ├ timestamp: {}\n\
             ├ raw_event_json: {}B\n\
             └ plugin_initialized: {}",
            req.command_name.as_str(),
            req.args.as_str(),
            req.sender_id.as_str(),
            if req.sender_nickname.is_empty() { "<empty>" } else { req.sender_nickname.as_str() },
            if req.group_id.is_empty() { "<private>" } else { req.group_id.as_str() },
            if req.message_id.is_empty() { "<none>" } else { req.message_id.as_str() },
            req.timestamp,
            req.raw_event_json.len(),
            initialized,
        );

        CommandResponse::text(&info)
    }

    /// 仅群聊命令 — 演示 scope = "group"
    #[command(name = "group-hello", description = "仅群聊打招呼 / Greet in group only", category = "示例", scope = "group")]
    fn group_hello(req: &CommandRequest) -> CommandResponse {
        let sender = req.sender_id.as_str();
        CommandResponse::builder()
            .at(sender)
            .text(" 这是一条仅在群聊中可用的命令！/ This command only works in groups!")
            .build()
    }

    /// 仅私聊命令 — 演示 scope = "private"
    #[command(name = "secret", description = "仅私聊悄悄话 / Private whisper only", category = "示例", scope = "private")]
    fn secret(_req: &CommandRequest) -> CommandResponse {
        CommandResponse::text("🤫 这是一条仅在私聊中可用的秘密消息！/ This is a private-only secret!")
    }

    /// 主动发送 — 演示 BotApi::send_group_msg 和 SendBuilder
    /// 用法: notify <group_id> <内容>
    #[command(name = "notify", description = "向指定群发送通知 / Send notification to a group", category = "示例", role = "admin")]
    fn notify(req: &CommandRequest) -> CommandResponse {
        let args = req.args.as_str().trim();
        let parts: Vec<&str> = args.splitn(2, ' ').collect();

        if parts.len() < 2 || parts[0].is_empty() || parts[1].is_empty() {
            return CommandResponse::text("用法：notify <group_id> <内容>\nUsage: notify <group_id> <message>");
        }

        let target_group = parts[0];
        let content = parts[1];

        // 简单文本发送
        BotApi::send_group_msg(target_group, &format!("[通知] {content}"));

        // 流式构建富媒体发送到发送者私聊
        let sender = req.sender_id.as_str();
        SendBuilder::private(sender)
            .text("✅ 你的通知已发送到群 ")
            .text(target_group)
            .text("：")
            .text(content)
            .send();

        CommandResponse::text(&format!("通知已发送到群 {target_group}！"))
    }

    /// 帮助菜单 — 纯静态文本
    #[command(name = "example-help", description = "示例插件帮助 / Example plugin help", aliases = "示例帮助", category = "示例")]
    fn help(_req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(
            "╭──── 示例插件 v0.1.0 ────╮\n\
             │ greet [内容]    打招呼    │\n\
             │ time            显示时间  │\n\
             │ echo <内容>     复读消息  │\n\
             │ info            请求详情  │\n\
             │ example-help    本帮助    │\n\
             ╰────────────────────────╯\n\
             \n\
             别名：hi / hello / 你好 / 时间 / 复读 / say / debug / 调试 / 示例帮助"
        )
    }

    // ── 拦截器 ──────────────────────────────────────────────────────────

    /// 消息预处理拦截器 — 演示 #[pre_handle]
    /// 记录所有收到的消息，始终放行。
    #[pre_handle]
    fn on_pre_handle(req: &InterceptorRequest) -> InterceptorResponse {
        let sender = req.sender_id.as_str();
        let group = req.group_id.as_str();
        let text = req.message_text.as_str();
        let ctx = if group.is_empty() { "private" } else { group };

        eprintln!(
            "[dynamic-example] pre_handle: sender={}, ctx={}, text={:?}",
            sender, ctx, text
        );

        // Allow all messages
        InterceptorResponse::allow()
    }

    // ── 事件路由 ──────────────────────────────────────────────────────────

    /// 戳一戳事件 — 演示 #[route] 事件路由
    #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
    fn on_poke(req: &NoticeRequest) -> NoticeResponse {
        let raw: serde_json::Value = serde_json::from_str(req.raw_event_json.as_str())
            .unwrap_or_default();

        let target_id = raw.get("target_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let sender_id = raw.get("user_id").and_then(|v| v.as_i64()).unwrap_or(0);
        let route = req.route.as_str();

        let text = format!(
            "👆 戳一戳 [{route}]\n\
             ├ 发起者: {sender_id}\n\
             └ 目标: {target_id}"
        );

        let segments = serde_json::json!([
            { "type": "text", "data": { "text": text } },
            { "type": "face", "data": { "id": "181" } }
        ]);

        NoticeResponse {
            action: DynamicActionResponse::rich_reply(&segments.to_string()),
        }
    }
}
