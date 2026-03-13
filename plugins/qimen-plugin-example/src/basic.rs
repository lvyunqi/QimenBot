// ── 基础命令示例 / Basic Command Examples ──
//
// 展示 #[module] + #[commands] 宏、参数、权限控制、信号等。
// Demonstrates #[module] + #[commands] macros, arguments, role control, signals.

use qimen_message::Segment;
use qimen_plugin_api::prelude::*;

#[module(id = "example-basic", version = "0.1.0",
         name = "Basic Commands / 基础命令",
         description = "Basic command examples / 基础命令示例")]
#[commands]
impl BasicModule {
    // ── /ping ────────────────────────────────────────────────────────────
    // 最简命令：无参数，直接返回 Message
    // Simplest command: no args, returns a Message directly

    #[command("Reply with pong / 回复 pong",
              examples = ["/ping"], category = "examples")]
    async fn ping(&self) -> Message {
        Message::text("pong!")
    }

    // ── /echo <text> ─────────────────────────────────────────────────────
    // 带参数命令 + 别名，支持富媒体（表情、图片等）
    // Command with arguments + alias, supports rich content (emoji, images, etc.)

    #[command("Echo back the given text / 回显文本",
              aliases = ["e"],
              examples = ["/echo hello", "/e world"],
              category = "examples")]
    async fn echo(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> Message {
        // 获取原始消息，提取命令名之后的所有消息段（包含表情、图片等）
        // Get original message and extract all segments after the command name
        // (including faces, images, etc.)
        if let Some(msg) = ctx.message() {
            let segments = &msg.segments;

            // 跳过 reply 段和命令前缀文本段，保留其余所有段
            // Skip reply segments and the command-prefix text segment, keep the rest
            let mut result_segments = Vec::new();
            let mut command_stripped = false;

            for seg in segments {
                // 跳过 reply 段 / skip reply segments
                if seg.kind == "reply" {
                    continue;
                }

                // 第一个 text 段包含命令名，需要去掉命令前缀
                // The first text segment contains the command name, strip it
                if !command_stripped && seg.kind == "text" {
                    command_stripped = true;
                    if let Some(text) = seg.get_text() {
                        // 去掉命令名（如 "/echo " 或 "e "）
                        // Remove command name (e.g. "/echo " or "e ")
                        let trimmed = text.trim_start();
                        let without_slash = trimmed.strip_prefix('/').unwrap_or(trimmed);
                        // 跳过命令名本身
                        let after_cmd = without_slash
                            .split_once(char::is_whitespace)
                            .map(|(_, rest)| rest)
                            .unwrap_or("");
                        if !after_cmd.is_empty() {
                            result_segments.push(Segment::text(after_cmd));
                        }
                    }
                    continue;
                }

                result_segments.push(seg.clone());
            }

            if !result_segments.is_empty() {
                return Message::from_segments(result_segments);
            }
        }

        // 回退到纯文本参数 / fallback to plain text args
        if !args.is_empty() {
            return Message::text(args.join(" "));
        }

        Message::text("(empty / 空)")
    }

    // ── /whoami ──────────────────────────────────────────────────────────
    // 展示 CommandPluginContext 的便捷方法
    // Demonstrates CommandPluginContext convenience methods

    #[command("Show your identity info / 显示你的身份信息",
              examples = ["/whoami"], category = "examples")]
    async fn whoami(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
        let sender = ctx.sender_id().unwrap_or("unknown");
        let nickname = ctx.event.sender_nickname().unwrap_or("unknown");
        let role = ctx.event.sender_role().unwrap_or("unknown");
        let scope = if ctx.is_group() {
            "group / 群聊"
        } else if ctx.is_private() {
            "private / 私聊"
        } else {
            "other / 其他"
        };

        CommandPluginSignal::Reply(Message::text(format!(
            "ID: {sender}\nNickname / 昵称: {nickname}\nRole / 角色: {role}\nScope / 场景: {scope}"
        )))
    }

    // ── /group-info ──────────────────────────────────────────────────────
    // 展示 OneBotActionClient 调用
    // Demonstrates OneBotActionClient API calls

    #[command("Show current group info / 显示当前群信息",
              aliases = ["gi"],
              examples = ["/group-info"],
              category = "examples")]
    async fn group_info(&self, ctx: &CommandPluginContext<'_>) -> CommandPluginSignal {
        let group_id = match ctx.group_id_i64() {
            Some(id) => id,
            // 非群聊则跳过 / skip if not in a group
            None => return CommandPluginSignal::Continue,
        };

        let actions = ctx.onebot_actions();
        let info = match actions.get_group_info(group_id, false).await {
            Ok(v) => v,
            Err(e) => {
                return CommandPluginSignal::Reply(Message::text(format!(
                    "Failed to get group info / 获取群信息失败: {e}"
                )));
            }
        };

        CommandPluginSignal::Reply(Message::text(format!(
            "Group / 群: {} ({})\nMembers / 成员: {}/{}",
            info.group_name,
            info.group_id,
            info.member_count
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
            info.max_member_count
                .map(|v| v.to_string())
                .unwrap_or_else(|| "?".into()),
        )))
    }

    // ── /ban <user_id> [seconds] ─────────────────────────────────────────
    // 展示权限控制 (role = "admin") + set_group_ban action
    // Demonstrates role-based access control + set_group_ban

    #[command("Ban a user in the group / 在群中禁言用户",
              role = "admin",
              examples = ["/ban 123456 60"],
              category = "admin")]
    async fn ban(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
        let group_id = match ctx.group_id_i64() {
            Some(id) => id,
            None => {
                return CommandPluginSignal::Reply(Message::text(
                    "This command only works in groups / 此命令仅在群聊中可用",
                ));
            }
        };

        // 解析参数 / parse arguments
        let user_id: i64 = match args.first().and_then(|s| s.parse().ok()) {
            Some(id) => id,
            None => {
                return CommandPluginSignal::Reply(Message::text(
                    "Usage / 用法: /ban <user_id> [seconds]",
                ));
            }
        };

        let duration: i64 = args
            .get(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(60); // 默认 60 秒 / default 60 seconds

        let actions = ctx.onebot_actions();
        match actions.set_group_ban(group_id, user_id, duration).await {
            Ok(()) => CommandPluginSignal::Reply(Message::text(format!(
                "Banned user {user_id} for {duration}s / 已禁言用户 {user_id} {duration}秒"
            ))),
            Err(e) => CommandPluginSignal::Reply(Message::text(format!(
                "Ban failed / 禁言失败: {e}"
            ))),
        }
    }

    // ── /group-only ──────────────────────────────────────────────────────
    // 展示 scope = "group"：仅在群聊中可用，私聊静默忽略
    // Demonstrates scope = "group": only available in group chats

    #[command("Only works in group chats / 仅群聊可用",
              examples = ["/group-only"], category = "examples", scope = "group")]
    async fn group_only(&self) -> Message {
        Message::text("This command only works in group chats! / 此命令仅在群聊中可用！")
    }

    // ── /private-only ───────────────────────────────────────────────────
    // 展示 scope = "private"：仅在私聊中可用，群聊静默忽略
    // Demonstrates scope = "private": only available in private chats

    #[command("Only works in private chats / 仅私聊可用",
              examples = ["/private-only"], category = "examples", scope = "private")]
    async fn private_only(&self) -> Message {
        Message::text("This command only works in private chats! / 此命令仅在私聊中可用！")
    }

    // ── /stop ────────────────────────────────────────────────────────────
    // 展示 Block 信号：终止插件链，不再执行后续插件
    // Demonstrates Block signal: stops the plugin chain

    #[command("Stop plugin chain / 终止插件链",
              examples = ["/stop"], category = "examples")]
    async fn stop(&self) -> CommandPluginSignal {
        // Block 会发送回复并阻止后续插件处理此命令
        // Block sends a reply and prevents subsequent plugins from handling this command
        CommandPluginSignal::Block(Message::text(
            "Plugin chain stopped here / 插件链在此终止",
        ))
    }
}
