//! QimenBot dynamic plugin example (动态插件示例)
//!
//! This crate demonstrates the v0.2 FFI interface for building dynamic plugins
//! that compile to `.so` / `.dll` / `.dylib` and are loaded at runtime.
//!
//! 本 crate 演示 v0.2 FFI 接口，用于构建编译为动态库（.so/.dll/.dylib）的插件，
//! 在运行时由宿主程序加载。
//!
//! ## Build 编译
//!
//! ```bash
//! cargo build --release -p qimen-dynamic-plugin-example
//! ```
//!
//! The resulting shared library will be in `target/release/`.
//! 生成的动态库位于 `target/release/` 目录下。

use abi_stable::std_types::RString;
use abi_stable_host_api::{
    CommandDescriptorEntry, CommandRequest, CommandResponse, DynamicActionResponse,
    NoticeRequest, NoticeResponse, PluginDescriptor,
};

// ─── Plugin Descriptor 插件描述符 ────────────────────────────────────────────

/// Entry point called by the host to discover plugin metadata.
/// 宿主调用此函数以获取插件元数据（命令列表、事件路由等）。
///
/// This is the **only** required symbol. It returns a [`PluginDescriptor`]
/// describing the plugin ID, supported commands, and event routes.
///
/// 这是唯一必须导出的符号，返回 [`PluginDescriptor`] 描述插件 ID、支持的命令和事件路由。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qimen_plugin_descriptor() -> PluginDescriptor {
    PluginDescriptor::new("dynamic-example", "0.1.0")
        // ── Command: greet ──
        // Greets the sender with an optional custom message.
        // 向发送者打招呼，可附带自定义消息。
        .add_command_full(CommandDescriptorEntry {
            name: RString::from("greet"),
            description: RString::from("Greet the sender / 向发送者打招呼"),
            callback_symbol: RString::from("dynamic_example_greet"),
            aliases: RString::from("hi,hello"),
            category: RString::from("general"),
            required_role: RString::new(), // anyone 任何人可用
        })
        // ── Command: time ──
        // Returns the current Unix timestamp.
        // 返回当前 Unix 时间戳。
        .add_command(
            "time",
            "Show current Unix timestamp / 显示当前 Unix 时间戳",
            "dynamic_example_time",
        )
        // ── Notice route: poke events ──
        // Handle both group and private poke notifications.
        // 处理群聊和私聊的戳一戳通知。
        .add_route("notice", "GroupPoke,PrivatePoke", "dynamic_example_on_poke")
}

// ─── Command Callbacks 命令回调 ──────────────────────────────────────────────

/// Handle the `greet` command.
/// 处理 `greet` 命令。
///
/// Demonstrates:
/// - Reading `sender_id` from the request to personalise the greeting.
///   从请求中读取 `sender_id` 以个性化问候。
/// - Using `args` for an optional custom greeting message.
///   使用 `args` 作为可选的自定义问候语。
/// - Building a rich-media response with `segments_json` (text + face).
///   通过 `segments_json` 构建富媒体响应（文本 + 表情）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_greet(req: &CommandRequest) -> CommandResponse {
    let sender = req.sender_id.as_str();
    let args = req.args.as_str().trim();

    // Build the greeting text.
    // 构建问候文本。
    let greeting = if args.is_empty() {
        format!("Hello, [CQ:at,qq={sender}]! Welcome to QimenBot! 你好！欢迎使用 QimenBot！")
    } else {
        format!("[CQ:at,qq={sender}] {args}")
    };

    // Build rich-media segments: text + face(emoji id=1 is 😊 in OneBot11).
    // 构建富媒体消息段：文本 + 表情（OneBot11 中 face id=1 对应经典 QQ 表情）。
    let segments = serde_json::json!([
        { "type": "text", "data": { "text": greeting } },
        { "type": "face", "data": { "id": "1" } }
    ]);

    CommandResponse {
        action: DynamicActionResponse::rich_reply(&segments.to_string()),
    }
}

/// Handle the `time` command.
/// 处理 `time` 命令。
///
/// Returns the current Unix timestamp as a simple text reply.
/// 以纯文本返回当前 Unix 时间戳。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_time(_req: &CommandRequest) -> CommandResponse {
    // std::time is available in no_std-friendly fashion; no async runtime needed.
    // 使用标准库获取时间，无需异步运行时。
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    CommandResponse {
        action: DynamicActionResponse::text_reply(&format!(
            "Current Unix timestamp / 当前 Unix 时间戳: {now}"
        )),
    }
}

// ─── Notice Callback 事件回调 ─────────────────────────────────────────────────

/// Handle poke notice events (GroupPoke / PrivatePoke).
/// 处理戳一戳通知事件（群聊 / 私聊）。
///
/// Demonstrates:
/// - Parsing `raw_event_json` to extract the poke target.
///   解析 `raw_event_json` 以提取被戳目标。
/// - Replying with rich content (text + face).
///   使用富媒体内容回复（文本 + 表情）。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dynamic_example_on_poke(req: &NoticeRequest) -> NoticeResponse {
    // Try to parse the raw event to find who was poked.
    // 尝试解析原始事件以确定谁被戳了。
    let target_id = serde_json::from_str::<serde_json::Value>(req.raw_event_json.as_str())
        .ok()
        .and_then(|v| v.get("target_id")?.as_i64())
        .unwrap_or(0);

    let route = req.route.as_str();

    // Build a fun reply with text + face segment.
    // 构建趣味回复：文本 + 表情。
    let text = format!(
        "Poke detected on route [{route}]! Target: {target_id} 🫵\n\
         检测到戳一戳事件 [{route}]！目标: {target_id}"
    );

    let segments = serde_json::json!([
        { "type": "text", "data": { "text": text } },
        { "type": "face", "data": { "id": "181" } }
    ]);

    NoticeResponse {
        action: DynamicActionResponse::rich_reply(&segments.to_string()),
    }
}
