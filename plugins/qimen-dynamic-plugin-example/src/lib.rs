//! QimenBot dynamic plugin example using API 0.5.
//!
//! This independent cdylib demonstrates commands, lifecycle hooks, interceptors,
//! system-event routes, HTTP webhooks, the legacy callback-flush send path, and
//! real-time sends from a background thread.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use abi_stable_host_api::{
    BotApi, CommandRequest, CommandResponse, DynamicActionResponse, InterceptorRequest,
    InterceptorResponse, NoticeRequest, NoticeResponse, PluginInitConfig, PluginInitResult,
    SendBuilder, SendEnqueueStatus, WebhookRequest, WebhookResponse,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

static STOP_BACKGROUND: AtomicBool = AtomicBool::new(false);
static BACKGROUND_THREAD: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[derive(Clone)]
struct BackgroundPushConfig {
    bot: BotSelector,
    kind: String,
    target_id: String,
    guild_id: Option<String>,
    message: String,
    interval: Duration,
}

#[derive(Clone)]
enum BotSelector {
    Id(String),
    Account(String),
}

fn parse_background_push(config_json: &str) -> Option<BackgroundPushConfig> {
    let root: serde_json::Value = serde_json::from_str(config_json).ok()?;
    let push = root.get("background_push")?;
    let bot_id = push
        .get("bot_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let account_id = push
        .get("account_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let bot = match (bot_id, account_id) {
        (Some(bot_id), None) => BotSelector::Id(bot_id.to_string()),
        (None, Some(account_id)) => BotSelector::Account(account_id.to_string()),
        _ => return None,
    };
    let kind = push.get("kind")?.as_str()?.trim().to_string();
    let target_id = push.get("target_id")?.as_str()?.trim().to_string();
    if target_id.is_empty() {
        return None;
    }

    Some(BackgroundPushConfig {
        bot,
        kind,
        target_id,
        guild_id: push
            .get("guild_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        message: push
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("API 0.5 background push")
            .to_string(),
        interval: Duration::from_secs(
            push.get("interval_secs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(60)
                .max(1),
        ),
    })
}

fn try_send_target(
    bot: &BotSelector,
    kind: &str,
    target_id: &str,
    guild_id: Option<&str>,
    message: &str,
) -> SendEnqueueStatus {
    match kind {
        "private" => match bot {
            BotSelector::Id(bot_id) => BotApi::for_bot(bot_id).send_private_msg(target_id, message),
            BotSelector::Account(account_id) => {
                BotApi::for_account(account_id).send_private_msg(target_id, message)
            }
        },
        "group" => match bot {
            BotSelector::Id(bot_id) => BotApi::for_bot(bot_id).send_group_msg(target_id, message),
            BotSelector::Account(account_id) => {
                BotApi::for_account(account_id).send_group_msg(target_id, message)
            }
        },
        "channel" => {
            let builder = select_bot(SendBuilder::channel(target_id), bot).text(message);
            match guild_id {
                Some(guild_id) => builder.guild_id(guild_id).try_send(),
                None => builder.try_send(),
            }
        }
        "channel_private" => {
            let builder = select_bot(SendBuilder::channel_private(target_id), bot).text(message);
            match guild_id {
                Some(guild_id) => builder.guild_id(guild_id).try_send(),
                None => builder.try_send(),
            }
        }
        _ => SendEnqueueStatus::InvalidRequest,
    }
}

fn select_bot(builder: SendBuilder, bot: &BotSelector) -> SendBuilder {
    match bot {
        BotSelector::Id(bot_id) => builder.bot(bot_id),
        BotSelector::Account(account_id) => builder.bot_account(account_id),
    }
}

fn parse_command_bot_selector(value: &str) -> Option<BotSelector> {
    let value = value.trim();
    if let Some(account_id) = value.strip_prefix("account:") {
        let account_id = account_id.trim();
        (!account_id.is_empty()).then(|| BotSelector::Account(account_id.to_string()))
    } else {
        (!value.is_empty()).then(|| BotSelector::Id(value.to_string()))
    }
}

#[dynamic_plugin(id = "dynamic-example", version = "0.1.0", api = "0.5")]
mod example {
    use super::*;

    /// Load optional background_push configuration and start a real-time worker.
    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult {
        STOP_BACKGROUND.store(false, Ordering::Release);
        let Some(push) = parse_background_push(config.config_json.as_str()) else {
            eprintln!("[dynamic-example] background push is not configured");
            return PluginInitResult::ok();
        };

        let handle = thread::spawn(move || {
            while !STOP_BACKGROUND.load(Ordering::Acquire) {
                let status = try_send_target(
                    &push.bot,
                    &push.kind,
                    &push.target_id,
                    push.guild_id.as_deref(),
                    &push.message,
                );
                eprintln!("[dynamic-example] proactive enqueue status: {status:?}");
                thread::park_timeout(push.interval);
            }
        });

        match BACKGROUND_THREAD.lock() {
            Ok(mut slot) => {
                *slot = Some(handle);
                PluginInitResult::ok()
            }
            Err(_) => {
                STOP_BACKGROUND.store(true, Ordering::Release);
                handle.thread().unpark();
                let _ = handle.join();
                PluginInitResult::err("background worker lock is poisoned")
            }
        }
    }

    /// Stop and join the plugin worker before Host API unbind and library unload.
    #[shutdown]
    fn on_shutdown() {
        STOP_BACKGROUND.store(true, Ordering::Release);
        if let Ok(mut slot) = BACKGROUND_THREAD.lock()
            && let Some(handle) = slot.take()
        {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }

    #[command(
        name = "greet",
        description = "Greet the sender",
        aliases = "hi,hello",
        category = "example"
    )]
    fn greet(req: &CommandRequest) -> CommandResponse {
        let nickname = req.sender_nickname.as_str();
        let display = if nickname.is_empty() {
            req.sender_id.as_str()
        } else {
            nickname
        };
        CommandResponse::text(&format!("Hello, {display}!"))
    }

    /// Legacy API 0.1-0.3 compatible send; the host flushes it after this callback.
    #[command(
        name = "legacy-notify",
        description = "Queue a legacy group notification",
        category = "example",
        role = "admin"
    )]
    fn legacy_notify(req: &CommandRequest) -> CommandResponse {
        let mut parts = req.args.as_str().trim().splitn(2, ' ');
        let Some(group_id) = parts.next().filter(|value| !value.is_empty()) else {
            return CommandResponse::text("Usage: legacy-notify <group_id> <message>");
        };
        let Some(message) = parts.next().filter(|value| !value.is_empty()) else {
            return CommandResponse::text("Usage: legacy-notify <group_id> <message>");
        };

        BotApi::send_group_msg(group_id, message);
        CommandResponse::text("Legacy send queued for callback flush")
    }

    /// Real-time send with an explicit bot and protocol-neutral target.
    #[command(
        name = "proactive-send",
        description = "Send immediately through API 0.5",
        category = "example",
        role = "admin"
    )]
    fn proactive_send(req: &CommandRequest) -> CommandResponse {
        let parts: Vec<&str> = req.args.as_str().trim().splitn(5, ' ').collect();
        if parts.len() != 5 {
            return CommandResponse::text(
                "Usage: proactive-send <bot_id|account:QQ> <private|group|channel|channel_private> <target_id> <guild_id|-> <message>",
            );
        }

        let Some(bot) = parse_command_bot_selector(parts[0]) else {
            return CommandResponse::text("Bot selector cannot be empty");
        };
        let guild_id = (parts[3] != "-").then_some(parts[3]);
        let status = try_send_target(&bot, parts[1], parts[2], guild_id, parts[4]);
        CommandResponse::text(&format!("Host enqueue status: {status:?}"))
    }

    /// Receive a framework-hosted HTTP webhook at
    /// `/webhooks/dynamic-example/events` with the default gateway base path.
    #[webhook(method = "POST", path = "/events")]
    fn receive_event(req: &WebhookRequest) -> WebhookResponse {
        let payload = serde_json::json!({
            "accepted": true,
            "method": req.method.as_str(),
            "path": req.path.as_str(),
            "query": req.query.as_str(),
            "remote_addr": req.remote_addr.as_str(),
            "headers": serde_json::from_str::<serde_json::Value>(req.headers_json.as_str())
                .unwrap_or_else(|_| serde_json::json!({})),
            "body": String::from_utf8_lossy(req.body.as_slice()),
        })
        .to_string();

        WebhookResponse::text(200, &payload).with_headers_json(
            r#"{"content-type":"application/json; charset=utf-8","x-qimen-plugin":"dynamic-example"}"#,
        )
    }

    #[pre_handle]
    fn on_pre_handle(req: &InterceptorRequest) -> InterceptorResponse {
        eprintln!(
            "[dynamic-example] message sender={} text={:?}",
            req.sender_id.as_str(),
            req.message_text.as_str()
        );
        InterceptorResponse::allow()
    }

    #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
    fn on_poke(req: &NoticeRequest) -> NoticeResponse {
        NoticeResponse {
            action: DynamicActionResponse::text_reply(&format!(
                "Received routed notice: {}",
                req.route.as_str()
            )),
        }
    }
}
