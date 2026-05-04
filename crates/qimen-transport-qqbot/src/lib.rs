use qimen_error::{QimenError, Result};
use qimen_transport_ws::OneBot11ForwardWsClient;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub const OP_DISPATCH: i64 = 0;
pub const OP_HEARTBEAT: i64 = 1;
pub const OP_IDENTIFY: i64 = 2;
pub const OP_RESUME: i64 = 6;
pub const OP_RECONNECT: i64 = 7;
pub const OP_INVALID_SESSION: i64 = 9;
pub const OP_HELLO: i64 = 10;
pub const OP_HEARTBEAT_ACK: i64 = 11;

const TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const PROD_BASE_URL: &str = "https://api.sgroup.qq.com";
const SANDBOX_BASE_URL: &str = "https://sandbox.api.sgroup.qq.com";

#[derive(Debug, Clone)]
pub struct QqBotOpenApiConfig {
    pub appid: String,
    pub secret: String,
    pub sandbox: bool,
    pub timeout: Duration,
    pub token_url: String,
    pub base_url: String,
}

impl QqBotOpenApiConfig {
    pub fn new(appid: impl Into<String>, secret: impl Into<String>) -> Self {
        Self {
            appid: appid.into(),
            secret: secret.into(),
            sandbox: false,
            timeout: Duration::from_secs(20),
            token_url: TOKEN_URL.to_string(),
            base_url: PROD_BASE_URL.to_string(),
        }
    }

    pub fn base_url(&self) -> &str {
        if self.sandbox {
            SANDBOX_BASE_URL
        } else {
            &self.base_url
        }
    }
}

#[derive(Debug)]
pub struct QqBotOpenApiClient {
    http: reqwest::Client,
    config: QqBotOpenApiConfig,
    token: Mutex<Option<CachedAccessToken>>,
}

#[derive(Debug, Clone)]
struct CachedAccessToken {
    value: String,
    expires_at: Instant,
}

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GatewayUrlResponse {
    pub url: String,
    #[serde(default)]
    pub shards: Option<u64>,
    #[serde(default)]
    pub session_start_limit: Option<SessionStartLimit>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionStartLimit {
    pub total: u64,
    pub remaining: u64,
    pub reset_after: u64,
    pub max_concurrency: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SendMessagePayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_type: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_seq: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ark: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

impl SendMessagePayload {
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            msg_type: Some(0),
            content: Some(content.into()),
            msg_id: None,
            msg_seq: None,
            event_id: None,
            markdown: None,
            keyboard: None,
            ark: None,
            embed: None,
            media: None,
            image: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadFilePayload {
    pub file_type: i64,
    pub url: String,
    pub srv_send_msg: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QqBotApiError {
    pub path: String,
    pub status: u16,
    pub code: Option<i64>,
    pub message: String,
    pub category: QqBotApiErrorCategory,
    pub retry_after_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QqBotApiErrorCategory {
    Authentication,
    Permission,
    RateLimited,
    NotFound,
    BadRequest,
    Server,
    Network,
    Unknown,
}

impl std::fmt::Display for QqBotApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "qqbot request {} failed with HTTP {}",
            self.path, self.status
        )?;
        if let Some(code) = self.code {
            write!(f, ", code {code}")?;
        }
        write!(f, ", category {:?}: {}", self.category, self.message)?;
        if let Some(retry_after_ms) = self.retry_after_ms {
            write!(f, ", retry_after_ms={retry_after_ms}")?;
        }
        Ok(())
    }
}

impl QqBotOpenApiClient {
    pub fn new(config: QqBotOpenApiConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|err| {
                QimenError::Transport(format!("failed to build qqbot http client: {err}"))
            })?;
        Ok(Self {
            http,
            config,
            token: Mutex::new(None),
        })
    }

    pub async fn access_token(&self) -> Result<String> {
        let mut guard = self.token.lock().await;
        if let Some(token) = guard.as_ref()
            && Instant::now() < token.expires_at
        {
            return Ok(token.value.clone());
        }

        let token = self.fetch_access_token().await?;
        let value = token.value.clone();
        *guard = Some(token);
        Ok(value)
    }

    pub async fn bot_authorization(&self) -> Result<String> {
        Ok(format!("QQBot {}", self.access_token().await?))
    }

    pub async fn get_gateway(&self) -> Result<GatewayUrlResponse> {
        self.get_json("/gateway/bot").await
    }

    pub async fn post_channel_message(
        &self,
        channel_id: &str,
        payload: &SendMessagePayload,
    ) -> Result<Value> {
        self.post_json(&format!("/channels/{channel_id}/messages"), payload)
            .await
    }

    pub async fn post_group_message(
        &self,
        group_openid: &str,
        payload: &SendMessagePayload,
    ) -> Result<Value> {
        self.post_json(&format!("/v2/groups/{group_openid}/messages"), payload)
            .await
    }

    pub async fn post_group_file(
        &self,
        group_openid: &str,
        payload: &UploadFilePayload,
    ) -> Result<Value> {
        self.post_json(&format!("/v2/groups/{group_openid}/files"), payload)
            .await
    }

    pub async fn post_c2c_message(
        &self,
        openid: &str,
        payload: &SendMessagePayload,
    ) -> Result<Value> {
        self.post_json(&format!("/v2/users/{openid}/messages"), payload)
            .await
    }

    pub async fn post_c2c_file(&self, openid: &str, payload: &UploadFilePayload) -> Result<Value> {
        self.post_json(&format!("/v2/users/{openid}/files"), payload)
            .await
    }

    pub async fn post_dms_message(
        &self,
        guild_id: &str,
        payload: &SendMessagePayload,
    ) -> Result<Value> {
        self.post_json(&format!("/dms/{guild_id}/messages"), payload)
            .await
    }

    pub async fn recall_channel_message(
        &self,
        channel_id: &str,
        message_id: &str,
        hidetip: bool,
    ) -> Result<Value> {
        self.delete_json(
            &format!("/channels/{channel_id}/messages/{message_id}"),
            &[("hidetip", hidetip.to_string())],
        )
        .await
    }

    async fn fetch_access_token(&self) -> Result<CachedAccessToken> {
        let response = self
            .http
            .post(&self.config.token_url)
            .json(&json!({
                "appId": self.config.appid,
                "clientSecret": self.config.secret,
            }))
            .send()
            .await
            .map_err(|err| {
                QimenError::Transport(format!("failed to request qqbot access token: {err}"))
            })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            QimenError::Transport(format!("failed to read qqbot token response: {err}"))
        })?;

        if !status.is_success() {
            return Err(QimenError::Transport(format!(
                "qqbot token request failed with {status}: {body}"
            )));
        }

        let parsed: TokenResponse = serde_json::from_str(&body)?;
        let ttl_secs = parsed.expires_in.parse::<u64>().map_err(|err| {
            QimenError::Transport(format!(
                "qqbot token response has invalid expires_in '{}': {err}",
                parsed.expires_in
            ))
        })?;
        let refresh_after = ttl_secs.saturating_sub(60).max(1);

        Ok(CachedAccessToken {
            value: parsed.access_token,
            expires_at: Instant::now() + Duration::from_secs(refresh_after),
        })
    }

    async fn get_json<T>(&self, path: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let token = self.bot_authorization().await?;
        let response = self
            .http
            .get(format!("{}{}", self.config.base_url(), path))
            .header("Authorization", token)
            .header("X-Union-Appid", self.config.appid.as_str())
            .send()
            .await
            .map_err(|err| QimenError::Transport(format!("qqbot GET {path} failed: {err}")))?;

        decode_response(response, path).await
    }

    async fn post_json<T>(&self, path: &str, payload: &T) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        let token = self.bot_authorization().await?;
        let response = self
            .http
            .post(format!("{}{}", self.config.base_url(), path))
            .header("Authorization", token)
            .header("X-Union-Appid", self.config.appid.as_str())
            .json(payload)
            .send()
            .await
            .map_err(|err| QimenError::Transport(format!("qqbot POST {path} failed: {err}")))?;

        decode_response(response, path).await
    }

    async fn delete_json(&self, path: &str, query: &[(&str, String)]) -> Result<Value> {
        let token = self.bot_authorization().await?;
        let response = self
            .http
            .delete(format!("{}{}", self.config.base_url(), path))
            .query(query)
            .header("Authorization", token)
            .header("X-Union-Appid", self.config.appid.as_str())
            .send()
            .await
            .map_err(|err| QimenError::Transport(format!("qqbot DELETE {path} failed: {err}")))?;

        decode_response(response, path).await
    }
}

async fn decode_response<T>(response: reqwest::Response, path: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        QimenError::Transport(format!("failed to read qqbot response for {path}: {err}"))
    })?;

    if !status.is_success() {
        return Err(QimenError::Transport(
            build_api_error(path, status.as_u16(), &body).to_string(),
        ));
    }

    if body.trim().is_empty() {
        return serde_json::from_value(Value::Null).map_err(QimenError::Json);
    }

    serde_json::from_str(&body).map_err(QimenError::Json)
}

fn build_api_error(path: &str, status: u16, body: &str) -> QqBotApiError {
    let parsed = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let code = parsed
        .get("code")
        .or_else(|| parsed.get("errcode"))
        .and_then(Value::as_i64);
    let message = parsed
        .get("message")
        .or_else(|| parsed.get("errmsg"))
        .or_else(|| parsed.get("msg"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| body.trim().to_string());
    let retry_after_ms = parsed
        .get("retry_after")
        .or_else(|| parsed.get("retry_after_ms"))
        .and_then(Value::as_u64);

    QqBotApiError {
        path: path.to_string(),
        status,
        code,
        category: classify_api_error(status, code, &message),
        message,
        retry_after_ms,
    }
}

fn classify_api_error(status: u16, code: Option<i64>, message: &str) -> QqBotApiErrorCategory {
    let lower = message.to_ascii_lowercase();
    if status == 401 || status == 403 && lower.contains("token") {
        return QqBotApiErrorCategory::Authentication;
    }
    if status == 429
        || lower.contains("rate")
        || lower.contains("frequency")
        || lower.contains("频控")
    {
        return QqBotApiErrorCategory::RateLimited;
    }
    if status == 403 {
        return QqBotApiErrorCategory::Permission;
    }
    if status == 404 {
        return QqBotApiErrorCategory::NotFound;
    }
    if (400..500).contains(&status) {
        return QqBotApiErrorCategory::BadRequest;
    }
    if status >= 500 {
        return QqBotApiErrorCategory::Server;
    }
    match code {
        Some(11241 | 304023 | 304024) => QqBotApiErrorCategory::RateLimited,
        _ => QqBotApiErrorCategory::Unknown,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QqBotGatewaySession {
    pub session_id: Option<String>,
    pub last_sequence: Option<i64>,
    pub intents: u64,
    pub shard_id: u64,
    pub shard_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayEvent {
    #[serde(rename = "op")]
    pub opcode: i64,
    #[serde(rename = "s")]
    pub sequence: Option<i64>,
    #[serde(rename = "t")]
    pub event_type: Option<String>,
    #[serde(rename = "d")]
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayHelloData {
    pub heartbeat_interval: u64,
}

#[derive(Debug, Clone)]
pub enum GatewayStep {
    Dispatch(GatewayEvent),
    HeartbeatAck,
    RemoteHeartbeat,
    Reconnect,
    InvalidSession,
    Ready,
    Resumed,
    Ignored,
}

pub struct QqBotGatewayClient {
    ws: OneBot11ForwardWsClient,
    session: QqBotGatewaySession,
    heartbeat_interval: Duration,
    awaiting_heartbeat_ack: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayFrameState {
    pub session: QqBotGatewaySession,
    pub awaiting_heartbeat_ack: bool,
}

impl GatewayFrameState {
    pub fn new(session: QqBotGatewaySession) -> Self {
        Self {
            session,
            awaiting_heartbeat_ack: false,
        }
    }

    pub fn apply_event(&mut self, event: GatewayEvent) -> GatewayStep {
        if let Some(sequence) = event.sequence {
            self.session.last_sequence = Some(sequence);
        }

        match event.opcode {
            OP_DISPATCH => match event.event_type.as_deref() {
                Some("READY") => {
                    self.session.apply_ready_data(&event.data);
                    GatewayStep::Ready
                }
                Some("RESUMED") => GatewayStep::Resumed,
                _ => GatewayStep::Dispatch(event),
            },
            OP_HEARTBEAT => {
                self.awaiting_heartbeat_ack = true;
                GatewayStep::RemoteHeartbeat
            }
            OP_HEARTBEAT_ACK => {
                self.awaiting_heartbeat_ack = false;
                GatewayStep::HeartbeatAck
            }
            OP_RECONNECT => GatewayStep::Reconnect,
            OP_INVALID_SESSION => {
                self.session.session_id = None;
                self.session.last_sequence = None;
                GatewayStep::InvalidSession
            }
            _ => GatewayStep::Ignored,
        }
    }
}

impl QqBotGatewayClient {
    pub async fn connect(
        endpoint: &str,
        session: QqBotGatewaySession,
        token: &str,
    ) -> Result<Self> {
        let mut ws = OneBot11ForwardWsClient::connect(endpoint, None).await?;
        let hello = wait_for_hello(&mut ws).await?;
        let heartbeat_interval = Duration::from_millis(hello.heartbeat_interval);

        let payload = if session.session_id.is_some() {
            session.resume_payload(token)?
        } else {
            session.identify_payload(token)
        };
        ws.send_text(&serde_json::to_string(&payload)?).await?;

        Ok(Self {
            ws,
            session,
            heartbeat_interval,
            awaiting_heartbeat_ack: false,
        })
    }

    pub fn session(&self) -> &QqBotGatewaySession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut QqBotGatewaySession {
        &mut self.session
    }

    pub fn heartbeat_interval(&self) -> Duration {
        self.heartbeat_interval
    }

    pub fn should_reconnect_for_missing_ack(&self) -> bool {
        self.awaiting_heartbeat_ack
    }

    pub async fn send_heartbeat(&mut self) -> Result<()> {
        let payload = self.session.heartbeat_payload();
        self.ws.send_text(&serde_json::to_string(&payload)?).await?;
        self.awaiting_heartbeat_ack = true;
        Ok(())
    }

    pub async fn next_step(&mut self) -> Result<Option<GatewayStep>> {
        let Some(text) = self.ws.next_event().await else {
            return Ok(None);
        };

        let event = parse_gateway_event(&text)?;
        let mut state = GatewayFrameState {
            session: self.session.clone(),
            awaiting_heartbeat_ack: self.awaiting_heartbeat_ack,
        };
        let step = state.apply_event(event);
        self.session = state.session;
        self.awaiting_heartbeat_ack = state.awaiting_heartbeat_ack;

        match step {
            GatewayStep::RemoteHeartbeat => {
                self.send_heartbeat().await?;
                Ok(Some(GatewayStep::Ignored))
            }
            other => Ok(Some(other)),
        }
    }
}

impl QqBotGatewaySession {
    pub fn identify_payload(&self, token: &str) -> Value {
        json!({
            "op": OP_IDENTIFY,
            "d": {
                "token": token,
                "intents": self.intents,
                "shard": [self.shard_id, self.shard_count],
            }
        })
    }

    pub fn resume_payload(&self, token: &str) -> Result<Value> {
        let Some(session_id) = self.session_id.as_deref() else {
            return Err(QimenError::Transport(
                "cannot resume qqbot gateway without session_id".to_string(),
            ));
        };

        Ok(json!({
            "op": OP_RESUME,
            "d": {
                "token": token,
                "session_id": session_id,
                "seq": self.last_sequence.unwrap_or_default(),
            }
        }))
    }

    pub fn heartbeat_payload(&self) -> Value {
        json!({
            "op": OP_HEARTBEAT,
            "d": self.last_sequence,
        })
    }

    pub fn apply_ready_data(&mut self, data: &Value) {
        if let Some(session_id) = data.get("session_id").and_then(Value::as_str) {
            self.session_id = Some(session_id.to_string());
        }
        if let Some(shard) = data.get("shard").and_then(Value::as_array)
            && shard.len() >= 2
        {
            if let Some(shard_id) = shard[0].as_u64() {
                self.shard_id = shard_id;
            }
            if let Some(shard_count) = shard[1].as_u64() {
                self.shard_count = shard_count;
            }
        }
    }
}

async fn wait_for_hello(ws: &mut OneBot11ForwardWsClient) -> Result<GatewayHelloData> {
    loop {
        let Some(text) = ws.next_event().await else {
            return Err(QimenError::Transport(
                "qqbot gateway closed before Hello".to_string(),
            ));
        };

        let event = parse_gateway_event(&text)?;
        if event.opcode != OP_HELLO {
            tracing::debug!(opcode = event.opcode, "ignoring gateway frame before Hello");
            continue;
        }

        return serde_json::from_value(event.data).map_err(QimenError::Json);
    }
}

fn parse_gateway_event(text: &str) -> Result<GatewayEvent> {
    serde_json::from_str(text).map_err(QimenError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    #[derive(Debug, Clone)]
    struct RecordedRequest {
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: String,
    }

    async fn spawn_mock_server() -> (String, tokio::sync::mpsc::Receiver<RecordedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    break;
                };
                let tx = tx.clone();
                tokio::spawn(async move {
                    let mut buffer = Vec::new();
                    let mut chunk = [0_u8; 1024];
                    let header_end;
                    loop {
                        let n = stream.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        buffer.extend_from_slice(&chunk[..n]);
                        if let Some(pos) = find_header_end(&buffer) {
                            header_end = pos;
                            break;
                        }
                    }

                    let headers_text = String::from_utf8_lossy(&buffer[..header_end]).to_string();
                    let content_length = content_length(&headers_text);
                    let body_start = header_end + 4;
                    while buffer.len() < body_start + content_length {
                        let n = stream.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        buffer.extend_from_slice(&chunk[..n]);
                    }

                    let request = parse_recorded_request(
                        &headers_text,
                        String::from_utf8_lossy(&buffer[body_start..body_start + content_length])
                            .to_string(),
                    );
                    let response = mock_response(&request.path);
                    let _ = tx.send(request).await;
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });

        (format!("http://{addr}"), rx)
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn content_length(headers_text: &str) -> usize {
        headers_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    fn parse_recorded_request(headers_text: &str, body: String) -> RecordedRequest {
        let mut lines = headers_text.lines();
        let request_line = lines.next().unwrap_or_default();
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts.next().unwrap_or_default().to_string();
        let path = request_parts.next().unwrap_or_default().to_string();
        let headers = lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
            })
            .collect();

        RecordedRequest {
            method,
            path,
            headers,
            body,
        }
    }

    fn mock_response(path: &str) -> String {
        let body = match path {
            "/app/getAppAccessToken" => json!({
                "access_token": "mock-token",
                "expires_in": "3600",
            }),
            "/gateway/bot" => json!({
                "url": "wss://mock-gateway/websocket",
                "shards": 2,
                "session_start_limit": {
                    "total": 1000,
                    "remaining": 999,
                    "reset_after": 10,
                    "max_concurrency": 1,
                },
            }),
            "/channels/channel-1/messages"
            | "/v2/groups/group-1/messages"
            | "/v2/users/user-1/messages"
            | "/dms/guild-1/messages" => json!({
                "id": "sent-message",
            }),
            "/v2/groups/group-1/files" | "/v2/users/user-1/files" => json!({
                "file_uuid": "file-uuid",
                "file_info": "file-info",
                "ttl": 3600,
            }),
            "/channels/channel-1/messages/message-1?hidetip=true" => Value::Null,
            "/rate-limited" => json!({
                "code": 11241,
                "message": "rate limit exceeded",
                "retry_after": 1000,
            }),
            "/forbidden" => json!({
                "code": 304003,
                "message": "permission denied",
            }),
            _ => json!({
                "code": 404,
                "message": "not found",
            }),
        }
        .to_string();

        let status = match path {
            "/unknown" => "404 Not Found",
            "/rate-limited" => "429 Too Many Requests",
            "/forbidden" => "403 Forbidden",
            _ => "200 OK",
        };
        format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    fn header_value<'a>(request: &'a RecordedRequest, name: &str) -> Option<&'a str> {
        request
            .headers
            .iter()
            .find_map(|(key, value)| (key == name).then_some(value.as_str()))
    }

    fn mock_config(base_url: String) -> QqBotOpenApiConfig {
        QqBotOpenApiConfig {
            appid: "appid".to_string(),
            secret: "secret".to_string(),
            sandbox: false,
            timeout: Duration::from_secs(5),
            token_url: format!("{base_url}/app/getAppAccessToken"),
            base_url,
        }
    }

    async fn spawn_mock_gateway(
        frames_after_identify: Vec<Value>,
    ) -> (String, tokio::sync::mpsc::Receiver<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        tokio::spawn(async move {
            let Ok((mut stream, _peer)) = listener.accept().await else {
                return;
            };

            read_ws_handshake(&mut stream).await.unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 101 Switching Protocols\r\n\
                    Upgrade: websocket\r\n\
                    Connection: Upgrade\r\n\r\n",
                )
                .await
                .unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
            write_ws_text_unmasked(
                &mut stream,
                &json!({
                    "op": OP_HELLO,
                    "d": {
                        "heartbeat_interval": 30_000,
                    },
                })
                .to_string(),
            )
            .await
            .unwrap();

            if let Some(identify) = read_ws_text_masked(&mut stream).await.unwrap() {
                let payload = serde_json::from_str::<Value>(&identify).unwrap();
                tx.send(payload).await.unwrap();
            }

            for frame in frames_after_identify {
                write_ws_text_unmasked(&mut stream, &frame.to_string())
                    .await
                    .unwrap();
            }

            while let Some(text) = read_ws_text_masked(&mut stream).await.unwrap() {
                let payload = serde_json::from_str::<Value>(&text).unwrap();
                if tx.send(payload).await.is_err() {
                    break;
                }
            }
        });

        (format!("ws://{addr}/websocket"), rx)
    }

    async fn read_ws_handshake(stream: &mut TcpStream) -> std::io::Result<String> {
        let mut buffer = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..n]);
            if find_header_end(&buffer).is_some() {
                break;
            }
        }
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    async fn read_ws_text_masked(stream: &mut TcpStream) -> std::io::Result<Option<String>> {
        let mut header = [0_u8; 2];
        match stream.read_exact(&mut header).await {
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(err) => return Err(err),
        }

        let opcode = header[0] & 0x0f;
        let masked = header[1] & 0x80 != 0;
        let mut payload_len = (header[1] & 0x7f) as u64;

        if payload_len == 126 {
            let mut len_buf = [0_u8; 2];
            stream.read_exact(&mut len_buf).await?;
            payload_len = u16::from_be_bytes(len_buf) as u64;
        } else if payload_len == 127 {
            let mut len_buf = [0_u8; 8];
            stream.read_exact(&mut len_buf).await?;
            payload_len = u64::from_be_bytes(len_buf);
        }

        let mut mask = [0_u8; 4];
        if masked {
            stream.read_exact(&mut mask).await?;
        }

        let mut payload = vec![0_u8; payload_len as usize];
        if payload_len > 0 {
            stream.read_exact(&mut payload).await?;
        }

        if masked {
            for (index, byte) in payload.iter_mut().enumerate() {
                *byte ^= mask[index % 4];
            }
        }

        match opcode {
            0x1 => String::from_utf8(payload)
                .map(Some)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err)),
            0x8 => Ok(None),
            _ => Ok(Some(String::new())),
        }
    }

    async fn write_ws_text_unmasked(stream: &mut TcpStream, text: &str) -> std::io::Result<()> {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 16);
        frame.push(0x81);

        if payload.len() <= 125 {
            frame.push(payload.len() as u8);
        } else if payload.len() <= u16::MAX as usize {
            frame.push(126);
            frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        } else {
            frame.push(127);
            frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
        }

        frame.extend_from_slice(payload);
        stream.write_all(&frame).await
    }

    async fn recv_gateway_payload(rx: &mut tokio::sync::mpsc::Receiver<Value>) -> Value {
        tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap()
    }

    fn gateway_session() -> QqBotGatewaySession {
        QqBotGatewaySession {
            session_id: None,
            last_sequence: None,
            intents: (1_u64 << 25) | (1_u64 << 30),
            shard_id: 0,
            shard_count: 1,
        }
    }

    #[test]
    fn config_selects_sandbox_base_url() {
        let mut config = QqBotOpenApiConfig::new("appid", "secret");
        assert_eq!(config.base_url(), PROD_BASE_URL);
        config.sandbox = true;
        assert_eq!(config.base_url(), SANDBOX_BASE_URL);
    }

    #[test]
    fn send_text_payload_defaults_to_text_type() {
        let payload = SendMessagePayload::text("hello");
        let value = serde_json::to_value(payload).unwrap();
        assert_eq!(value.get("msg_type").and_then(Value::as_i64), Some(0));
        assert_eq!(value.get("content").and_then(Value::as_str), Some("hello"));
        assert!(value.get("msg_id").is_none());
    }

    #[test]
    fn send_rich_payload_serializes_markdown_and_keyboard() {
        let payload = SendMessagePayload {
            msg_type: Some(2),
            content: Some("fallback".to_string()),
            msg_id: Some("msg-1".to_string()),
            msg_seq: Some(1),
            event_id: None,
            markdown: Some(json!({ "content": "# Title" })),
            keyboard: Some(json!({ "id": "keyboard-template" })),
            ark: Some(json!({ "template_id": 37 })),
            embed: Some(json!({ "title": "embed" })),
            media: Some(json!({ "file_info": "file-info" })),
            image: Some("https://example.invalid/a.png".to_string()),
        };

        let value = serde_json::to_value(payload).unwrap();

        assert_eq!(
            value,
            json!({
                "msg_type": 2,
                "content": "fallback",
                "msg_id": "msg-1",
                "msg_seq": 1,
                "markdown": { "content": "# Title" },
                "keyboard": { "id": "keyboard-template" },
                "ark": { "template_id": 37 },
                "embed": { "title": "embed" },
                "media": { "file_info": "file-info" },
                "image": "https://example.invalid/a.png",
            })
        );
    }

    #[tokio::test]
    async fn openapi_fetches_and_caches_access_token() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();

        assert_eq!(client.access_token().await.unwrap(), "mock-token");
        assert_eq!(client.access_token().await.unwrap(), "mock-token");

        let request = requests.recv().await.unwrap();
        assert_eq!(request.method, "POST");
        assert_eq!(request.path, "/app/getAppAccessToken");
        assert_eq!(
            serde_json::from_str::<Value>(&request.body).unwrap(),
            json!({
                "appId": "appid",
                "clientSecret": "secret",
            })
        );
        assert!(requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn openapi_get_gateway_uses_authorization_headers() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();

        let gateway = client.get_gateway().await.unwrap();

        assert_eq!(gateway.url, "wss://mock-gateway/websocket");
        assert_eq!(gateway.shards, Some(2));
        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");
        let gateway_request = requests.recv().await.unwrap();
        assert_eq!(gateway_request.method, "GET");
        assert_eq!(gateway_request.path, "/gateway/bot");
        assert_eq!(
            header_value(&gateway_request, "authorization"),
            Some("QQBot mock-token")
        );
        assert_eq!(
            header_value(&gateway_request, "x-union-appid"),
            Some("appid")
        );
    }

    #[tokio::test]
    async fn openapi_posts_all_text_message_routes() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();
        let payload = SendMessagePayload::text("pong");

        client
            .post_channel_message("channel-1", &payload)
            .await
            .unwrap();
        client
            .post_group_message("group-1", &payload)
            .await
            .unwrap();
        client.post_c2c_message("user-1", &payload).await.unwrap();
        client.post_dms_message("guild-1", &payload).await.unwrap();

        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");
        for expected_path in [
            "/channels/channel-1/messages",
            "/v2/groups/group-1/messages",
            "/v2/users/user-1/messages",
            "/dms/guild-1/messages",
        ] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, expected_path);
            assert_eq!(
                header_value(&request, "authorization"),
                Some("QQBot mock-token")
            );
            assert_eq!(
                serde_json::from_str::<Value>(&request.body).unwrap(),
                json!({
                    "msg_type": 0,
                    "content": "pong",
                })
            );
        }
    }

    #[tokio::test]
    async fn openapi_posts_group_and_c2c_file_uploads() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();
        let payload = UploadFilePayload {
            file_type: 1,
            url: "https://example.invalid/a.png".to_string(),
            srv_send_msg: false,
        };

        let group_media = client.post_group_file("group-1", &payload).await.unwrap();
        let c2c_media = client.post_c2c_file("user-1", &payload).await.unwrap();

        assert_eq!(
            group_media.get("file_uuid").and_then(Value::as_str),
            Some("file-uuid")
        );
        assert_eq!(
            c2c_media.get("file_info").and_then(Value::as_str),
            Some("file-info")
        );
        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");
        for expected_path in ["/v2/groups/group-1/files", "/v2/users/user-1/files"] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, expected_path);
            assert_eq!(
                serde_json::from_str::<Value>(&request.body).unwrap(),
                json!({
                    "file_type": 1,
                    "url": "https://example.invalid/a.png",
                    "srv_send_msg": false,
                })
            );
        }
    }

    #[tokio::test]
    async fn openapi_recalls_channel_message() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();

        let value = client
            .recall_channel_message("channel-1", "message-1", true)
            .await
            .unwrap();

        assert_eq!(value, Value::Null);
        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");
        let request = requests.recv().await.unwrap();
        assert_eq!(request.method, "DELETE");
        assert_eq!(
            request.path,
            "/channels/channel-1/messages/message-1?hidetip=true"
        );
        assert_eq!(
            header_value(&request, "authorization"),
            Some("QQBot mock-token")
        );
    }

    #[test]
    fn api_error_classifies_rate_limit_and_permission() {
        let rate_limit = build_api_error(
            "/v2/users/user-1/messages",
            429,
            r#"{"code":11241,"message":"rate limit exceeded","retry_after":1000}"#,
        );
        let permission = build_api_error(
            "/channels/channel-1/messages/message-1",
            403,
            r#"{"code":304003,"message":"permission denied"}"#,
        );

        assert_eq!(rate_limit.category, QqBotApiErrorCategory::RateLimited);
        assert_eq!(rate_limit.retry_after_ms, Some(1000));
        assert_eq!(permission.category, QqBotApiErrorCategory::Permission);
        assert!(rate_limit.to_string().contains("RateLimited"));
    }

    #[tokio::test]
    async fn gateway_connect_sends_identify_after_hello() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(Vec::new()).await;
        let _client = QqBotGatewayClient::connect(&endpoint, gateway_session(), "QQBot token")
            .await
            .unwrap();

        let identify = recv_gateway_payload(&mut sent_payloads).await;
        assert_eq!(
            identify,
            json!({
                "op": OP_IDENTIFY,
                "d": {
                    "token": "QQBot token",
                    "intents": (1_u64 << 25) | (1_u64 << 30),
                    "shard": [0, 1],
                }
            })
        );
    }

    #[tokio::test]
    async fn gateway_connect_sends_resume_when_session_exists() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(Vec::new()).await;
        let mut session = gateway_session();
        session.session_id = Some("session-1".to_string());
        session.last_sequence = Some(42);

        let _client = QqBotGatewayClient::connect(&endpoint, session, "QQBot token")
            .await
            .unwrap();

        let resume = recv_gateway_payload(&mut sent_payloads).await;
        assert_eq!(
            resume,
            json!({
                "op": OP_RESUME,
                "d": {
                    "token": "QQBot token",
                    "session_id": "session-1",
                    "seq": 42,
                }
            })
        );
    }

    #[tokio::test]
    async fn gateway_ready_updates_session() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(vec![json!({
            "op": OP_DISPATCH,
            "s": 7,
            "t": "READY",
            "d": {
                "session_id": "session-ready",
                "shard": [0, 2],
            }
        })])
        .await;
        let mut client = QqBotGatewayClient::connect(&endpoint, gateway_session(), "QQBot token")
            .await
            .unwrap();
        let _identify = recv_gateway_payload(&mut sent_payloads).await;

        let step = client.next_step().await.unwrap().unwrap();

        assert!(matches!(step, GatewayStep::Ready));
        assert_eq!(
            client.session().session_id.as_deref(),
            Some("session-ready")
        );
        assert_eq!(client.session().last_sequence, Some(7));
        assert_eq!(client.session().shard_count, 2);
    }

    #[tokio::test]
    async fn gateway_send_heartbeat_and_ack_clears_pending_flag() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(vec![json!({
            "op": OP_HEARTBEAT_ACK,
        })])
        .await;
        let mut client = QqBotGatewayClient::connect(&endpoint, gateway_session(), "QQBot token")
            .await
            .unwrap();
        let _identify = recv_gateway_payload(&mut sent_payloads).await;

        client.send_heartbeat().await.unwrap();
        assert!(client.should_reconnect_for_missing_ack());
        let heartbeat = recv_gateway_payload(&mut sent_payloads).await;
        assert_eq!(
            heartbeat,
            json!({
                "op": OP_HEARTBEAT,
                "d": null,
            })
        );

        let step = client.next_step().await.unwrap().unwrap();

        assert!(matches!(step, GatewayStep::HeartbeatAck));
        assert!(!client.should_reconnect_for_missing_ack());
    }

    #[tokio::test]
    async fn gateway_control_frames_surface_reconnect_and_invalid_session() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(vec![
            json!({
                "op": OP_RECONNECT,
            }),
            json!({
                "op": OP_INVALID_SESSION,
            }),
        ])
        .await;
        let mut client = QqBotGatewayClient::connect(&endpoint, gateway_session(), "QQBot token")
            .await
            .unwrap();
        let _identify = recv_gateway_payload(&mut sent_payloads).await;

        let step = client.next_step().await.unwrap().unwrap();
        assert!(matches!(step, GatewayStep::Reconnect));

        let step = client.next_step().await.unwrap().unwrap();
        assert!(matches!(step, GatewayStep::InvalidSession));
        assert_eq!(client.session().session_id, None);
        assert_eq!(client.session().last_sequence, None);
    }

    #[test]
    fn gateway_identify_payload_matches_official_shape() {
        let session = QqBotGatewaySession {
            session_id: None,
            last_sequence: None,
            intents: (1_u64 << 25) | (1_u64 << 30),
            shard_id: 0,
            shard_count: 1,
        };

        assert_eq!(
            session.identify_payload("QQBot token"),
            json!({
                "op": OP_IDENTIFY,
                "d": {
                    "token": "QQBot token",
                    "intents": (1_u64 << 25) | (1_u64 << 30),
                    "shard": [0, 1],
                }
            })
        );
    }

    #[test]
    fn gateway_resume_requires_session_id() {
        let session = QqBotGatewaySession {
            session_id: None,
            last_sequence: Some(10),
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        };
        assert!(session.resume_payload("QQBot token").is_err());
    }

    #[test]
    fn gateway_heartbeat_uses_last_sequence() {
        let session = QqBotGatewaySession {
            session_id: Some("session".to_string()),
            last_sequence: Some(99),
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        };
        assert_eq!(
            session.heartbeat_payload(),
            json!({
                "op": OP_HEARTBEAT,
                "d": 99,
            })
        );
    }

    #[test]
    fn ready_data_updates_gateway_session_fields() {
        let mut session = QqBotGatewaySession {
            session_id: None,
            last_sequence: Some(1),
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        };

        session.apply_ready_data(&json!({
            "session_id": "session-1",
            "shard": [1, 4],
        }));

        assert_eq!(session.session_id.as_deref(), Some("session-1"));
        assert_eq!(session.shard_id, 1);
        assert_eq!(session.shard_count, 4);
    }

    #[test]
    fn gateway_event_parses_opcode_and_sequence() {
        let event = parse_gateway_event(
            r#"{"op":0,"s":12,"t":"GROUP_AT_MESSAGE_CREATE","d":{"id":"msg"}}"#,
        )
        .unwrap();

        assert_eq!(event.opcode, OP_DISPATCH);
        assert_eq!(event.sequence, Some(12));
        assert_eq!(event.event_type.as_deref(), Some("GROUP_AT_MESSAGE_CREATE"));
        assert_eq!(event.data.get("id").and_then(Value::as_str), Some("msg"));
    }

    #[test]
    fn gateway_event_allows_missing_data_for_control_frames() {
        let event = parse_gateway_event(r#"{"op":11}"#).unwrap();

        assert_eq!(event.opcode, OP_HEARTBEAT_ACK);
        assert_eq!(event.sequence, None);
        assert_eq!(event.event_type, None);
        assert_eq!(event.data, Value::Null);
    }

    #[test]
    fn gateway_frame_state_applies_ready_and_sequence() {
        let mut state = GatewayFrameState::new(QqBotGatewaySession {
            session_id: None,
            last_sequence: None,
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        });

        let step = state.apply_event(GatewayEvent {
            opcode: OP_DISPATCH,
            sequence: Some(10),
            event_type: Some("READY".to_string()),
            data: json!({
                "session_id": "session-1",
                "shard": [2, 4],
            }),
        });

        assert!(matches!(step, GatewayStep::Ready));
        assert_eq!(state.session.last_sequence, Some(10));
        assert_eq!(state.session.session_id.as_deref(), Some("session-1"));
        assert_eq!(state.session.shard_id, 2);
        assert_eq!(state.session.shard_count, 4);
    }

    #[test]
    fn gateway_frame_state_clears_invalid_session() {
        let mut state = GatewayFrameState::new(QqBotGatewaySession {
            session_id: Some("session-1".to_string()),
            last_sequence: Some(99),
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        });

        let step = state.apply_event(GatewayEvent {
            opcode: OP_INVALID_SESSION,
            sequence: None,
            event_type: None,
            data: Value::Null,
        });

        assert!(matches!(step, GatewayStep::InvalidSession));
        assert_eq!(state.session.session_id, None);
        assert_eq!(state.session.last_sequence, None);
    }

    #[test]
    fn gateway_frame_state_tracks_heartbeat_ack() {
        let mut state = GatewayFrameState::new(QqBotGatewaySession {
            session_id: None,
            last_sequence: None,
            intents: 1,
            shard_id: 0,
            shard_count: 1,
        });

        let step = state.apply_event(GatewayEvent {
            opcode: OP_HEARTBEAT,
            sequence: None,
            event_type: None,
            data: Value::Null,
        });
        assert!(matches!(step, GatewayStep::RemoteHeartbeat));
        assert!(state.awaiting_heartbeat_ack);

        let step = state.apply_event(GatewayEvent {
            opcode: OP_HEARTBEAT_ACK,
            sequence: None,
            event_type: None,
            data: Value::Null,
        });
        assert!(matches!(step, GatewayStep::HeartbeatAck));
        assert!(!state.awaiting_heartbeat_ack);
    }
}
