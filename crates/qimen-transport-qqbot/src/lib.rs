use bytes::Bytes;
use futures_util::stream::{self, StreamExt, TryStreamExt};
use md5::Md5;
use qimen_error::{QimenError, Result};
use qimen_transport_ws::OneBot11ForwardWsClient;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::Sha1;
use std::fmt;
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
const MAX_UPLOAD_BYTES: u64 = 200_000_000;
const MD5_10M_BYTES: usize = 10_002_432;
const MAX_UPLOAD_CONCURRENCY: u64 = 8;
const MAX_PART_RETRY_SECONDS: u64 = 60;

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
    expires_in: Value,
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
    pub is_wakeup: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub markdown: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ark: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embed: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_notify: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<Value>,
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
            is_wakeup: None,
            markdown: None,
            keyboard: None,
            ark: None,
            embed: None,
            card: None,
            input_notify: None,
            message_reference: None,
            media: None,
            image: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadFilePayload {
    pub file_type: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub srv_send_msg: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_id: Option<String>,
}

/// Request body for QQ's local-file pre-upload endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct UploadPreparePayload {
    pub file_type: i64,
    pub file_size: String,
    pub file_name: String,
    pub md5: String,
    pub sha1: String,
    pub md5_10m: String,
}

/// A pre-signed object-storage upload slot returned by QQ.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadPart {
    pub index: u64,
    pub presigned_url: String,
    #[serde(deserialize_with = "deserialize_u64")]
    pub block_size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadConfig {
    #[serde(
        default = "default_upload_concurrency",
        deserialize_with = "deserialize_u64"
    )]
    pub concurrency: u64,
    #[serde(
        default = "default_retry_timeout",
        deserialize_with = "deserialize_u64"
    )]
    pub retry_timeout: u64,
    #[serde(default = "default_retry_delay", deserialize_with = "deserialize_u64")]
    pub retry_delay: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            concurrency: default_upload_concurrency(),
            retry_timeout: default_retry_timeout(),
            retry_delay: default_retry_delay(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UploadPrepareResponse {
    pub upload_id: String,
    #[serde(deserialize_with = "deserialize_u64")]
    pub block_size: u64,
    #[serde(default)]
    pub parts: Vec<UploadPart>,
    #[serde(default)]
    pub upload_config: Option<UploadConfig>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadPartFinishPayload {
    pub upload_id: String,
    pub part_index: u64,
    pub block_size: String,
    pub md5: String,
}

/// Bytes used by the channel/DMS multipart message APIs.
#[derive(Debug, Clone)]
pub struct LocalImagePayload {
    pub data: Bytes,
    pub file_name: String,
    pub content_type: String,
}

#[derive(Debug, Clone)]
struct PlannedUploadPart {
    index: u64,
    presigned_url: String,
    data: Bytes,
    md5: String,
}

fn default_upload_concurrency() -> u64 {
    1
}

fn default_retry_timeout() -> u64 {
    300
}

fn default_retry_delay() -> u64 {
    1
}

fn deserialize_u64<'de, D>(deserializer: D) -> std::result::Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    struct U64Visitor;

    impl<'de> Visitor<'de> for U64Visitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("an unsigned integer or a decimal string")
        }

        fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            u64::try_from(value).map_err(|_| E::custom("expected a non-negative integer"))
        }

        fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            value
                .parse::<u64>()
                .map_err(|_| E::custom("expected a decimal integer string"))
        }

        fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U64Visitor)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QqBotApiError {
    pub path: String,
    pub status: u16,
    pub code: Option<i64>,
    pub message: String,
    pub category: QqBotApiErrorCategory,
    pub retry_after_ms: Option<u64>,
    pub trace_id: Option<String>,
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
        if let Some(trace_id) = self.trace_id.as_deref() {
            write!(f, ", trace_id={trace_id}")?;
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

    pub async fn post_channel_message_multipart(
        &self,
        channel_id: &str,
        payload: &SendMessagePayload,
        image: LocalImagePayload,
    ) -> Result<Value> {
        self.post_multipart_message(&format!("/channels/{channel_id}/messages"), payload, image)
            .await
    }

    pub async fn post_dms_message_multipart(
        &self,
        guild_id: &str,
        payload: &SendMessagePayload,
        image: LocalImagePayload,
    ) -> Result<Value> {
        self.post_multipart_message(&format!("/dms/{guild_id}/messages"), payload, image)
            .await
    }

    pub async fn prepare_group_file_upload(
        &self,
        group_openid: &str,
        payload: &UploadPreparePayload,
    ) -> Result<UploadPrepareResponse> {
        let value: Value = self
            .post_json(
                &format!("/v2/groups/{group_openid}/upload_prepare"),
                payload,
            )
            .await?;
        serde_json::from_value(value).map_err(QimenError::Json)
    }

    pub async fn prepare_c2c_file_upload(
        &self,
        openid: &str,
        payload: &UploadPreparePayload,
    ) -> Result<UploadPrepareResponse> {
        let value: Value = self
            .post_json(&format!("/v2/users/{openid}/upload_prepare"), payload)
            .await?;
        serde_json::from_value(value).map_err(QimenError::Json)
    }

    pub async fn finish_group_file_part(
        &self,
        group_openid: &str,
        payload: &UploadPartFinishPayload,
    ) -> Result<Value> {
        self.post_json(
            &format!("/v2/groups/{group_openid}/upload_part_finish"),
            payload,
        )
        .await
    }

    pub async fn finish_c2c_file_part(
        &self,
        openid: &str,
        payload: &UploadPartFinishPayload,
    ) -> Result<Value> {
        self.post_json(&format!("/v2/users/{openid}/upload_part_finish"), payload)
            .await
    }

    /// Upload local bytes through QQ's pre-signed multipart flow, then merge them into a group file.
    pub async fn post_group_file_bytes(
        &self,
        group_openid: &str,
        file_type: i64,
        file_name: &str,
        data: Bytes,
        srv_send_msg: bool,
    ) -> Result<Value> {
        self.post_file_bytes(
            "group_file",
            group_openid,
            file_type,
            file_name,
            data,
            srv_send_msg,
        )
        .await
    }

    /// Upload local bytes through QQ's pre-signed multipart flow, then merge them into a C2C file.
    pub async fn post_c2c_file_bytes(
        &self,
        openid: &str,
        file_type: i64,
        file_name: &str,
        data: Bytes,
        srv_send_msg: bool,
    ) -> Result<Value> {
        self.post_file_bytes("c2c_file", openid, file_type, file_name, data, srv_send_msg)
            .await
    }

    /// Upload a single part to the pre-signed URL. QQ credentials are intentionally omitted.
    pub async fn put_presigned_upload_part(&self, url: &str, data: Bytes) -> Result<()> {
        let response = self.http.put(url).body(data).send().await.map_err(|err| {
            QimenError::Transport(format!(
                "qqbot pre-signed upload PUT failed: {}",
                reqwest_error_kind(&err)
            ))
        })?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        let body = body.trim().chars().take(512).collect::<String>();
        Err(QimenError::Transport(format!(
            "qqbot pre-signed upload PUT failed with HTTP {}: {}",
            status.as_u16(),
            body
        )))
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

    pub async fn recall_group_message(
        &self,
        group_openid: &str,
        message_id: &str,
    ) -> Result<Value> {
        self.delete_json(
            &format!("/v2/groups/{group_openid}/messages/{message_id}"),
            &[],
        )
        .await
    }

    pub async fn recall_c2c_message(&self, openid: &str, message_id: &str) -> Result<Value> {
        self.delete_json(&format!("/v2/users/{openid}/messages/{message_id}"), &[])
            .await
    }

    pub async fn recall_dms_message(
        &self,
        guild_id: &str,
        message_id: &str,
        hidetip: bool,
    ) -> Result<Value> {
        self.delete_json(
            &format!("/dms/{guild_id}/messages/{message_id}"),
            &[("hidetip", hidetip.to_string())],
        )
        .await
    }

    pub async fn acknowledge_interaction(&self, interaction_id: &str, code: i64) -> Result<Value> {
        self.put_json(
            &format!("/interactions/{interaction_id}"),
            &json!({ "code": code }),
        )
        .await
    }

    async fn post_file_bytes(
        &self,
        route: &str,
        target_id: &str,
        file_type: i64,
        file_name: &str,
        data: Bytes,
        srv_send_msg: bool,
    ) -> Result<Value> {
        if !(1..=4).contains(&file_type) {
            return Err(QimenError::Protocol(format!(
                "qqbot file_type must be between 1 and 4, got {file_type}"
            )));
        }
        if data.is_empty() {
            return Err(QimenError::Protocol(
                "qqbot local media cannot be empty".to_string(),
            ));
        }
        let file_size = u64::try_from(data.len()).map_err(|_| {
            QimenError::Protocol("qqbot local media size does not fit into u64".to_string())
        })?;
        if file_size > MAX_UPLOAD_BYTES {
            return Err(QimenError::Protocol(format!(
                "qqbot local media exceeds the 200 MB hard limit ({file_size} bytes)"
            )));
        }
        let file_name = sanitize_file_name(file_name);
        let digest_data = data.clone();
        let (md5, sha1, md5_10m) = tokio::task::spawn_blocking(move || {
            (
                digest_hex::<Md5>(&digest_data),
                digest_hex::<Sha1>(&digest_data),
                digest_hex::<Md5>(&digest_data[..digest_data.len().min(MD5_10M_BYTES)]),
            )
        })
        .await
        .map_err(|_| QimenError::Transport("qqbot local media hashing task failed".to_string()))?;
        let prepare_payload = UploadPreparePayload {
            file_type,
            file_size: file_size.to_string(),
            file_name: file_name.clone(),
            md5,
            sha1,
            md5_10m,
        };

        let prepared = match route {
            "group_file" => {
                self.prepare_group_file_upload(target_id, &prepare_payload)
                    .await?
            }
            "c2c_file" => {
                self.prepare_c2c_file_upload(target_id, &prepare_payload)
                    .await?
            }
            _ => {
                return Err(QimenError::Protocol(format!(
                    "unsupported qqbot local upload route '{route}'"
                )));
            }
        };
        if prepared.upload_id.trim().is_empty() {
            return Err(QimenError::Protocol(
                "qqbot upload_prepare returned an empty upload_id".to_string(),
            ));
        }
        let prepared_for_plan = prepared.clone();
        let data_for_plan = data.clone();
        let parts = tokio::task::spawn_blocking(move || {
            plan_upload_parts(&prepared_for_plan, data_for_plan)
        })
        .await
        .map_err(|_| {
            QimenError::Transport("qqbot upload part hashing task failed".to_string())
        })??;
        let config = prepared.upload_config.clone().unwrap_or_default();
        let concurrency = config.concurrency.clamp(1, MAX_UPLOAD_CONCURRENCY) as usize;
        let retry_timeout = config.retry_timeout.clamp(1, MAX_PART_RETRY_SECONDS);
        let total_upload_timeout = config.retry_timeout.clamp(1, 300);
        let retry_delay = config.retry_delay.min(5);
        let upload_id = prepared.upload_id.clone();

        let upload_parts = stream::iter(parts.into_iter().map(|part| {
            let upload_id = upload_id.clone();
            async move {
                self.upload_and_finish_part(
                    route,
                    target_id,
                    &upload_id,
                    part,
                    retry_timeout,
                    retry_delay,
                )
                .await
            }
        }))
        .buffer_unordered(concurrency)
        .try_collect::<Vec<_>>();
        tokio::time::timeout(Duration::from_secs(total_upload_timeout), upload_parts)
            .await
            .map_err(|_| {
                QimenError::Transport(format!(
                    "qqbot upload timed out after {total_upload_timeout} seconds"
                ))
            })??;

        let payload = UploadFilePayload {
            file_type,
            url: None,
            srv_send_msg,
            file_name: Some(file_name),
            upload_id: Some(upload_id),
        };
        match route {
            "group_file" => self.post_group_file(target_id, &payload).await,
            "c2c_file" => self.post_c2c_file(target_id, &payload).await,
            _ => unreachable!(),
        }
    }

    async fn upload_and_finish_part(
        &self,
        route: &str,
        target_id: &str,
        upload_id: &str,
        part: PlannedUploadPart,
        retry_timeout_secs: u64,
        retry_delay_secs: u64,
    ) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(retry_timeout_secs);
        let mut attempt = 0_u32;
        let mut last_error = None;
        loop {
            attempt += 1;
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() && attempt > 1 {
                break;
            }
            let timeout = remaining
                .min(self.config.timeout)
                .max(Duration::from_millis(1));
            let put_result = tokio::time::timeout(
                timeout,
                self.put_presigned_upload_part(&part.presigned_url, part.data.clone()),
            )
            .await
            .map_err(|_| {
                QimenError::Transport(format!(
                    "qqbot pre-signed upload part {} timed out",
                    part.index
                ))
            })
            .and_then(|result| result);

            match put_result {
                Ok(()) => {
                    let finish_payload = UploadPartFinishPayload {
                        upload_id: upload_id.to_string(),
                        part_index: part.index,
                        block_size: part.data.len().to_string(),
                        md5: part.md5.clone(),
                    };
                    let finish_result = match route {
                        "group_file" => {
                            self.finish_group_file_part(target_id, &finish_payload)
                                .await
                        }
                        "c2c_file" => self.finish_c2c_file_part(target_id, &finish_payload).await,
                        _ => unreachable!(),
                    };
                    match finish_result {
                        Ok(_) => return Ok(()),
                        Err(err) => last_error = Some(err),
                    }
                }
                Err(err) => last_error = Some(err),
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if attempt >= 3 || remaining.is_zero() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(retry_delay_secs).min(remaining)).await;
        }

        Err(last_error.unwrap_or_else(|| {
            QimenError::Transport(format!(
                "qqbot upload part {} failed after retries",
                part.index
            ))
        }))
    }

    async fn post_multipart_message(
        &self,
        path: &str,
        payload: &SendMessagePayload,
        image: LocalImagePayload,
    ) -> Result<Value> {
        let token = self.bot_authorization().await?;
        let mut fields = serde_json::to_value(payload)?;
        if let Some(fields) = fields.as_object_mut() {
            fields.remove("message_reference");
        }
        let file_name = sanitize_file_name(&image.file_name);
        let image_len = u64::try_from(image.data.len()).map_err(|_| {
            QimenError::Protocol("QQ multipart image size does not fit into u64".to_string())
        })?;
        if tracing::enabled!(target: "qimen_raw_message", tracing::Level::DEBUG) {
            let mut logged = fields.clone();
            if let Some(logged) = logged.as_object_mut() {
                logged.insert(
                    "file_image".to_string(),
                    json!({
                        "file_name": file_name.clone(),
                        "content_type": image.content_type.clone(),
                        "size": image_len,
                        "data": "<redacted>",
                    }),
                );
            }
            tracing::debug!(
                target: "qimen_raw_message",
                direction = "outbound",
                protocol = "qq-official",
                transport = "http-api-multipart",
                appid = %self.config.appid,
                path,
                message = %logged,
            );
        }
        let mut form = reqwest::multipart::Form::new();
        if let Some(fields) = fields.as_object() {
            for (key, value) in fields {
                if value.is_null() {
                    continue;
                }
                let text = match value {
                    Value::String(value) => value.clone(),
                    _ => serde_json::to_string(value)?,
                };
                form = form.text(key.clone(), text);
            }
        }
        let part = reqwest::multipart::Part::stream_with_length(image.data.clone(), image_len)
            .file_name(file_name)
            .mime_str(&image.content_type)
            .map_err(|err| {
                QimenError::Protocol(format!("invalid QQ multipart media content type: {err}"))
            })?;
        form = form.part("file_image", part);

        let response = self
            .http
            .post(format!("{}{}", self.config.base_url(), path))
            .header("Authorization", token)
            .header("X-Union-Appid", self.config.appid.as_str())
            .multipart(form)
            .send()
            .await
            .map_err(|err| {
                QimenError::Transport(format!("qqbot POST multipart {path} failed: {err}"))
            })?;

        decode_response(response, path).await
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
        let ttl_secs = parsed
            .expires_in
            .as_u64()
            .or_else(|| parsed.expires_in.as_str()?.parse::<u64>().ok())
            .ok_or_else(|| {
                QimenError::Transport(format!(
                    "qqbot token response has invalid expires_in '{}'",
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
        if is_message_api_path(path)
            && tracing::enabled!(target: "qimen_raw_message", tracing::Level::DEBUG)
            && let Ok(raw_payload) = serde_json::to_string(payload)
        {
            tracing::debug!(
                target: "qimen_raw_message",
                direction = "outbound",
                protocol = "qq-official",
                transport = "http-api",
                appid = %self.config.appid,
                path,
                message = %raw_payload,
            );
        }
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

    async fn put_json<T>(&self, path: &str, payload: &T) -> Result<Value>
    where
        T: Serialize + ?Sized,
    {
        let token = self.bot_authorization().await?;
        let response = self
            .http
            .put(format!("{}{}", self.config.base_url(), path))
            .header("Authorization", token)
            .header("X-Union-Appid", self.config.appid.as_str())
            .json(payload)
            .send()
            .await
            .map_err(|err| QimenError::Transport(format!("qqbot PUT {path} failed: {err}")))?;

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

fn is_message_api_path(path: &str) -> bool {
    path.contains("/messages") || path.ends_with("/files")
}

fn digest_hex<D>(data: &[u8]) -> String
where
    D: md5::Digest + Default,
{
    let mut digest = D::default();
    digest.update(data);
    digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn plan_upload_parts(
    prepared: &UploadPrepareResponse,
    data: Bytes,
) -> Result<Vec<PlannedUploadPart>> {
    if prepared.parts.is_empty() {
        // A hash hit can complete as an instant upload with no bytes to PUT.
        return Ok(Vec::new());
    }
    let mut parts = prepared.parts.clone();
    parts.sort_by_key(|part| part.index);
    let mut offset = 0_usize;
    let mut planned = Vec::with_capacity(parts.len());
    for (expected_index, part) in parts.into_iter().enumerate() {
        if part.index != expected_index as u64 {
            return Err(QimenError::Protocol(format!(
                "qqbot upload parts must use contiguous indexes from 0; expected {expected_index}, got {}",
                part.index
            )));
        }
        if part.presigned_url.trim().is_empty() {
            return Err(QimenError::Protocol(format!(
                "qqbot upload part {} has an empty presigned_url",
                part.index
            )));
        }
        let remaining = data.len().saturating_sub(offset);
        if remaining == 0 {
            return Err(QimenError::Protocol(
                "qqbot upload_prepare returned more parts than the file requires".to_string(),
            ));
        }
        let requested = if part.block_size == 0 {
            prepared.block_size
        } else {
            part.block_size
        };
        if requested == 0 {
            return Err(QimenError::Protocol(format!(
                "qqbot upload part {} has an invalid block_size",
                part.index
            )));
        }
        let length = remaining.min(usize::try_from(requested).map_err(|_| {
            QimenError::Protocol("qqbot upload block_size does not fit into usize".to_string())
        })?);
        planned.push(PlannedUploadPart {
            index: part.index,
            presigned_url: part.presigned_url,
            data: data.slice(offset..offset + length),
            md5: digest_hex::<Md5>(&data[offset..offset + length]),
        });
        offset += length;
    }
    if offset != data.len() {
        return Err(QimenError::Protocol(format!(
            "qqbot upload parts cover {offset} bytes, but the file has {} bytes",
            data.len()
        )));
    }
    Ok(planned)
}

fn sanitize_file_name(file_name: &str) -> String {
    let file_name = file_name.trim();
    if file_name.is_empty() {
        return "qimenbot-media.bin".to_string();
    }
    let base_name = file_name
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("qimenbot-media.bin");
    let sanitized = base_name
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '"' | '\'' | ';') {
                '_'
            } else {
                character
            }
        })
        .take(255)
        .collect::<String>();
    if sanitized.is_empty() {
        "qimenbot-media.bin".to_string()
    } else {
        sanitized
    }
}

fn reqwest_error_kind(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_request() {
        "request could not be built or sent"
    } else if error.is_body() {
        "request body failed"
    } else {
        "network error"
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
        .or_else(|| parsed.get("err_code"))
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
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str()?.parse::<u64>().ok())
        });
    let trace_id = parsed
        .get("trace_id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);

    QqBotApiError {
        path: path.to_string(),
        status,
        code,
        category: classify_api_error(status, code, &message),
        message,
        retry_after_ms,
        trace_id,
    }
}

fn classify_api_error(status: u16, code: Option<i64>, message: &str) -> QqBotApiErrorCategory {
    let lower = message.to_ascii_lowercase();
    if status == 401
        || status == 403 && lower.contains("token")
        || matches!(
            code,
            Some(100007 | 100016 | 11241 | 11242 | 11243 | 11251 | 11252 | 11261)
        )
    {
        return QqBotApiErrorCategory::Authentication;
    }
    if status == 429
        || lower.contains("rate")
        || lower.contains("frequency")
        || lower.contains("频控")
        || lower.contains("限频")
        || matches!(
            code,
            Some(
                100001
                    | 20028
                    | 304019
                    | 304035
                    | 304045
                    | 304047
                    | 304049
                    | 304050
                    | 40034100
                    | 1100100
            )
        )
    {
        return QqBotApiErrorCategory::RateLimited;
    }
    if status == 403
        || matches!(
            code,
            Some(11253 | 11254 | 11264 | 304004 | 304036 | 304037 | 40034105 | 40062003 | 306004)
        )
    {
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
    QqBotApiErrorCategory::Unknown
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
    #[serde(rename = "id")]
    pub event_id: Option<String>,
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
        match event.opcode {
            OP_DISPATCH => {
                if event.event_type.as_deref() == Some("READY") {
                    self.session.apply_ready_data(&event.data);
                }
                GatewayStep::Dispatch(event)
            }
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

    /// Commit a dispatch sequence after the application has handled the event successfully.
    pub fn acknowledge_dispatch(&mut self, sequence: Option<i64>) {
        if let Some(sequence) = sequence {
            self.session.last_sequence = Some(sequence);
        }
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
                Ok(Some(GatewayStep::RemoteHeartbeat))
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
            && let (Some(shard_id), Some(shard_count)) = (shard[0].as_u64(), shard[1].as_u64())
            && shard_count > 0
            && shard_id < shard_count
        {
            self.shard_id = shard_id;
            self.shard_count = shard_count;
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

    #[test]
    fn raw_logging_filters_message_api_paths() {
        assert!(is_message_api_path("/v2/groups/group-1/messages"));
        assert!(is_message_api_path("/v2/users/user-1/files"));
        assert!(!is_message_api_path("/gateway/bot"));
        assert!(!is_message_api_path("/interactions/event-1"));
    }

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
        let base_url = format!("http://{addr}");
        let response_base_url = base_url.clone();

        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _peer)) = listener.accept().await else {
                    break;
                };
                let tx = tx.clone();
                let response_base_url = response_base_url.clone();
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
                    let response = mock_response(&request.path, &response_base_url);
                    let _ = tx.send(request).await;
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });

        (base_url, rx)
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

    fn mock_response(path: &str, base_url: &str) -> String {
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
            "/v2/groups/group-instant/files" => json!({
                "file_uuid": "instant-file-uuid",
                "file_info": "instant-file-info",
                "ttl": 3600,
            }),
            "/v2/groups/group-1/upload_prepare" | "/v2/users/user-1/upload_prepare" => json!({
                "upload_id": "upload-1",
                "block_size": "4",
                "parts": [
                    {
                        "index": 0,
                        "presigned_url": format!("{base_url}/presigned/0"),
                        "block_size": "4",
                    },
                    {
                        "index": 1,
                        "presigned_url": format!("{base_url}/presigned/1"),
                        "block_size": "4",
                    },
                ],
                "upload_config": {
                    "concurrency": 2,
                    "retry_timeout": 5,
                    "retry_delay": 0,
                },
            }),
            "/v2/groups/group-instant/upload_prepare" => json!({
                "upload_id": "instant-upload",
                "block_size": "4",
                "parts": [],
                "upload_config": {
                    "concurrency": 1,
                    "retry_timeout": 5,
                    "retry_delay": 0,
                },
            }),
            "/v2/groups/group-1/upload_part_finish"
            | "/v2/users/user-1/upload_part_finish"
            | "/presigned/0"
            | "/presigned/1" => json!({}),
            "/channels/channel-1/messages/message-1?hidetip=true"
            | "/v2/groups/group-1/messages/message-1"
            | "/v2/users/user-1/messages/message-1"
            | "/dms/guild-1/messages/message-1?hidetip=false"
            | "/interactions/interaction-1" => Value::Null,
            "/rate-limited" => json!({
                "err_code": 40034100,
                "message": "rate limit exceeded",
                "retry_after": 1000,
                "trace_id": "trace-1",
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
            is_wakeup: Some(false),
            markdown: Some(json!({ "content": "# Title" })),
            keyboard: Some(json!({ "id": "keyboard-template" })),
            ark: Some(json!({ "template_id": 37 })),
            embed: Some(json!({ "title": "embed" })),
            card: Some(json!({ "type": "tuwen" })),
            input_notify: Some(json!({ "input_type": 1, "input_second": 30 })),
            message_reference: Some(json!({ "message_id": "quoted-message" })),
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
                "is_wakeup": false,
                "markdown": { "content": "# Title" },
                "keyboard": { "id": "keyboard-template" },
                "ark": { "template_id": 37 },
                "embed": { "title": "embed" },
                "card": { "type": "tuwen" },
                "input_notify": { "input_type": 1, "input_second": 30 },
                "message_reference": { "message_id": "quoted-message" },
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
            url: Some("https://example.invalid/a.png".to_string()),
            srv_send_msg: false,
            file_name: None,
            upload_id: None,
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
    async fn openapi_uploads_local_bytes_through_prepare_parts_and_merge() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();

        let response = client
            .post_group_file_bytes(
                "group-1",
                4,
                "example.bin",
                Bytes::from_static(b"abcdefgh"),
                false,
            )
            .await
            .unwrap();

        assert_eq!(
            response.get("file_info").and_then(Value::as_str),
            Some("file-info")
        );
        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");

        let mut recorded = Vec::new();
        for _ in 0..6 {
            recorded.push(requests.recv().await.unwrap());
        }
        let prepare = recorded
            .iter()
            .find(|request| request.path == "/v2/groups/group-1/upload_prepare")
            .unwrap();
        let prepare_body = serde_json::from_str::<Value>(&prepare.body).unwrap();
        assert_eq!(
            prepare_body.get("file_type").and_then(Value::as_i64),
            Some(4)
        );
        assert_eq!(
            prepare_body.get("file_size").and_then(Value::as_str),
            Some("8")
        );
        assert_eq!(
            prepare_body.get("file_name").and_then(Value::as_str),
            Some("example.bin")
        );
        assert_eq!(
            prepare_body.get("md5").and_then(Value::as_str),
            Some("e8dc4081b13434b45189a720b77b6818")
        );
        assert_eq!(
            prepare_body.get("sha1").and_then(Value::as_str),
            Some("425af12a0743502b322e93a015bcf868e324d56a")
        );
        assert_eq!(prepare_body.get("md5"), prepare_body.get("md5_10m"));

        let mut uploaded_chunks = recorded
            .iter()
            .filter(|request| request.path.starts_with("/presigned/"))
            .collect::<Vec<_>>();
        uploaded_chunks.sort_by(|left, right| left.path.cmp(&right.path));
        assert_eq!(uploaded_chunks.len(), 2);
        assert_eq!(uploaded_chunks[0].method, "PUT");
        assert_eq!(uploaded_chunks[0].body, "abcd");
        assert_eq!(uploaded_chunks[1].body, "efgh");
        for request in uploaded_chunks {
            assert!(header_value(request, "authorization").is_none());
            assert!(header_value(request, "x-union-appid").is_none());
        }

        let finish = recorded
            .iter()
            .filter(|request| request.path == "/v2/groups/group-1/upload_part_finish")
            .collect::<Vec<_>>();
        assert_eq!(finish.len(), 2);
        for request in finish {
            let body = serde_json::from_str::<Value>(&request.body).unwrap();
            assert_eq!(
                body.get("upload_id").and_then(Value::as_str),
                Some("upload-1")
            );
            assert_eq!(body.get("block_size").and_then(Value::as_str), Some("4"));
            assert!(body.get("part_index").and_then(Value::as_u64).is_some());
            assert_eq!(body.get("md5").and_then(Value::as_str).unwrap().len(), 32);
        }

        let merge = recorded
            .iter()
            .find(|request| request.path == "/v2/groups/group-1/files")
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&merge.body).unwrap(),
            json!({
                "file_type": 4,
                "srv_send_msg": false,
                "file_name": "example.bin",
                "upload_id": "upload-1",
            })
        );
    }

    #[tokio::test]
    async fn openapi_merges_instant_upload_without_putting_parts() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();

        let response = client
            .post_group_file_bytes(
                "group-instant",
                1,
                "cached.png",
                Bytes::from_static(b"cached-image"),
                false,
            )
            .await
            .unwrap();

        assert_eq!(
            response.get("file_info").and_then(Value::as_str),
            Some("instant-file-info")
        );
        assert_eq!(
            requests.recv().await.unwrap().path,
            "/app/getAppAccessToken"
        );
        assert_eq!(
            requests.recv().await.unwrap().path,
            "/v2/groups/group-instant/upload_prepare"
        );
        let merge = requests.recv().await.unwrap();
        assert_eq!(merge.path, "/v2/groups/group-instant/files");
        assert_eq!(
            serde_json::from_str::<Value>(&merge.body).unwrap(),
            json!({
                "file_type": 1,
                "srv_send_msg": false,
                "file_name": "cached.png",
                "upload_id": "instant-upload",
            })
        );
        assert!(requests.try_recv().is_err());
    }

    #[tokio::test]
    async fn openapi_posts_channel_and_dms_images_as_multipart() {
        let (base_url, mut requests) = spawn_mock_server().await;
        let client = QqBotOpenApiClient::new(mock_config(base_url)).unwrap();
        let payload = SendMessagePayload {
            msg_type: None,
            content: Some("hello".to_string()),
            msg_id: Some("message-1".to_string()),
            msg_seq: None,
            event_id: None,
            is_wakeup: None,
            markdown: None,
            keyboard: None,
            ark: None,
            embed: None,
            card: None,
            input_notify: None,
            message_reference: Some(json!({ "message_id": "ignored" })),
            media: None,
            image: None,
        };
        let image = LocalImagePayload {
            data: Bytes::from_static(b"image-bytes"),
            file_name: "image.png".to_string(),
            content_type: "image/png".to_string(),
        };

        client
            .post_channel_message_multipart("channel-1", &payload, image.clone())
            .await
            .unwrap();
        client
            .post_dms_message_multipart("guild-1", &payload, image)
            .await
            .unwrap();

        let token_request = requests.recv().await.unwrap();
        assert_eq!(token_request.path, "/app/getAppAccessToken");
        for expected_path in ["/channels/channel-1/messages", "/dms/guild-1/messages"] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request.method, "POST");
            assert_eq!(request.path, expected_path);
            assert_eq!(
                header_value(&request, "authorization"),
                Some("QQBot mock-token")
            );
            assert!(
                header_value(&request, "content-type")
                    .is_some_and(|value| value.starts_with("multipart/form-data; boundary="))
            );
            assert!(request.body.contains("name=\"content\""));
            assert!(request.body.contains("hello"));
            assert!(request.body.contains("name=\"msg_id\""));
            assert!(request.body.contains("message-1"));
            assert!(request.body.contains("name=\"file_image\""));
            assert!(request.body.contains("filename=\"image.png\""));
            assert!(request.body.contains("Content-Type: image/png"));
            assert!(request.body.contains("image-bytes"));
            assert!(!request.body.contains("message_reference"));
        }
    }

    #[tokio::test]
    async fn openapi_recalls_messages_and_acknowledges_interaction() {
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

        client
            .recall_group_message("group-1", "message-1")
            .await
            .unwrap();
        client
            .recall_c2c_message("user-1", "message-1")
            .await
            .unwrap();
        client
            .recall_dms_message("guild-1", "message-1", false)
            .await
            .unwrap();
        client
            .acknowledge_interaction("interaction-1", 0)
            .await
            .unwrap();

        for (method, path) in [
            ("DELETE", "/v2/groups/group-1/messages/message-1"),
            ("DELETE", "/v2/users/user-1/messages/message-1"),
            ("DELETE", "/dms/guild-1/messages/message-1?hidetip=false"),
            ("PUT", "/interactions/interaction-1"),
        ] {
            let request = requests.recv().await.unwrap();
            assert_eq!(request.method, method);
            assert_eq!(request.path, path);
            if method == "PUT" {
                assert_eq!(
                    serde_json::from_str::<Value>(&request.body).unwrap(),
                    json!({ "code": 0 })
                );
            }
        }
    }

    #[test]
    fn api_error_classifies_rate_limit_and_permission() {
        let rate_limit = build_api_error(
            "/v2/users/user-1/messages",
            429,
            r#"{"err_code":40034100,"message":"rate limit exceeded","retry_after":"1000","trace_id":"trace-1"}"#,
        );
        let permission = build_api_error(
            "/channels/channel-1/messages/message-1",
            400,
            r#"{"err_code":304004,"message":"ARK_NOT_ALLOWED"}"#,
        );

        assert_eq!(rate_limit.category, QqBotApiErrorCategory::RateLimited);
        assert_eq!(rate_limit.retry_after_ms, Some(1000));
        assert_eq!(rate_limit.code, Some(40034100));
        assert_eq!(rate_limit.trace_id.as_deref(), Some("trace-1"));
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

        assert!(matches!(
            step,
            GatewayStep::Dispatch(GatewayEvent {
                event_type: Some(ref event_type),
                ..
            }) if event_type == "READY"
        ));
        assert_eq!(
            client.session().session_id.as_deref(),
            Some("session-ready")
        );
        assert_eq!(client.session().last_sequence, None);
        client.acknowledge_dispatch(Some(7));
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
    async fn gateway_remote_heartbeat_is_answered_and_surfaced() {
        let (endpoint, mut sent_payloads) = spawn_mock_gateway(vec![json!({
            "op": OP_HEARTBEAT,
        })])
        .await;
        let mut client = QqBotGatewayClient::connect(&endpoint, gateway_session(), "QQBot token")
            .await
            .unwrap();
        let _identify = recv_gateway_payload(&mut sent_payloads).await;

        let step = client.next_step().await.unwrap().unwrap();
        let heartbeat = recv_gateway_payload(&mut sent_payloads).await;

        assert!(matches!(step, GatewayStep::RemoteHeartbeat));
        assert_eq!(heartbeat, json!({ "op": OP_HEARTBEAT, "d": null }));
        assert!(client.should_reconnect_for_missing_ack());
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

        session.apply_ready_data(&json!({ "shard": [0, 0] }));
        assert_eq!(session.shard_id, 1);
        assert_eq!(session.shard_count, 4);
    }

    #[test]
    fn gateway_event_parses_opcode_and_sequence() {
        let event = parse_gateway_event(
            r#"{"op":0,"s":12,"t":"GROUP_AT_MESSAGE_CREATE","id":"event-1","d":{"id":"msg"}}"#,
        )
        .unwrap();

        assert_eq!(event.opcode, OP_DISPATCH);
        assert_eq!(event.sequence, Some(12));
        assert_eq!(event.event_type.as_deref(), Some("GROUP_AT_MESSAGE_CREATE"));
        assert_eq!(event.event_id.as_deref(), Some("event-1"));
        assert_eq!(event.data.get("id").and_then(Value::as_str), Some("msg"));
    }

    #[test]
    fn gateway_event_allows_missing_data_for_control_frames() {
        let event = parse_gateway_event(r#"{"op":11}"#).unwrap();

        assert_eq!(event.opcode, OP_HEARTBEAT_ACK);
        assert_eq!(event.sequence, None);
        assert_eq!(event.event_type, None);
        assert_eq!(event.event_id, None);
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
            event_id: None,
            data: json!({
                "session_id": "session-1",
                "shard": [2, 4],
            }),
        });

        assert!(matches!(
            step,
            GatewayStep::Dispatch(GatewayEvent {
                event_type: Some(ref event_type),
                ..
            }) if event_type == "READY"
        ));
        assert_eq!(state.session.last_sequence, None);
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
            event_id: None,
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
            event_id: None,
            data: Value::Null,
        });
        assert!(matches!(step, GatewayStep::RemoteHeartbeat));
        assert!(state.awaiting_heartbeat_ack);

        let step = state.apply_event(GatewayEvent {
            opcode: OP_HEARTBEAT_ACK,
            sequence: None,
            event_type: None,
            event_id: None,
            data: Value::Null,
        });
        assert!(matches!(step, GatewayStep::HeartbeatAck));
        assert!(!state.awaiting_heartbeat_ack);
    }
}
