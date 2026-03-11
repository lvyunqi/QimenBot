use qimen_error::{QimenError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;

// ---------------------------------------------------------------------------
// Reconnect policy
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub stable_connection_threshold: Duration,
    pub idle_timeout: Duration,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            stable_connection_threshold: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(60),
        }
    }
}

impl ReconnectPolicy {
    pub fn next_delay(&self, current: Duration) -> Duration {
        std::cmp::min(current.saturating_mul(2), self.max_delay)
    }
}

// ---------------------------------------------------------------------------
// TLS helpers
// ---------------------------------------------------------------------------

fn build_tls_connector() -> Result<tokio_rustls::TlsConnector> {
    let mut root_store = rustls::RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

// ---------------------------------------------------------------------------
// Endpoint parsing (ws:// and wss://)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WsScheme {
    Ws,
    Wss,
}

fn parse_ws_endpoint(endpoint: &str) -> Result<(WsScheme, String, u16, String)> {
    let trimmed = endpoint.trim();

    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("wss://") {
        (WsScheme::Wss, rest)
    } else if let Some(rest) = trimmed.strip_prefix("ws://") {
        (WsScheme::Ws, rest)
    } else {
        return Err(QimenError::Transport(
            "only ws:// and wss:// endpoints are supported".to_string(),
        ));
    };

    let default_port = match scheme {
        WsScheme::Ws => 80,
        WsScheme::Wss => 443,
    };

    let (host_port, path) = match rest.split_once('/') {
        Some((host_port, path)) => (host_port, format!("/{path}")),
        None => (rest, "/".to_string()),
    };

    let (host, port) = match host_port.rsplit_once(':') {
        Some((host, port)) => {
            let port = port
                .parse::<u16>()
                .map_err(|err| QimenError::Transport(err.to_string()))?;
            (host.to_string(), port)
        }
        None => (host_port.to_string(), default_port),
    };

    Ok((scheme, host, port, path))
}

// ---------------------------------------------------------------------------
// Forward WebSocket client (ws:// and wss://)
// ---------------------------------------------------------------------------

/// A concrete split stream – either plain TCP or TLS-wrapped TCP.
enum SplitStream {
    Plain {
        reader: ReadHalf<TcpStream>,
        writer: WriteHalf<TcpStream>,
    },
    Tls {
        reader: ReadHalf<tokio_rustls::client::TlsStream<TcpStream>>,
        writer: WriteHalf<tokio_rustls::client::TlsStream<TcpStream>>,
    },
}

/// Type-erased writer so the client struct remains a single type.
type DynWriter = Box<dyn AsyncWrite + Unpin + Send>;

pub struct OneBot11ForwardWsClient {
    writer: Arc<Mutex<DynWriter>>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
    event_rx: mpsc::Receiver<String>,
    reader_task: JoinHandle<()>,
}

impl OneBot11ForwardWsClient {
    pub async fn connect(endpoint: &str, access_token: Option<&str>) -> Result<Self> {
        let (scheme, host, port, path) = parse_ws_endpoint(endpoint)?;
        let tcp = TcpStream::connect((host.as_str(), port)).await?;

        let split = match scheme {
            WsScheme::Ws => {
                let (reader, writer) = tokio::io::split(tcp);
                SplitStream::Plain { reader, writer }
            }
            WsScheme::Wss => {
                let connector = build_tls_connector()?;
                let domain = rustls::pki_types::ServerName::try_from(host.clone())
                    .map_err(|err| QimenError::Transport(format!("invalid TLS server name: {err}")))?;
                let tls_stream = connector.connect(domain, tcp).await.map_err(|err| {
                    QimenError::Transport(format!("TLS handshake failed: {err}"))
                })?;
                let (reader, writer) = tokio::io::split(tls_stream);
                SplitStream::Tls { reader, writer }
            }
        };

        // Perform WebSocket HTTP upgrade on the writer side, then read response
        // on the reader side. We need a temporary combined stream for the
        // handshake, so we do the handshake *before* splitting.
        //
        // Actually we already split – let's use the writer to send and reader
        // to receive the handshake.

        let key = "MDEyMzQ1Njc4OWFiY2RlZg==";
        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n"
        );

        if let Some(token) = access_token.filter(|token| !token.is_empty()) {
            request.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }

        request.push_str("\r\n");

        match split {
            SplitStream::Plain { mut reader, mut writer } => {
                writer.write_all(request.as_bytes()).await?;

                let mut response_buf = vec![0_u8; 4096];
                let read = reader.read(&mut response_buf).await?;
                let response_text = String::from_utf8_lossy(&response_buf[..read]);
                if !response_text.starts_with("HTTP/1.1 101") {
                    return Err(QimenError::Transport(format!(
                        "websocket handshake failed: {response_text}"
                    )));
                }

                tracing::info!(endpoint = %endpoint, "websocket handshake completed (ws)");

                let dyn_writer: DynWriter = Box::new(writer);
                let writer = Arc::new(Mutex::new(dyn_writer));
                let pending = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<String>>::new()));
                let (event_tx, event_rx) = mpsc::channel(128);

                let reader_task = {
                    let writer_clone = writer.clone();
                    let pending_clone = pending.clone();
                    tokio::spawn(async move {
                        generic_reader_task(reader, writer_clone, event_tx, pending_clone).await;
                    })
                };

                Ok(Self { writer, pending, event_rx, reader_task })
            }
            SplitStream::Tls { mut reader, mut writer } => {
                writer.write_all(request.as_bytes()).await?;

                let mut response_buf = vec![0_u8; 4096];
                let read = reader.read(&mut response_buf).await?;
                let response_text = String::from_utf8_lossy(&response_buf[..read]);
                if !response_text.starts_with("HTTP/1.1 101") {
                    return Err(QimenError::Transport(format!(
                        "websocket handshake failed: {response_text}"
                    )));
                }

                tracing::info!(endpoint = %endpoint, "websocket handshake completed (wss)");

                let dyn_writer: DynWriter = Box::new(writer);
                let writer = Arc::new(Mutex::new(dyn_writer));
                let pending = Arc::new(Mutex::new(HashMap::<String, oneshot::Sender<String>>::new()));
                let (event_tx, event_rx) = mpsc::channel(128);

                let reader_task = {
                    let writer_clone = writer.clone();
                    let pending_clone = pending.clone();
                    tokio::spawn(async move {
                        generic_reader_task(reader, writer_clone, event_tx, pending_clone).await;
                    })
                };

                Ok(Self { writer, pending, event_rx, reader_task })
            }
        }
    }

    pub async fn next_event(&mut self) -> Option<String> {
        self.event_rx.recv().await
    }

    pub async fn send_text(&self, text: &str) -> Result<()> {
        let mut writer = self.writer.lock().await;
        write_ws_text_frame_masked(&mut *writer, text).await
    }

    pub async fn send_text_await_echo(
        &self,
        text: &str,
        echo: &str,
        timeout: Duration,
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(echo.to_string(), tx);

        if let Err(err) = self.send_text(text).await {
            self.pending.lock().await.remove(echo);
            return Err(err);
        }

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(payload)) => Ok(payload),
            Ok(Err(_)) => Err(QimenError::Transport(format!(
                "pending echo channel closed for {echo}"
            ))),
            Err(_) => {
                self.pending.lock().await.remove(echo);
                Err(QimenError::Transport(format!(
                    "timed out waiting for echo {echo}"
                )))
            }
        }
    }
}

impl Drop for OneBot11ForwardWsClient {
    fn drop(&mut self) {
        self.reader_task.abort();
    }
}

// ---------------------------------------------------------------------------
// WS-Reverse server configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WsReverseConfig {
    pub host: String,
    pub port: u16,
    pub access_token: Option<String>,
}

// ---------------------------------------------------------------------------
// WS-Reverse server
// ---------------------------------------------------------------------------

pub struct WsReverseServer {
    event_rx: mpsc::Receiver<String>,
    listener_task: JoinHandle<()>,
}

impl WsReverseServer {
    pub async fn bind(config: WsReverseConfig) -> Result<Self> {
        let addr = format!("{}:{}", config.host, config.port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!(address = %addr, "ws-reverse server listening");

        let (event_tx, event_rx) = mpsc::channel(128);

        let listener_task = tokio::spawn(accept_loop(listener, event_tx, config.access_token));

        Ok(Self {
            event_rx,
            listener_task,
        })
    }

    pub async fn next_event(&mut self) -> Option<String> {
        self.event_rx.recv().await
    }
}

impl Drop for WsReverseServer {
    fn drop(&mut self) {
        self.listener_task.abort();
    }
}

async fn accept_loop(
    listener: TcpListener,
    event_tx: mpsc::Sender<String>,
    access_token: Option<String>,
) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::info!(peer = %peer, "ws-reverse: accepted connection");
                let event_tx = event_tx.clone();
                let access_token = access_token.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_reverse_connection(stream, event_tx, access_token.as_deref()).await
                    {
                        tracing::error!(peer = %peer, error = %err, "ws-reverse: connection error");
                    }
                });
            }
            Err(err) => {
                tracing::error!(error = %err, "ws-reverse: accept failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

async fn handle_reverse_connection(
    mut stream: TcpStream,
    event_tx: mpsc::Sender<String>,
    access_token: Option<&str>,
) -> Result<()> {
    // Read the HTTP upgrade request from the client
    let mut buf = vec![0_u8; 4096];
    let n = stream.read(&mut buf).await?;
    if n == 0 {
        return Err(QimenError::Transport("empty request".to_string()));
    }
    let request_text = String::from_utf8_lossy(&buf[..n]);

    // Validate it is a WebSocket upgrade request
    if !request_text.contains("Upgrade: websocket") && !request_text.contains("upgrade: websocket")
    {
        let response = "HTTP/1.1 400 Bad Request\r\n\r\n";
        stream.write_all(response.as_bytes()).await?;
        return Err(QimenError::Transport(
            "not a websocket upgrade request".to_string(),
        ));
    }

    // Check access token if configured
    if let Some(expected) = access_token.filter(|t| !t.is_empty()) {
        let authorized = request_text.lines().any(|line| {
            // Header name matching is case-insensitive per HTTP spec
            if let Some(colon_pos) = line.find(':') {
                let name = &line[..colon_pos];
                if !name.eq_ignore_ascii_case("authorization") {
                    return false;
                }
                let value = line[colon_pos + 1..].trim();
                // "Bearer" prefix is case-insensitive, token is exact match
                if value.len() > 7 && value[..7].eq_ignore_ascii_case("bearer ") {
                    return &value[7..] == expected;
                }
            }
            false
        });

        if !authorized {
            let response = "HTTP/1.1 401 Unauthorized\r\n\r\n";
            stream.write_all(response.as_bytes()).await?;
            return Err(QimenError::Transport("unauthorized".to_string()));
        }
    }

    // Extract Sec-WebSocket-Key
    let ws_key = request_text
        .lines()
        .find_map(|line| {
            let lower = line.to_lowercase();
            if lower.starts_with("sec-websocket-key:") {
                Some(line.splitn(2, ':').nth(1).unwrap_or("").trim().to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| QimenError::Transport("missing Sec-WebSocket-Key".to_string()))?;

    // Compute accept key
    let accept_key = compute_ws_accept_key(&ws_key);

    // Send 101 Switching Protocols
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept_key}\r\n\r\n"
    );
    stream.write_all(response.as_bytes()).await?;
    tracing::info!("ws-reverse: websocket handshake completed");

    let (mut reader, _writer) = tokio::io::split(stream);

    // Server-side reader loop – clients send masked frames; we just forward
    // text messages to the event channel.
    loop {
        match read_ws_frame_server(&mut reader).await {
            Ok(Some(payload)) => {
                if payload.is_empty() {
                    continue;
                }
                if event_tx.send(payload).await.is_err() {
                    tracing::warn!("ws-reverse: event receiver dropped, stopping");
                    break;
                }
            }
            Ok(None) => {
                tracing::info!("ws-reverse: client closed connection");
                break;
            }
            Err(err) => {
                tracing::error!(error = %err, "ws-reverse: read error");
                break;
            }
        }
    }

    Ok(())
}

fn compute_ws_accept_key(key: &str) -> String {
    use std::io::Write;
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-5AB5DC525B63";

    // SHA-1 implementation (minimal, for the WebSocket accept key only)
    let mut input = Vec::new();
    write!(input, "{key}{WS_GUID}").unwrap();
    let hash = sha1_digest(&input);
    base64_encode(&hash)
}

/// Minimal SHA-1 used solely for the WebSocket accept-key derivation.
fn sha1_digest(data: &[u8]) -> [u8; 20] {
    let mut h0: u32 = 0x67452301;
    let mut h1: u32 = 0xEFCDAB89;
    let mut h2: u32 = 0x98BADCFE;
    let mut h3: u32 = 0x10325476;
    let mut h4: u32 = 0xC3D2E1F0;

    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0_u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999_u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1_u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC_u32),
                _ => (b ^ c ^ d, 0xCA62C1D6_u32),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut digest = [0_u8; 20];
    digest[0..4].copy_from_slice(&h0.to_be_bytes());
    digest[4..8].copy_from_slice(&h1.to_be_bytes());
    digest[8..12].copy_from_slice(&h2.to_be_bytes());
    digest[12..16].copy_from_slice(&h3.to_be_bytes());
    digest[16..20].copy_from_slice(&h4.to_be_bytes());
    digest
}

fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Generic reader task (forward mode)
// ---------------------------------------------------------------------------

async fn generic_reader_task<R>(
    mut reader: R,
    writer: Arc<Mutex<DynWriter>>,
    event_tx: mpsc::Sender<String>,
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<String>>>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    loop {
        match read_ws_frame_client(&mut reader, &writer).await {
            Ok(Some(payload)) => {
                if payload.is_empty() {
                    continue;
                }

                if let Some(echo) = extract_action_response_echo(&payload) {
                    let maybe_sender = pending.lock().await.remove(&echo);
                    if let Some(sender) = maybe_sender {
                        let _ = sender.send(payload);
                        continue;
                    }
                }

                if event_tx.send(payload).await.is_err() {
                    tracing::warn!("event receiver dropped, stopping ws reader task");
                    break;
                }
            }
            Ok(None) => {
                pending.lock().await.clear();
                tracing::warn!("remote closed websocket stream");
                break;
            }
            Err(err) => {
                pending.lock().await.clear();
                tracing::error!(error = %err, "ws reader task failed");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// WebSocket frame reading
// ---------------------------------------------------------------------------

/// Read a WebSocket frame from a client perspective (server frames are NOT
/// masked, and we respond to pings by writing a masked pong).
async fn read_ws_frame_client<R>(
    reader: &mut R,
    writer: &Arc<Mutex<DynWriter>>,
) -> Result<Option<String>>
where
    R: AsyncRead + Unpin,
{
    let (opcode, payload) = match read_raw_ws_frame(reader).await? {
        Some(frame) => frame,
        None => return Ok(None),
    };

    match opcode {
        0x1 => String::from_utf8(payload)
            .map(Some)
            .map_err(|err| QimenError::Transport(err.to_string())),
        0x8 => Ok(None),
        0x9 => {
            // Ping – reply with masked pong
            let mut w = writer.lock().await;
            write_ws_frame_masked(&mut *w, 0xA, &payload).await?;
            Ok(Some(String::new()))
        }
        0xA => Ok(Some(String::new())),
        _ => Ok(Some(String::new())),
    }
}

/// Read a WebSocket frame from a server perspective (client frames are masked,
/// no pong writer needed – pings are not sent by the server side).
async fn read_ws_frame_server<R>(reader: &mut R) -> Result<Option<String>>
where
    R: AsyncRead + Unpin,
{
    let (opcode, payload) = match read_raw_ws_frame(reader).await? {
        Some(frame) => frame,
        None => return Ok(None),
    };

    match opcode {
        0x1 => String::from_utf8(payload)
            .map(Some)
            .map_err(|err| QimenError::Transport(err.to_string())),
        0x8 => Ok(None),
        0x9 | 0xA => Ok(Some(String::new())),
        _ => Ok(Some(String::new())),
    }
}

/// Low-level: read one WebSocket frame, handling mask transparently.
async fn read_raw_ws_frame<R>(stream: &mut R) -> Result<Option<(u8, Vec<u8>)>>
where
    R: AsyncRead + Unpin,
{
    let mut header = [0_u8; 2];
    match stream.read_exact(&mut header).await {
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
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

    Ok(Some((opcode, payload)))
}

// ---------------------------------------------------------------------------
// WebSocket frame writing
// ---------------------------------------------------------------------------

/// Write a text frame with client masking (for forward mode).
async fn write_ws_text_frame_masked<W>(stream: &mut W, text: &str) -> Result<()>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    write_ws_frame_masked(stream, 0x1, text.as_bytes()).await
}

/// Write a WebSocket frame with client masking.
async fn write_ws_frame_masked<W>(stream: &mut W, opcode: u8, payload: &[u8]) -> Result<()>
where
    W: AsyncWrite + Unpin + ?Sized,
{
    let mut frame = Vec::with_capacity(payload.len() + 16);
    frame.push(0x80 | opcode);

    let payload_len = payload.len();
    let mask_bit = 0x80;

    if payload_len <= 125 {
        frame.push(mask_bit | payload_len as u8);
    } else if payload_len <= u16::MAX as usize {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(payload_len as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(payload_len as u64).to_be_bytes());
    }

    let mask = [0x12_u8, 0x34, 0x56, 0x78];
    frame.extend_from_slice(&mask);

    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[index % 4]);
    }

    stream.write_all(&frame).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_action_response_echo(payload: &str) -> Option<String> {
    let value: Value = serde_json::from_str(payload).ok()?;
    if value.get("post_type").is_some() {
        return None;
    }
    let echo = value.get("echo")?;
    match echo {
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}
