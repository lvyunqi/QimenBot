use qimen_error::{QimenError, Result};
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// HTTP API Client (outbound: bot -> OneBot implementation)
// ---------------------------------------------------------------------------

/// HTTP client for sending OneBot11 API actions.
///
/// Sends POST requests to `{base_url}/{action}` with JSON body.
pub struct OneBot11HttpClient {
    base_url: String,
    access_token: Option<String>,
    client: reqwest::Client,
}

impl OneBot11HttpClient {
    pub fn new(base_url: &str, access_token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token,
            client: reqwest::Client::new(),
        }
    }

    /// Send an action request via HTTP POST.
    ///
    /// POST `{base_url}/{action}` with the given params as JSON body.
    /// Returns the `data` field from the response on success.
    pub async fn send_action(&self, action: &str, params: Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, action);

        let mut request = self.client.post(&url).json(&params);

        if let Some(token) = self.access_token.as_deref().filter(|t| !t.is_empty()) {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request.send().await.map_err(|err| {
            QimenError::Transport(format!("http request to {url} failed: {err}"))
        })?;

        let status = response.status();
        let body = response.text().await.map_err(|err| {
            QimenError::Transport(format!("failed to read response body: {err}"))
        })?;

        if !status.is_success() {
            return Err(QimenError::Transport(format!(
                "http {status} from {url}: {body}"
            )));
        }

        let parsed: Value = serde_json::from_str(&body)?;

        let retcode = parsed.get("retcode").and_then(|v| v.as_i64()).unwrap_or(-1);
        if retcode != 0 {
            let msg = parsed
                .get("msg")
                .or_else(|| parsed.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(QimenError::Transport(format!(
                "action {action} failed (retcode={retcode}): {msg}"
            )));
        }

        Ok(parsed.get("data").cloned().unwrap_or(Value::Null))
    }
}

// ---------------------------------------------------------------------------
// HTTP Event Server (inbound: OneBot implementation -> bot)
// ---------------------------------------------------------------------------

/// Configuration for the HTTP event server.
#[derive(Debug, Clone)]
pub struct HttpEventServerConfig {
    pub host: String,
    pub port: u16,
    pub access_token: Option<String>,
}

/// HTTP server that receives OneBot11 event pushes.
///
/// The OneBot implementation POSTs JSON event payloads to this server.
pub struct OneBot11HttpEventServer {
    event_rx: mpsc::Receiver<String>,
    listener_task: JoinHandle<()>,
}

impl OneBot11HttpEventServer {
    pub async fn bind(config: HttpEventServerConfig) -> Result<Self> {
        let addr = format!("{}:{}", config.host, config.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!(address = %addr, "http event server listening");

        let (event_tx, event_rx) = mpsc::channel(128);

        let listener_task = tokio::spawn(http_accept_loop(
            listener,
            event_tx,
            config.access_token,
        ));

        Ok(Self {
            event_rx,
            listener_task,
        })
    }

    pub async fn next_event(&mut self) -> Option<String> {
        self.event_rx.recv().await
    }
}

impl Drop for OneBot11HttpEventServer {
    fn drop(&mut self) {
        self.listener_task.abort();
    }
}

// ---------------------------------------------------------------------------
// HTTP server internals
// ---------------------------------------------------------------------------

async fn http_accept_loop(
    listener: TcpListener,
    event_tx: mpsc::Sender<String>,
    access_token: Option<String>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::debug!(peer = %peer, "http-event: accepted connection");
                let event_tx = event_tx.clone();
                let access_token = access_token.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_http_connection(stream, access_token.as_deref(), event_tx).await
                    {
                        tracing::error!(peer = %peer, error = %err, "http-event: connection error");
                    }
                });
            }
            Err(err) => {
                tracing::error!(error = %err, "http-event: accept failed");
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_http_connection(
    mut stream: TcpStream,
    access_token: Option<&str>,
    event_tx: mpsc::Sender<String>,
) -> Result<()> {
    // Read the HTTP request headers. We read in a loop until we find the
    // header/body separator (\r\n\r\n).
    let mut buf = Vec::with_capacity(4096);
    let mut tmp = [0_u8; 4096];

    let header_end;
    loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Err(QimenError::Transport("connection closed before headers complete".to_string()));
        }
        buf.extend_from_slice(&tmp[..n]);

        if let Some(pos) = find_header_end(&buf) {
            header_end = pos;
            break;
        }

        if buf.len() > 64 * 1024 {
            write_response(&mut stream, 400, "Bad Request", "headers too large").await?;
            return Err(QimenError::Transport("headers too large".to_string()));
        }
    }

    let header_bytes = &buf[..header_end];
    let header_text = String::from_utf8_lossy(header_bytes);

    // Parse request line
    let first_line = header_text.lines().next().unwrap_or("");
    if !first_line.starts_with("POST ") {
        write_response(&mut stream, 405, "Method Not Allowed", "only POST is accepted").await?;
        return Err(QimenError::Transport(format!(
            "unexpected method: {first_line}"
        )));
    }

    // Check access token if configured
    if let Some(expected) = access_token.filter(|t| !t.is_empty()) {
        let authorized = header_text.lines().any(|line| {
            if let Some(colon_pos) = line.find(':') {
                let name = &line[..colon_pos];
                if !name.eq_ignore_ascii_case("authorization") {
                    return false;
                }
                let value = line[colon_pos + 1..].trim();
                if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
                    return &value[7..] == expected;
                }
            }
            false
        });

        if !authorized {
            write_response(&mut stream, 401, "Unauthorized", "").await?;
            return Err(QimenError::Transport("unauthorized".to_string()));
        }
    }

    // Extract Content-Length
    let content_length = header_text
        .lines()
        .find_map(|line| {
            if let Some(colon_pos) = line.find(':') {
                let name = &line[..colon_pos];
                if name.eq_ignore_ascii_case("content-length") {
                    return line[colon_pos + 1..].trim().parse::<usize>().ok();
                }
            }
            None
        })
        .unwrap_or(0);

    if content_length == 0 {
        write_response(&mut stream, 400, "Bad Request", "missing or zero Content-Length").await?;
        return Err(QimenError::Transport("missing body".to_string()));
    }

    if content_length > 16 * 1024 * 1024 {
        write_response(&mut stream, 413, "Payload Too Large", "").await?;
        return Err(QimenError::Transport("payload too large".to_string()));
    }

    // Read the body. Some of it may already be in `buf` after the headers.
    let body_start = header_end + 4; // skip \r\n\r\n
    let already_read = buf.len() - body_start;
    let mut body = Vec::with_capacity(content_length);
    body.extend_from_slice(&buf[body_start..]);

    if already_read < content_length {
        let remaining = content_length - already_read;
        let mut rest = vec![0_u8; remaining];
        stream.read_exact(&mut rest).await?;
        body.extend_from_slice(&rest);
    }

    // Truncate to content_length in case we read extra
    body.truncate(content_length);

    let payload = String::from_utf8(body).map_err(|err| {
        QimenError::Transport(format!("invalid UTF-8 body: {err}"))
    })?;

    // Validate it is valid JSON
    let _: Value = serde_json::from_str(&payload)?;

    // Send through channel
    if event_tx.send(payload).await.is_err() {
        tracing::warn!("http-event: event receiver dropped");
    }

    // 204 No Content
    write_response(&mut stream, 204, "No Content", "").await?;

    Ok(())
}

/// Find the position of \r\n\r\n in a byte buffer, returning the offset of
/// the first \r in the sequence.
fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    body: &str,
) -> Result<()> {
    let response = if body.is_empty() {
        format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
    } else {
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    };
    stream.write_all(response.as_bytes()).await?;
    Ok(())
}
