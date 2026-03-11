use super::SystemEventContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestDecision {
    Approve { remark: Option<String> },
    Reject { reason: Option<String> },
    Ignore,
}

pub fn evaluate_friend_request(ctx: &SystemEventContext<'_>) -> RequestDecision {
    let user_id = field_string(ctx.payload, "user_id");
    let comment = field_string(ctx.payload, "comment");

    if contains_id(ctx.auto_approve_friend_request_user_blacklist, &user_id) {
        return RequestDecision::Ignore;
    }

    if matches_any_keyword(&comment, ctx.auto_reject_friend_request_comment_keywords) {
        return RequestDecision::Reject { reason: None };
    }

    if !ctx.auto_approve_friend_request_user_whitelist.is_empty()
        && !contains_id(ctx.auto_approve_friend_request_user_whitelist, &user_id)
    {
        return RequestDecision::Ignore;
    }

    if !ctx.auto_approve_friend_request_comment_keywords.is_empty()
        && !matches_any_keyword(&comment, ctx.auto_approve_friend_request_comment_keywords)
    {
        return RequestDecision::Ignore;
    }

    RequestDecision::Approve {
        remark: ctx
            .auto_approve_friend_request_remark
            .filter(|text| !text.trim().is_empty())
            .map(ToOwned::to_owned),
    }
}

pub fn evaluate_group_invite(ctx: &SystemEventContext<'_>) -> RequestDecision {
    let user_id = field_string(ctx.payload, "user_id");
    let group_id = field_string(ctx.payload, "group_id");
    let comment = field_string(ctx.payload, "comment");

    if contains_id(ctx.auto_approve_group_invite_user_blacklist, &user_id)
        || contains_id(ctx.auto_approve_group_invite_group_blacklist, &group_id)
    {
        return RequestDecision::Ignore;
    }

    if matches_any_keyword(&comment, ctx.auto_reject_group_invite_comment_keywords) {
        return RequestDecision::Reject {
            reason: ctx
                .auto_reject_group_invite_reason
                .filter(|text| !text.trim().is_empty())
                .map(ToOwned::to_owned),
        };
    }

    if !ctx.auto_approve_group_invite_user_whitelist.is_empty()
        && !contains_id(ctx.auto_approve_group_invite_user_whitelist, &user_id)
    {
        return RequestDecision::Ignore;
    }

    if !ctx.auto_approve_group_invite_group_whitelist.is_empty()
        && !contains_id(ctx.auto_approve_group_invite_group_whitelist, &group_id)
    {
        return RequestDecision::Ignore;
    }

    if !ctx.auto_approve_group_invite_comment_keywords.is_empty()
        && !matches_any_keyword(&comment, ctx.auto_approve_group_invite_comment_keywords)
    {
        return RequestDecision::Ignore;
    }

    RequestDecision::Approve { remark: None }
}

pub fn render_notice_template(template: &str, ctx: &SystemEventContext<'_>) -> String {
    template
        .replace("{user_id}", &field_string(ctx.payload, "user_id"))
        .replace("{group_id}", &field_string(ctx.payload, "group_id"))
        .replace("{target_id}", &field_string(ctx.payload, "target_id"))
        .replace("{bot_id}", ctx.bot_id)
}

pub fn decision_label(decision: &RequestDecision) -> &'static str {
    match decision {
        RequestDecision::Approve { .. } => "approve",
        RequestDecision::Reject { .. } => "reject",
        RequestDecision::Ignore => "ignore",
    }
}

fn contains_id(list: &[String], target: &str) -> bool {
    list.iter().any(|item| item == target)
}

fn matches_any_keyword(comment: &str, keywords: &[String]) -> bool {
    let lowered = comment.to_lowercase();
    keywords
        .iter()
        .filter(|keyword| !keyword.trim().is_empty())
        .any(|keyword| lowered.contains(&keyword.to_lowercase()))
}

fn field_string(payload: &serde_json::Value, field: &str) -> String {
    payload
        .get(field)
        .map(|value| match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::Bool(flag) => flag.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{
        evaluate_friend_request, evaluate_group_invite, render_notice_template, RequestDecision,
    };
    use crate::onebot11_dispatch::SystemEventContext;
    use async_trait::async_trait;
    use qimen_error::{QimenError, Result};
    use qimen_message::Message;
    use qimen_plugin_api::{OwnedTaskFuture, RuntimeBotContext, TaskHandle};
    use qimen_protocol_core::{
        ActionStatus, CapabilitySet, NormalizedActionRequest, NormalizedActionResponse,
        NormalizedEvent, ProtocolId,
    };

    struct TestRuntimeBotContext;

    #[async_trait]
    impl RuntimeBotContext for TestRuntimeBotContext {
        fn bot_instance(&self) -> &str {
            "qq-main"
        }

        fn protocol(&self) -> ProtocolId {
            ProtocolId::OneBot11
        }

        fn capabilities(&self) -> &CapabilitySet {
            static CAPABILITIES: std::sync::OnceLock<CapabilitySet> = std::sync::OnceLock::new();
            CAPABILITIES.get_or_init(CapabilitySet::default)
        }

        async fn send_action(&self, _req: NormalizedActionRequest) -> Result<NormalizedActionResponse> {
            Err(QimenError::Runtime("test runtime does not send actions".to_string()))
        }

        async fn reply(&self, _event: &NormalizedEvent, _message: Message) -> Result<NormalizedActionResponse> {
            Ok(NormalizedActionResponse {
                protocol: ProtocolId::OneBot11,
                bot_instance: "qq-main".to_string(),
                status: ActionStatus::Ok,
                retcode: 0,
                data: serde_json::Value::Null,
                echo: None,
                latency_ms: 0,
                raw_json: serde_json::json!({
                    "status": "ok",
                    "retcode": 0,
                    "data": null
                }),
            })
        }

        fn spawn_owned(&self, name: &str, _fut: OwnedTaskFuture) -> TaskHandle {
            TaskHandle {
                name: name.to_string(),
            }
        }
    }

    static TEST_RUNTIME: TestRuntimeBotContext = TestRuntimeBotContext;

    fn base_ctx<'a>(payload: &'a serde_json::Value) -> SystemEventContext<'a> {
        SystemEventContext {
            bot_id: "qq-main",
            payload,
            runtime: &TEST_RUNTIME,
            auto_approve_friend_requests: true,
            auto_approve_group_invites: true,
            auto_reply_poke_enabled: true,
            auto_reply_poke_message: Some("你好 {user_id} 来自 {group_id}"),
            auto_approve_friend_request_user_whitelist: &[],
            auto_approve_friend_request_user_blacklist: &[],
            auto_approve_friend_request_comment_keywords: &[],
            auto_reject_friend_request_comment_keywords: &[],
            auto_approve_friend_request_remark: Some("自动通过"),
            auto_approve_group_invite_user_whitelist: &[],
            auto_approve_group_invite_user_blacklist: &[],
            auto_approve_group_invite_group_whitelist: &[],
            auto_approve_group_invite_group_blacklist: &[],
            auto_approve_group_invite_comment_keywords: &[],
            auto_reject_group_invite_comment_keywords: &[],
            auto_reject_group_invite_reason: Some("拒绝原因"),
        }
    }

    #[test]
    fn friend_policy_can_approve_with_remark() {
        let payload = serde_json::json!({"user_id": "10001", "comment": "你好"});
        let decision = evaluate_friend_request(&base_ctx(&payload));
        assert_eq!(
            decision,
            RequestDecision::Approve {
                remark: Some("自动通过".to_string())
            }
        );
    }

    #[test]
    fn group_policy_can_reject_on_keyword() {
        let payload =
            serde_json::json!({"user_id": "10001", "group_id": "20002", "comment": "广告合作"});
        let reject_keywords = vec!["广告".to_string()];
        let ctx = SystemEventContext {
            auto_reject_group_invite_comment_keywords: &reject_keywords,
            ..base_ctx(&payload)
        };

        let decision = evaluate_group_invite(&ctx);
        assert_eq!(
            decision,
            RequestDecision::Reject {
                reason: Some("拒绝原因".to_string())
            }
        );
    }

    #[test]
    fn notice_template_renders_variables() {
        let payload =
            serde_json::json!({"user_id": "10001", "group_id": "20002", "target_id": "30003"});
        let rendered = render_notice_template(
            "hi {user_id} in {group_id} -> {target_id} ({bot_id})",
            &base_ctx(&payload),
        );
        assert_eq!(rendered, "hi 10001 in 20002 -> 30003 (qq-main)");
    }
}
