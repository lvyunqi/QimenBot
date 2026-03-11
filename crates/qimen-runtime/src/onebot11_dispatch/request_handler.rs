use async_trait::async_trait;
use serde_json::Value;

use super::{
    OneBotSystemDispatchSignal, OneBotSystemEventHandler, RequestRoute, SystemEventContext,
    field_string,
};
use super::policy::{RequestDecision, decision_label, evaluate_friend_request, evaluate_group_invite};

pub struct LoggingRequestHandler;

#[async_trait]
impl OneBotSystemEventHandler for LoggingRequestHandler {
    async fn on_request(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &RequestRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        handle_request(ctx.bot_id, route, ctx.payload);
        None
    }
}

pub struct AutoApproveRequestHandler;

#[async_trait]
impl OneBotSystemEventHandler for AutoApproveRequestHandler {
    async fn on_request(
        &self,
        ctx: &SystemEventContext<'_>,
        route: &RequestRoute,
    ) -> Option<OneBotSystemDispatchSignal> {
        match route {
            RequestRoute::Friend => {
                if ctx.auto_approve_friend_requests {
                    let decision = evaluate_friend_request(ctx);
                    tracing::info!(
                        bot_id = %ctx.bot_id,
                        user_id = %field_string(ctx.payload, "user_id"),
                        flag = %field_string(ctx.payload, "flag"),
                        decision = %decision_label(&decision),
                        "auto-approve handler evaluated friend request"
                    );
                    return match decision {
                        RequestDecision::Approve { remark } => {
                            Some(OneBotSystemDispatchSignal::AutoApproveFriend {
                                flag: field_string(ctx.payload, "flag"),
                                remark,
                            })
                        }
                        RequestDecision::Reject { reason } => {
                            Some(OneBotSystemDispatchSignal::AutoRejectFriend {
                                flag: field_string(ctx.payload, "flag"),
                                reason,
                            })
                        }
                        RequestDecision::Ignore => None,
                    };
                }
            }
            RequestRoute::GroupInvite => {
                if ctx.auto_approve_group_invites {
                    let decision = evaluate_group_invite(ctx);
                    tracing::info!(
                        bot_id = %ctx.bot_id,
                        group_id = %field_string(ctx.payload, "group_id"),
                        user_id = %field_string(ctx.payload, "user_id"),
                        flag = %field_string(ctx.payload, "flag"),
                        decision = %decision_label(&decision),
                        "auto-approve handler evaluated group invite request"
                    );
                    return match decision {
                        RequestDecision::Approve { .. } => {
                            Some(OneBotSystemDispatchSignal::AutoApproveGroupInvite {
                                flag: field_string(ctx.payload, "flag"),
                                sub_type: "invite".to_string(),
                            })
                        }
                        RequestDecision::Reject { reason } => {
                            Some(OneBotSystemDispatchSignal::AutoRejectGroupInvite {
                                flag: field_string(ctx.payload, "flag"),
                                sub_type: "invite".to_string(),
                                reason,
                            })
                        }
                        RequestDecision::Ignore => None,
                    };
                }
            }
            _ => {}
        }

        None
    }
}

pub fn handle_request(bot_id: &str, route: &RequestRoute, payload: &Value) {
    match route {
        RequestRoute::Friend => {
            tracing::info!(
                bot_id = %bot_id,
                user_id = %field_string(payload, "user_id"),
                flag = %field_string(payload, "flag"),
                comment = %field_string(payload, "comment"),
                "handled request.friend"
            );
        }
        RequestRoute::GroupAdd => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                flag = %field_string(payload, "flag"),
                comment = %field_string(payload, "comment"),
                "handled request.group.add"
            );
        }
        RequestRoute::GroupInvite => {
            tracing::info!(
                bot_id = %bot_id,
                group_id = %field_string(payload, "group_id"),
                user_id = %field_string(payload, "user_id"),
                flag = %field_string(payload, "flag"),
                comment = %field_string(payload, "comment"),
                "handled request.group.invite"
            );
        }
        RequestRoute::Unknown {
            request_type,
            sub_type,
        } => {
            tracing::info!(
                bot_id = %bot_id,
                request_type = %request_type,
                sub_type = %sub_type.as_deref().unwrap_or(""),
                "handled request.unknown"
            );
        }
    }
}
