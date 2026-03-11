use async_trait::async_trait;
use serde_json::Value;

use super::{
    NoticeRoute, OneBotSystemDispatchSignal, OneBotSystemEventHandler, SystemEventContext,
    field_string, nested_field_string,
};
use super::policy::render_notice_template;

pub struct LoggingNoticeHandler;

#[async_trait]
impl OneBotSystemEventHandler for LoggingNoticeHandler {
    async fn on_notice(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &NoticeRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        handle_notice(ctx, route);
        None
    }
}

pub struct AutoReplyPokeHandler;

#[async_trait]
impl OneBotSystemEventHandler for AutoReplyPokeHandler {
    async fn on_notice(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &NoticeRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        if !ctx.auto_reply_poke_enabled {
            return None;
        }

        if matches!(route, NoticeRoute::GroupPoke | NoticeRoute::PrivatePoke) {
            // Only reply when the bot itself is the poke target
            let self_id = ctx.payload.get("self_id").and_then(|v| v.as_i64());
            let target_id = ctx.payload.get("target_id").and_then(|v| v.as_i64());
            if self_id.is_none() || target_id.is_none() || self_id != target_id {
                return None;
            }

            let template = ctx
                .auto_reply_poke_message
                .filter(|text| !text.trim().is_empty())
                .unwrap_or("别戳了，{user_id}，我在忙。 group={group_id}");
            let message = render_notice_template(template, ctx);
            tracing::info!(bot_id = %ctx.bot_id, message = %message, "auto-reply poke handler emitted reply");
            return Some(OneBotSystemDispatchSignal::NoticeReply { message });
        }

        None
    }
}

pub fn handle_notice(ctx: &SystemEventContext<'_>, route: &NoticeRoute) {
    let bot_id = ctx.bot_id;
    let payload = ctx.payload;

    match route {
        NoticeRoute::GroupUpload => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                file_name = %nested_field_string(payload, "file", "name"),
                "handled notice.group_upload"
            );
        }
        NoticeRoute::GroupAdminSet => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                "handled notice.group_admin.set"
            );
        }
        NoticeRoute::GroupAdminUnset => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                "handled notice.group_admin.unset"
            );
        }
        NoticeRoute::GroupDecreaseLeave => {
            log_group_change(bot_id, payload, "handled notice.group_decrease.leave")
        }
        NoticeRoute::GroupDecreaseKick => {
            log_group_change(bot_id, payload, "handled notice.group_decrease.kick")
        }
        NoticeRoute::GroupDecreaseKickMe => {
            log_group_change(bot_id, payload, "handled notice.group_decrease.kick_me")
        }
        NoticeRoute::GroupDecreaseOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled notice.group_decrease.other");
        }
        NoticeRoute::GroupIncreaseApprove => {
            log_group_change(bot_id, payload, "handled notice.group_increase.approve")
        }
        NoticeRoute::GroupIncreaseInvite => {
            log_group_change(bot_id, payload, "handled notice.group_increase.invite")
        }
        NoticeRoute::GroupIncreaseOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled notice.group_increase.other");
        }
        NoticeRoute::GroupBanBan => log_group_ban(bot_id, payload, "handled notice.group_ban.ban"),
        NoticeRoute::GroupBanLiftBan => {
            log_group_ban(bot_id, payload, "handled notice.group_ban.lift_ban")
        }
        NoticeRoute::GroupBanOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled notice.group_ban.other");
        }
        NoticeRoute::FriendAdd => {
            tracing::info!(bot_id = %bot_id, user_id = %field_string(payload, "user_id"), "handled notice.friend_add");
        }
        NoticeRoute::GroupRecall => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                message_id = %field_string(payload, "message_id"),
                "handled notice.group_recall"
            );
        }
        NoticeRoute::FriendRecall => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                message_id = %field_string(payload, "message_id"),
                "handled notice.friend_recall"
            );
        }
        NoticeRoute::GroupPoke => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                target_id = %field_string(payload, "target_id"),
                "handled notice.notify.poke (group)"
            );
        }
        NoticeRoute::PrivatePoke => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                target_id = %field_string(payload, "target_id"),
                "handled notice.notify.poke (private)"
            );
        }
        NoticeRoute::NotifyLuckyKing => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                target_id = %field_string(payload, "target_id"),
                "handled notice.notify.lucky_king"
            );
        }
        NoticeRoute::NotifyHonor(honor_type) => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                honor_type = %honor_type.as_deref().unwrap_or("unknown"),
                "handled notice.notify.honor"
            );
        }
        NoticeRoute::NotifyOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled notice.notify.other");
        }
        NoticeRoute::GroupCard => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                card_new = %field_string(payload, "card_new"),
                "handled notice.group_card"
            );
        }
        NoticeRoute::OfflineFile => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                file_name = %nested_field_string(payload, "file", "name"),
                "handled notice.offline_file"
            );
        }
        NoticeRoute::ClientStatus => {
            tracing::info!(
                bot_id = %bot_id,
                client = %nested_field_string(payload, "client", "app_id"),
                online = %field_string(payload, "online"),
                "handled notice.client_status"
            );
        }
        NoticeRoute::EssenceAdd => {
            tracing::info!(bot_id = %bot_id, group_id = %field_string(payload, "group_id"), message_id = %field_string(payload, "message_id"), "handled notice.essence.add");
        }
        NoticeRoute::EssenceDelete => {
            tracing::info!(bot_id = %bot_id, group_id = %field_string(payload, "group_id"), message_id = %field_string(payload, "message_id"), "handled notice.essence.delete");
        }
        NoticeRoute::EssenceOther(sub_type) => {
            tracing::info!(bot_id = %bot_id, sub_type = %sub_type, "handled notice.essence.other");
        }
        NoticeRoute::ChannelCreated => {
            tracing::info!(
                bot_id = %bot_id,
                guild_id = %field_string(payload, "guild_id"),
                channel_id = %field_string(payload, "channel_id"),
                "handled notice.channel_created"
            );
        }
        NoticeRoute::ChannelDestroyed => {
            tracing::info!(
                bot_id = %bot_id,
                guild_id = %field_string(payload, "guild_id"),
                channel_id = %field_string(payload, "channel_id"),
                "handled notice.channel_destroyed"
            );
        }
        NoticeRoute::ChannelUpdated => {
            tracing::info!(
                bot_id = %bot_id,
                guild_id = %field_string(payload, "guild_id"),
                channel_id = %field_string(payload, "channel_id"),
                "handled notice.channel_updated"
            );
        }
        NoticeRoute::GuildMessageReactionsUpdated => {
            tracing::info!(
                bot_id = %bot_id,
                guild_id = %field_string(payload, "guild_id"),
                channel_id = %field_string(payload, "channel_id"),
                message_id = %field_string(payload, "message_id"),
                "handled notice.message_reactions_updated"
            );
        }
        NoticeRoute::GroupReaction => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                message_id = %field_string(payload, "message_id"),
                user_id = %field_string(payload, "user_id"),
                "handled notice.group_msg_emoji_like"
            );
        }
        NoticeRoute::MessageEmojiLike => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                message_id = %field_string(payload, "message_id"),
                "handled notice.notify.msg_emoji_like"
            );
        }
        NoticeRoute::Unknown(notice_type) => {
            tracing::info!(bot_id = %bot_id, notice_type = %notice_type, "handled notice.unknown");
        }
    }
}

fn log_group_change(bot_id: &str, payload: &Value, message: &str) {
    tracing::info!(
        bot_id = %bot_id,
        group_id = %field_string(payload, "group_id"),
        user_id = %field_string(payload, "user_id"),
        operator_id = %field_string(payload, "operator_id"),
        "{message}"
    );
}

fn log_group_ban(bot_id: &str, payload: &Value, message: &str) {
    tracing::info!(
        bot_id = %bot_id,
        group_id = %field_string(payload, "group_id"),
        user_id = %field_string(payload, "user_id"),
        operator_id = %field_string(payload, "operator_id"),
        duration = %field_string(payload, "duration"),
        "{message}"
    );
}
