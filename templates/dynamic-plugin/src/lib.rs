//! QimenBot 动态插件 API 0.6 模板。

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use abi_stable_host_api::{
    BotApi, CommandRequest, CommandResponse, PluginConfigRequest, PluginConfigResult,
    PluginInitConfig, PluginInitResult, SendEnqueueStatus, WebhookRequest, WebhookResponse,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

static STOP_WORKER: AtomicBool = AtomicBool::new(false);
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[derive(Clone)]
enum BotSelector {
    Instance(String),
    Account(String),
}

impl BotSelector {
    fn send_group_msg(&self, group_id: &str, message: &str) -> SendEnqueueStatus {
        match self {
            Self::Instance(bot_id) => BotApi::for_bot(bot_id).send_group_msg(group_id, message),
            Self::Account(account_id) => {
                BotApi::for_account(account_id).send_group_msg(group_id, message)
            }
        }
    }
}

fn parse_config(config_json: &str) -> Result<serde_json::Value, String> {
    if config_json.trim().is_empty() {
        Ok(serde_json::json!({}))
    } else {
        serde_json::from_str(config_json).map_err(|error| format!("配置 JSON 无效：{error}"))
    }
}

fn validate_config(root: &serde_json::Value) -> Result<(), String> {
    let Some(push) = root.get("background_push") else {
        return Ok(());
    };
    if !push
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(());
    }
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
    if !matches!((bot_id, account_id), (Some(_), None) | (None, Some(_))) {
        return Err("background_push 必须且只能配置 bot_id 或 account_id 其中一个".to_string());
    }
    Ok(())
}

#[dynamic_plugin(
    id = "{{name}}",
    version = "0.1.0",
    api = "0.6",
    config_schema = "../config.schema.json",
    config_ui = "../config.ui.json",
    config_version = 1,
    config_apply = "reload"
)]
mod plugin {
    use super::*;

    /// 可选配置：
    /// background_push = { account_id = "2733944636", group_id = "123", interval_secs = 60 }
    #[init]
    fn init(config: PluginInitConfig) -> PluginInitResult {
        let root = match parse_config(config.config_json.as_str()) {
            Ok(root) => root,
            Err(error) => return PluginInitResult::err(&error),
        };
        if let Err(error) = validate_config(&root) {
            return PluginInitResult::err(&error);
        }
        let Some(push) = root.get("background_push") else {
            return PluginInitResult::ok();
        };
        if !push
            .get("enabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            return PluginInitResult::ok();
        }

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
        let selector = match (bot_id, account_id) {
            (Some(bot_id), None) => BotSelector::Instance(bot_id.to_string()),
            (None, Some(account_id)) => BotSelector::Account(account_id.to_string()),
            _ => {
                return PluginInitResult::err(
                    "background_push 必须且只能配置 bot_id 或 account_id 其中一个",
                );
            }
        };

        let Some(group_id) = push
            .get("group_id")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return PluginInitResult::err("background_push.group_id 不能为空");
        };
        let group_id = group_id.to_string();
        let message = push
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("来自动态插件后台线程的消息")
            .to_string();
        let interval = Duration::from_secs(
            push.get("interval_secs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(60)
                .max(1),
        );

        STOP_WORKER.store(false, Ordering::Release);
        let handle = thread::spawn(move || {
            while !STOP_WORKER.load(Ordering::Acquire) {
                let status = selector.send_group_msg(&group_id, &message);
                eprintln!("[{{name}}] 主动发送入队状态: {status:?}");
                thread::park_timeout(interval);
            }
        });

        match WORKER.lock() {
            Ok(mut worker) => {
                *worker = Some(handle);
                PluginInitResult::ok()
            }
            Err(_) => {
                STOP_WORKER.store(true, Ordering::Release);
                handle.thread().unpark();
                let _ = handle.join();
                PluginInitResult::err("后台线程锁已损坏")
            }
        }
    }

    #[validate_config]
    fn validate_online_config(request: &PluginConfigRequest) -> PluginConfigResult {
        match parse_config(request.config_json.as_str()).and_then(|root| {
            validate_config(&root)?;
            Ok(root)
        }) {
            Ok(_) => PluginConfigResult::ok(),
            Err(error) => PluginConfigResult::err(&error),
        }
    }

    #[shutdown]
    fn shutdown() {
        STOP_WORKER.store(true, Ordering::Release);
        if let Ok(mut worker) = WORKER.lock()
            && let Some(handle) = worker.take()
        {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }

    #[command(
        name = "hello",
        description = "向发送者打招呼",
        aliases = "hi,你好",
        category = "general"
    )]
    fn hello(req: &CommandRequest) -> CommandResponse {
        let nickname = req.sender_nickname.as_str();
        let display = if nickname.is_empty() {
            req.sender_id.as_str()
        } else {
            nickname
        };
        CommandResponse::text(&format!("你好，{display}"))
    }

    /// 启用宿主 Webhook Gateway 后，对外路径为 /webhooks/{{name}}/events。
    #[webhook(method = "POST", path = "/events")]
    fn receive_event(req: &WebhookRequest) -> WebhookResponse {
        let response = serde_json::json!({
            "accepted": true,
            "bytes": req.body.len(),
        })
        .to_string();
        WebhookResponse::text(200, &response)
            .with_headers_json(r#"{"content-type":"application/json; charset=utf-8"}"#)
    }
}
