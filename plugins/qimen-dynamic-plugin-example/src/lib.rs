//! QimenBot dynamic plugin example using API 0.6.
//!
//! This independent cdylib demonstrates commands, lifecycle hooks, interceptors,
//! system-event routes, HTTP webhooks, schema-driven online configuration, the
//! legacy callback-flush send path, and real-time sends from a background thread.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use abi_stable_host_api::{
    BotApi, CommandRequest, CommandResponse, DynamicActionResponse, InterceptorRequest,
    InterceptorResponse, NoticeRequest, NoticeResponse, PluginConfigRequest, PluginConfigResult,
    PluginInitConfig, PluginInitResult, SendBuilder, SendEnqueueStatus, WebhookRequest,
    WebhookResponse,
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

fn parse_config(config_json: &str) -> Result<serde_json::Value, String> {
    if config_json.trim().is_empty() {
        Ok(serde_json::json!({}))
    } else {
        serde_json::from_str(config_json).map_err(|error| format!("配置 JSON 无效：{error}"))
    }
}

fn parse_background_push(root: &serde_json::Value) -> Result<Option<BackgroundPushConfig>, String> {
    let Some(push) = root.get("background_push") else {
        return Ok(None);
    };
    if !push
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let selector = push
        .get("selector")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("account");
    let bot = match selector {
        "account" => BotSelector::Account(required_string(push, "account_id")?),
        "instance" => BotSelector::Id(required_string(push, "bot_id")?),
        _ => return Err("background_push.selector 必须是 account 或 instance".to_string()),
    };
    let kind = required_string(push, "kind")?;
    let target_id = required_string(push, "target_id")?;

    Ok(Some(BackgroundPushConfig {
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
            .unwrap_or("API 0.6 background push")
            .to_string(),
        interval: Duration::from_secs(
            push.get("interval_secs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(60)
                .max(1),
        ),
    }))
}

fn required_string(value: &serde_json::Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("background_push.{key} 不能为空"))
}

fn validate_config(config_json: &str) -> Result<serde_json::Value, String> {
    let root = parse_config(config_json)?;
    let _ = parse_background_push(&root)?;
    let mut names = BTreeSet::new();
    for connection in root
        .get("connections")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(name) = connection.get("name").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !names.insert(name) {
            return Err(format!("连接名称 '{name}' 不能重复"));
        }
    }
    Ok(root)
}

fn stop_background_worker() -> Result<(), String> {
    STOP_BACKGROUND.store(true, Ordering::Release);
    let handle = BACKGROUND_THREAD
        .lock()
        .map_err(|_| "background worker lock is poisoned".to_string())?
        .take();
    if let Some(handle) = handle {
        handle.thread().unpark();
        handle
            .join()
            .map_err(|_| "background worker panicked while stopping".to_string())?;
    }
    Ok(())
}

fn apply_background_config(config_json: &str) -> Result<(), String> {
    let root = validate_config(config_json)?;
    let push = parse_background_push(&root)?;
    stop_background_worker()?;
    let Some(push) = push else {
        return Ok(());
    };

    let mut slot = BACKGROUND_THREAD
        .lock()
        .map_err(|_| "background worker lock is poisoned".to_string())?;
    STOP_BACKGROUND.store(false, Ordering::Release);
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
    *slot = Some(handle);
    Ok(())
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

#[dynamic_plugin(
    id = "dynamic-example",
    version = "0.1.0",
    api = "0.6",
    config_schema = "../config.schema.json",
    config_ui = "../config.ui.json",
    config_version = 1,
    config_apply = "live"
)]
mod example {
    use super::*;

    /// Load optional background_push configuration and start a real-time worker.
    #[init]
    fn on_init(config: PluginInitConfig) -> PluginInitResult {
        match apply_background_config(config.config_json.as_str()) {
            Ok(()) => PluginInitResult::ok(),
            Err(error) => PluginInitResult::err(&error),
        }
    }

    #[validate_config]
    fn on_validate_config(request: &PluginConfigRequest) -> PluginConfigResult {
        match validate_config(request.config_json.as_str()) {
            Ok(_) => PluginConfigResult::ok(),
            Err(error) => PluginConfigResult::err(&error),
        }
    }

    #[config_change]
    fn on_config_change(request: &PluginConfigRequest) -> PluginConfigResult {
        match apply_background_config(request.config_json.as_str()) {
            Ok(()) => PluginConfigResult::ok(),
            Err(error) => PluginConfigResult::err(&error),
        }
    }

    /// Stop and join the plugin worker before Host API unbind and library unload.
    #[shutdown]
    fn on_shutdown() {
        let _ = stop_background_worker();
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
        description = "Send immediately through API 0.6",
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
