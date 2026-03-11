// ── 系统事件处理示例 / System Event Handling Examples ──
//
// 展示 #[notice], #[request], #[meta] 宏处理各类系统事件。
// Demonstrates #[notice], #[request], #[meta] macros for system events.

use qimen_plugin_api::prelude::*;

#[module(id = "example-events", version = "0.1.0",
         name = "Event Demo / 事件演示",
         description = "System event handling examples / 系统事件处理示例")]
#[commands]
impl EventDemoModule {
    // ── Poke 戳一戳 ──────────────────────────────────────────────────────
    // 展示 is_poke_self() 判断 + 不同回复
    // Demonstrates is_poke_self() check + different replies

    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        // 通过原始 JSON 判断是否戳了自己
        // Check if the bot itself was poked via raw JSON
        let is_self = {
            let target = ctx.event.get("target_id").and_then(|v| v.as_i64());
            let self_id = ctx.event.get("self_id").and_then(|v| v.as_i64());
            matches!((target, self_id), (Some(t), Some(s)) if t == s)
        };

        if is_self {
            SystemPluginSignal::Reply(Message::text(
                "Don't poke me! / 别戳我！",
            ))
        } else {
            SystemPluginSignal::Continue
        }
    }

    // ── 新成员入群 / New member joined ───────────────────────────────────
    // 展示使用 SystemPluginContext.onebot_actions() 发送欢迎消息
    // Demonstrates SystemPluginContext.onebot_actions() for welcome messages

    #[notice(GroupIncreaseApprove, GroupIncreaseInvite)]
    async fn on_member_join(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let user_id = ctx.event.get("user_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let group_id = ctx.event.get("group_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        if user_id == 0 || group_id == 0 {
            return SystemPluginSignal::Continue;
        }

        // 构建欢迎消息，@新成员 / Build welcome message, @new member
        let welcome = Message::builder()
            .text("Welcome / 欢迎 ")
            .at(user_id.to_string())
            .text(" to the group! / 加入本群！")
            .build();

        // 使用 onebot_actions 发送群消息
        // Use onebot_actions to send group message
        let actions = ctx.onebot_actions();
        let _ = actions.send_group_msg(group_id, welcome).await;

        SystemPluginSignal::Continue
    }

    // ── 消息撤回通知 / Message recall notification ───────────────────────

    #[notice(GroupRecall)]
    async fn on_recall(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let operator = ctx.event.get("operator_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let user = ctx.event.get("user_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let msg_id = ctx.event.get("message_id")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let text = if operator == user {
            format!("User {user} recalled message {msg_id} / 用户 {user} 撤回了消息 {msg_id}")
        } else {
            format!(
                "Admin {operator} recalled message {msg_id} from user {user} / \
                 管理员 {operator} 撤回了用户 {user} 的消息 {msg_id}"
            )
        };

        SystemPluginSignal::Reply(Message::text(text))
    }

    // ── 好友请求自动同意 / Auto-approve friend requests ──────────────────
    // 展示 ApproveFriend 信号
    // Demonstrates ApproveFriend signal

    #[request(Friend)]
    async fn on_friend_request(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event.get("flag")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let comment = ctx.event.get("comment")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        tracing::info!(
            flag = flag.as_str(),
            comment,
            "Auto-approving friend request / 自动同意好友请求"
        );

        SystemPluginSignal::ApproveFriend {
            flag,
            remark: None, // 不设置备注 / no remark
        }
    }

    // ── 群邀请处理 / Group invite handling ────────────────────────────────
    // 展示 ApproveGroupInvite 信号
    // Demonstrates ApproveGroupInvite signal

    #[request(GroupInvite)]
    async fn on_group_invite(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let flag = ctx.event.get("flag")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let sub_type = ctx.event.get("sub_type")
            .and_then(|v| v.as_str())
            .unwrap_or("invite")
            .to_string();

        tracing::info!(
            flag = flag.as_str(),
            sub_type = sub_type.as_str(),
            "Auto-approving group invite / 自动同意群邀请"
        );

        SystemPluginSignal::ApproveGroupInvite {
            flag,
            sub_type,
        }
    }

    // ── 心跳事件 / Heartbeat event ───────────────────────────────────────
    // 展示 #[meta] 宏处理元事件
    // Demonstrates #[meta] macro for meta events

    #[meta(Heartbeat)]
    async fn on_heartbeat(&self, ctx: &SystemPluginContext<'_>) -> SystemPluginSignal {
        let interval = ctx.event.get("interval")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        tracing::trace!(
            interval,
            "Heartbeat received / 收到心跳"
        );

        SystemPluginSignal::Continue
    }
}
