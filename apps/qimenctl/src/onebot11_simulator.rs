use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use qimen_config::{AppConfig, BotConfig};
use qimen_error::{QimenError, Result};
use qimen_transport_ws::OneBot11ForwardWsClient;
use serde_json::{Value, json};

const DEFAULT_CONFIG_PATH: &str = "config/base.toml";
const DEFAULT_USER_ID: &str = "10000";
const DEFAULT_SELF_ID: &str = "10001";

#[derive(Debug, Clone)]
struct Options {
    config_path: String,
    bot_id: Option<String>,
    endpoint: Option<String>,
    access_token: Option<String>,
    message: Option<String>,
    raw_event: Option<PathBuf>,
    user_id: String,
    self_id: String,
    group_id: Option<String>,
    timeout: Duration,
    idle_timeout: Duration,
    send_lifecycle: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            config_path: DEFAULT_CONFIG_PATH.to_string(),
            bot_id: None,
            endpoint: None,
            access_token: None,
            message: None,
            raw_event: None,
            user_id: DEFAULT_USER_ID.to_string(),
            self_id: DEFAULT_SELF_ID.to_string(),
            group_id: None,
            timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_millis(750),
            send_lifecycle: true,
        }
    }
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            let argument = args[index].as_str();
            match argument {
                "--help" | "-h" => return Err(help_error()),
                "--no-lifecycle" => options.send_lifecycle = false,
                "--config" => options.config_path = take_value(args, &mut index, argument)?,
                "--bot" => options.bot_id = Some(take_value(args, &mut index, argument)?),
                "--endpoint" => options.endpoint = Some(take_value(args, &mut index, argument)?),
                "--access-token" => {
                    options.access_token = Some(take_value(args, &mut index, argument)?)
                }
                "--access-token-env" => {
                    let name = take_value(args, &mut index, argument)?;
                    options.access_token = Some(std::env::var(&name).map_err(|_| {
                        QimenError::Config(format!("environment variable '{name}' is not set"))
                    })?);
                }
                "--message" => options.message = Some(take_value(args, &mut index, argument)?),
                "--raw-event" => {
                    options.raw_event = Some(PathBuf::from(take_value(args, &mut index, argument)?))
                }
                "--user-id" => options.user_id = take_value(args, &mut index, argument)?,
                "--self-id" => options.self_id = take_value(args, &mut index, argument)?,
                "--group-id" => options.group_id = Some(take_value(args, &mut index, argument)?),
                "--timeout-secs" => {
                    options.timeout = Duration::from_secs(parse_u64(
                        &take_value(args, &mut index, argument)?,
                        argument,
                    )?)
                }
                "--idle-millis" => {
                    options.idle_timeout = Duration::from_millis(parse_u64(
                        &take_value(args, &mut index, argument)?,
                        argument,
                    )?)
                }
                other => {
                    return Err(QimenError::Config(format!(
                        "unknown simulate-onebot11 option '{other}'"
                    )));
                }
            }
            index += 1;
        }

        match (&options.bot_id, &options.endpoint) {
            (None, None) => {
                return Err(QimenError::Config(
                    "simulate-onebot11 requires --bot or --endpoint".to_string(),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(QimenError::Config(
                    "--bot and --endpoint cannot be used together".to_string(),
                ));
            }
            _ => {}
        }
        match (&options.message, &options.raw_event) {
            (None, None) => {
                return Err(QimenError::Config(
                    "simulate-onebot11 requires --message or --raw-event".to_string(),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(QimenError::Config(
                    "--message and --raw-event cannot be used together".to_string(),
                ));
            }
            _ => {}
        }
        if options.timeout.is_zero() {
            return Err(QimenError::Config(
                "--timeout-secs must be greater than zero".to_string(),
            ));
        }
        Ok(options)
    }
}

fn take_value(args: &[String], index: &mut usize, option: &str) -> Result<String> {
    *index += 1;
    args.get(*index)
        .cloned()
        .ok_or_else(|| QimenError::Config(format!("{option} requires a value")))
}

fn parse_u64(value: &str, option: &str) -> Result<u64> {
    value
        .parse()
        .map_err(|error| QimenError::Config(format!("invalid {option} value '{value}': {error}")))
}

fn help_error() -> QimenError {
    QimenError::Config(help_text().to_string())
}

fn help_text() -> &'static str {
    "Usage:\n  qimenctl simulate-onebot11 --bot <id> --message <text> [options]\n  qimenctl simulate-onebot11 --endpoint <ws-url> --raw-event <json-file> [options]\n\nOptions:\n  --config <path>             Config file used by --bot\n  --bot <id>                  OneBot 11 ws-reverse bot from the config\n  --endpoint <ws-url>         Explicit reverse WebSocket endpoint\n  --access-token <token>      Explicit WebSocket bearer token\n  --access-token-env <name>   Read the bearer token from an environment variable\n  --message <text>            Build and send a private or group message event\n  --raw-event <path>          Send an exact OneBot 11 JSON event\n  --user-id <id>              Sender ID (default: 10000)\n  --self-id <id>              Bot account ID (default: 10001)\n  --group-id <id>             Build a group event instead of a private event\n  --timeout-secs <seconds>    Wait for the first Action (default: 10)\n  --idle-millis <ms>          Collect further Actions after the first (default: 750)\n  --no-lifecycle              Do not send lifecycle.connect before the test event\n\nThe real OneBot client must be disconnected, or a dedicated ws-reverse test bot must be used."
}

pub async fn run(args: &[String]) -> Result<()> {
    if args
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        println!("{}", help_text());
        return Ok(());
    }

    let options = Options::parse(args)?;
    let (endpoint, access_token) = resolve_connection(&options)?;
    let event = load_or_build_event(&options)?;

    let action_count = run_simulation(&endpoint, access_token.as_deref(), &event, &options).await?;
    println!("OneBot 11 simulation completed: {action_count} Action(s) acknowledged");
    Ok(())
}

/// 建立模拟会话、上报事件，并为框架发出的 Action 回写对应 echo。
async fn run_simulation(
    endpoint: &str,
    access_token: Option<&str>,
    event: &Value,
    options: &Options,
) -> Result<usize> {
    println!("connecting OneBot 11 simulator to {endpoint}");
    let mut client = OneBot11ForwardWsClient::connect(endpoint, access_token).await?;

    if options.send_lifecycle {
        let lifecycle = lifecycle_event(&options.self_id);
        client
            .send_text(&serde_json::to_string(&lifecycle)?)
            .await?;
    }
    client.send_text(&serde_json::to_string(&event)?).await?;
    println!("test event sent: {}", serde_json::to_string_pretty(&event)?);

    collect_actions(&mut client, options.timeout, options.idle_timeout).await
}

/// 显式 endpoint 可独立于仓库配置运行；按 bot 解析时才加载配置文件。
fn resolve_connection(options: &Options) -> Result<(String, Option<String>)> {
    if let Some(endpoint) = &options.endpoint {
        return Ok((endpoint.clone(), options.access_token.clone()));
    }

    let config = AppConfig::load_from_path(&options.config_path)?;
    let bot_id = options.bot_id.as_deref().expect("validated bot ID");
    let bot = config
        .bots
        .iter()
        .find(|bot| bot.id == bot_id)
        .ok_or_else(|| QimenError::Config(format!("bot '{bot_id}' was not found")))?;
    validate_test_bot(bot)?;

    let bind = bot
        .bind
        .as_deref()
        .ok_or_else(|| QimenError::Config(format!("bot '{bot_id}' has no ws-reverse bind")))?;
    let path = bot.path.as_deref().unwrap_or("/");
    let endpoint = endpoint_from_bind(bind, path)?;
    Ok((
        endpoint,
        options
            .access_token
            .clone()
            .or_else(|| bot.access_token.clone()),
    ))
}

fn validate_test_bot(bot: &BotConfig) -> Result<()> {
    if bot.protocol != "onebot11" || bot.transport != "ws-reverse" {
        return Err(QimenError::Config(format!(
            "bot '{}' must use protocol=onebot11 and transport=ws-reverse",
            bot.id
        )));
    }
    if !bot.enabled {
        return Err(QimenError::Config(format!(
            "bot '{}' is disabled and has no active reverse WebSocket listener",
            bot.id
        )));
    }
    Ok(())
}

fn endpoint_from_bind(bind: &str, path: &str) -> Result<String> {
    let address: SocketAddr = bind.parse().map_err(|error| {
        QimenError::Config(format!("invalid ws-reverse bind address '{bind}': {error}"))
    })?;
    let host = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".to_string(),
        IpAddr::V4(ip) => ip.to_string(),
        IpAddr::V6(ip) => format!("[{ip}]"),
    };
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    Ok(format!("ws://{host}:{}{path}", address.port()))
}

fn load_or_build_event(options: &Options) -> Result<Value> {
    if let Some(path) = &options.raw_event {
        let raw = std::fs::read_to_string(path)?;
        let event: Value = serde_json::from_str(&raw)?;
        if !event.is_object() {
            return Err(QimenError::Config(
                "--raw-event must contain one JSON object".to_string(),
            ));
        }
        return Ok(event);
    }

    Ok(message_event(
        options.message.as_deref().expect("validated message"),
        &options.user_id,
        &options.self_id,
        options.group_id.as_deref(),
    ))
}

fn lifecycle_event(self_id: &str) -> Value {
    json!({
        "time": unix_time(),
        "self_id": id_value(self_id),
        "post_type": "meta_event",
        "meta_event_type": "lifecycle",
        "sub_type": "connect"
    })
}

fn message_event(message: &str, user_id: &str, self_id: &str, group_id: Option<&str>) -> Value {
    let timestamp = unix_time();
    let message_id = test_message_id();
    let segments = json!([{"type": "text", "data": {"text": message}}]);

    if let Some(group_id) = group_id {
        json!({
            "time": timestamp,
            "self_id": id_value(self_id),
            "post_type": "message",
            "message_type": "group",
            "sub_type": "normal",
            "message_id": message_id,
            "message_seq": message_id,
            "group_id": id_value(group_id),
            "user_id": id_value(user_id),
            "message": segments,
            "message_format": "array",
            "raw_message": message,
            "font": 14,
            "sender": {
                "user_id": id_value(user_id),
                "nickname": "qimenctl-simulator",
                "card": "",
                "role": "owner",
                "title": ""
            }
        })
    } else {
        json!({
            "time": timestamp,
            "self_id": id_value(self_id),
            "post_type": "message",
            "message_type": "private",
            "sub_type": "friend",
            "message_id": message_id,
            "message_seq": message_id,
            "user_id": id_value(user_id),
            "message": segments,
            "message_format": "array",
            "raw_message": message,
            "font": 14,
            "sender": {
                "user_id": id_value(user_id),
                "nickname": "qimenctl-simulator"
            }
        })
    }
}

async fn collect_actions(
    client: &mut OneBot11ForwardWsClient,
    first_timeout: Duration,
    idle_timeout: Duration,
) -> Result<usize> {
    let mut action_count = 0;
    loop {
        let timeout = if action_count == 0 {
            first_timeout
        } else {
            idle_timeout
        };
        let next = tokio::time::timeout(timeout, client.next_event()).await;
        let payload = match next {
            Ok(Some(payload)) => payload,
            Ok(None) if action_count == 0 => {
                return Err(QimenError::Transport(
                    "reverse WebSocket closed before the framework emitted an Action".to_string(),
                ));
            }
            Ok(None) => break,
            Err(_) if action_count == 0 => {
                return Err(QimenError::Runtime(format!(
                    "no OneBot Action was received within {} seconds; check command registration and ensure the real QQ client is disconnected",
                    first_timeout.as_secs()
                )));
            }
            Err(_) => break,
        };

        let action: Value = serde_json::from_str(&payload)?;
        let Some(action_name) = action.get("action").and_then(Value::as_str) else {
            println!("received non-Action payload: {payload}");
            continue;
        };

        action_count += 1;
        println!(
            "received Action #{action_count} ({action_name}): {}",
            serde_json::to_string_pretty(&action)?
        );
        let response = action_response(&action, action_count as i64)?;
        client.send_text(&serde_json::to_string(&response)?).await?;
    }
    Ok(action_count)
}

fn action_response(action: &Value, sequence: i64) -> Result<Value> {
    let echo = action.get("echo").cloned().ok_or_else(|| {
        QimenError::Protocol("framework Action did not contain an echo field".to_string())
    })?;
    let action_name = action.get("action").and_then(Value::as_str).unwrap_or("");
    let data = match action_name {
        "send_msg" | "send_private_msg" | "send_group_msg" => {
            json!({"message_id": 900_000 + sequence})
        }
        "get_login_info" => json!({"user_id": 10001, "nickname": "qimenctl-simulator"}),
        _ => json!({}),
    };
    Ok(json!({
        "status": "ok",
        "retcode": 0,
        "data": data,
        "message": "",
        "wording": "",
        "echo": echo
    }))
}

fn id_value(id: &str) -> Value {
    id.parse::<u64>()
        .map(Value::from)
        .unwrap_or_else(|_| Value::String(id.to_string()))
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn test_message_id() -> i64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    (nanos % i32::MAX as u128) as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use qimen_transport_ws::{WsReverseConfig, WsReverseServer};

    #[test]
    fn builds_private_array_message_event() {
        let event = message_event("status", "20001", "30001", None);
        assert_eq!(event["message_type"], "private");
        assert_eq!(event["user_id"], 20001);
        assert_eq!(event["self_id"], 30001);
        assert_eq!(event["message"][0]["data"]["text"], "status");
        assert_eq!(event["raw_message"], "status");
    }

    #[test]
    fn builds_group_array_message_event() {
        let event = message_event("health", "20001", "30001", Some("40001"));
        assert_eq!(event["message_type"], "group");
        assert_eq!(event["group_id"], 40001);
        assert_eq!(event["sender"]["role"], "owner");
    }

    #[test]
    fn converts_unspecified_bind_to_loopback_endpoint() {
        assert_eq!(
            endpoint_from_bind("0.0.0.0:6710", "/onebot/qimenbot").unwrap(),
            "ws://127.0.0.1:6710/onebot/qimenbot"
        );
        assert_eq!(
            endpoint_from_bind("[::]:6710", "onebot/qimenbot").unwrap(),
            "ws://[::1]:6710/onebot/qimenbot"
        );
    }

    #[test]
    fn action_response_preserves_echo() {
        let response = action_response(
            &json!({
                "action": "send_msg",
                "params": {"user_id": 20001, "message": "ok"},
                "echo": "reply-test"
            }),
            1,
        )
        .unwrap();
        assert_eq!(response["status"], "ok");
        assert_eq!(response["retcode"], 0);
        assert_eq!(response["echo"], "reply-test");
        assert_eq!(response["data"]["message_id"], 900001);
    }

    #[test]
    fn rejects_missing_test_source() {
        let error = Options::parse(&["--bot".into(), "test".into()]).unwrap_err();
        assert!(error.to_string().contains("--message or --raw-event"));
    }

    #[test]
    fn rejects_ambiguous_connection_source() {
        let error = Options::parse(&[
            "--bot".into(),
            "test".into(),
            "--endpoint".into(),
            "ws://127.0.0.1:6710/test".into(),
            "--message".into(),
            "status".into(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("cannot be used together"));
    }

    #[test]
    fn explicit_endpoint_does_not_load_config() {
        let options = Options {
            config_path: "missing-config.toml".to_string(),
            endpoint: Some("ws://127.0.0.1:6710/test".to_string()),
            access_token: Some("secret".to_string()),
            message: Some("status".to_string()),
            ..Options::default()
        };

        let connection = resolve_connection(&options).unwrap();
        assert_eq!(connection.0, "ws://127.0.0.1:6710/test");
        assert_eq!(connection.1.as_deref(), Some("secret"));
    }

    #[tokio::test]
    async fn completes_reverse_websocket_event_action_echo_round_trip() {
        let mut server = WsReverseServer::bind(WsReverseConfig {
            bind: "127.0.0.1:0".to_string(),
            path: "/onebot/test".to_string(),
            access_token: Some("test-token".to_string()),
        })
        .await
        .unwrap();
        let endpoint = format!("ws://{}/onebot/test", server.local_addr());
        let options = Options {
            endpoint: Some(endpoint.clone()),
            access_token: Some("test-token".to_string()),
            message: Some("status".to_string()),
            timeout: Duration::from_secs(2),
            idle_timeout: Duration::from_millis(25),
            ..Options::default()
        };
        let event = message_event("status", "20001", "30001", None);

        let simulator = tokio::spawn(async move {
            run_simulation(&endpoint, options.access_token.as_deref(), &event, &options).await
        });

        let mut connection = tokio::time::timeout(Duration::from_secs(2), server.next_connection())
            .await
            .unwrap()
            .expect("simulator connection");
        let lifecycle: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(2), connection.next_event())
                .await
                .unwrap()
                .expect("lifecycle event"),
        )
        .unwrap();
        assert_eq!(lifecycle["meta_event_type"], "lifecycle");

        let message: Value = serde_json::from_str(
            &tokio::time::timeout(Duration::from_secs(2), connection.next_event())
                .await
                .unwrap()
                .expect("message event"),
        )
        .unwrap();
        assert_eq!(message["raw_message"], "status");

        let action = json!({
            "action": "send_msg",
            "params": {"user_id": 20001, "message": "ok"},
            "echo": "qimenctl-round-trip"
        });
        let response_text = connection
            .send_text_await_echo(
                &serde_json::to_string(&action).unwrap(),
                "qimenctl-round-trip",
                Duration::from_secs(2),
            )
            .await
            .unwrap();
        let response: Value = serde_json::from_str(&response_text).unwrap();
        assert_eq!(response["status"], "ok");
        assert_eq!(response["retcode"], 0);
        assert_eq!(response["echo"], "qimenctl-round-trip");

        assert_eq!(simulator.await.unwrap().unwrap(), 1);
    }
}
