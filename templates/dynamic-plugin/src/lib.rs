//! QimenBot API 0.4 dynamic plugin template.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use abi_stable_host_api::{
    BotApi, CommandRequest, CommandResponse, PluginInitConfig, PluginInitResult,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

static STOP_WORKER: AtomicBool = AtomicBool::new(false);
static WORKER: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

#[dynamic_plugin(id = "{{name}}", version = "0.1.0", api = "0.4")]
mod plugin {
    use super::*;

    /// Optional configuration:
    /// background_push = { bot_id = "qq-main", group_id = "123", interval_secs = 60 }
    #[init]
    fn init(config: PluginInitConfig) -> PluginInitResult {
        let Ok(root) = serde_json::from_str::<serde_json::Value>(config.config_json.as_str()) else {
            return PluginInitResult::ok();
        };
        let Some(push) = root.get("background_push") else {
            return PluginInitResult::ok();
        };
        let Some(bot_id) = push.get("bot_id").and_then(serde_json::Value::as_str) else {
            return PluginInitResult::err("background_push.bot_id is required");
        };
        let Some(group_id) = push.get("group_id").and_then(serde_json::Value::as_str) else {
            return PluginInitResult::err("background_push.group_id is required");
        };
        let interval = Duration::from_secs(
            push.get("interval_secs")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(60)
                .max(1),
        );
        let bot_id = bot_id.to_string();
        let group_id = group_id.to_string();

        STOP_WORKER.store(false, Ordering::Release);
        let handle = thread::spawn(move || {
            while !STOP_WORKER.load(Ordering::Acquire) {
                let status = BotApi::for_bot(&bot_id)
                    .send_group_msg(&group_id, "Hello from a background plugin worker");
                eprintln!("[{{name}}] proactive enqueue status: {status:?}");
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
                PluginInitResult::err("worker lock is poisoned")
            }
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

    #[command(name = "hello", description = "Say hello", aliases = "hi")]
    fn hello(req: &CommandRequest) -> CommandResponse {
        CommandResponse::text(&format!("Hello, {}!", req.sender_id.as_str()))
    }
}
