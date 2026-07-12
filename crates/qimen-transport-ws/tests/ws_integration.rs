//! Integration tests for WebSocket transport.
//! These tests require a running OneBot11 implementation at ws://127.0.0.1:3001
//! Run with: cargo test -p qimen-transport-ws --test ws_integration -- --ignored

use qimen_transport_ws::{OneBot11ForwardWsClient, WsReverseConfig, WsReverseServer};
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Test connecting to a local OneBot11 WebSocket server.
/// This test is ignored by default since it requires a running server.
#[tokio::test]
#[ignore]
async fn connect_to_local_ws_server() {
    let result = OneBot11ForwardWsClient::connect("ws://127.0.0.1:3001", None).await;
    assert!(result.is_ok(), "failed to connect: {:?}", result.err());
    let client = result.unwrap();
    // Give the client a moment to receive any initial events (like heartbeat)
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    // If we get here without panic, the connection works
    drop(client);
}

/// Test connecting with an access token.
#[tokio::test]
#[ignore]
async fn connect_with_access_token() {
    // This should fail if the server doesn't expect a token, or succeed if it does
    let result = OneBot11ForwardWsClient::connect("ws://127.0.0.1:3001", Some("test-token")).await;
    // We just verify it doesn't panic - the actual result depends on server config
    let _ = result;
}

/// Test that connecting to a non-existent server returns an error.
#[tokio::test]
async fn connect_to_nonexistent_server_fails() {
    let result = OneBot11ForwardWsClient::connect("ws://127.0.0.1:19999", None).await;
    assert!(result.is_err());
}

/// Test that invalid URL is handled gracefully.
#[tokio::test]
async fn connect_with_invalid_url_fails() {
    let result = OneBot11ForwardWsClient::connect("not-a-url", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn reverse_ws_rejects_wrong_path_and_token() {
    let mut server = WsReverseServer::bind(WsReverseConfig {
        bind: "127.0.0.1:0".to_string(),
        path: "/onebot/reverse".to_string(),
        access_token: Some("expected-token".to_string()),
    })
    .await
    .unwrap();
    let address = server.local_addr();

    let (_, response) = reverse_handshake(address, "/wrong", Some("expected-token")).await;
    assert!(response.starts_with("HTTP/1.1 404"));

    let (_, response) = reverse_handshake(address, "/onebot/reverse", Some("wrong-token")).await;
    assert!(response.starts_with("HTTP/1.1 401"));

    assert!(
        tokio::time::timeout(Duration::from_millis(100), server.next_connection())
            .await
            .is_err(),
        "rejected handshakes must not create runtime connections"
    );
}

#[tokio::test]
async fn reverse_ws_routes_events_and_action_responses_bidirectionally() {
    let mut server = WsReverseServer::bind(WsReverseConfig {
        bind: "127.0.0.1:0".to_string(),
        path: "/onebot/reverse".to_string(),
        access_token: Some("test-token".to_string()),
    })
    .await
    .unwrap();
    let address = server.local_addr();

    let (mut peer, response) =
        reverse_handshake(address, "/onebot/reverse", Some("test-token")).await;
    assert!(response.starts_with("HTTP/1.1 101"));
    let mut connection = tokio::time::timeout(Duration::from_secs(1), server.next_connection())
        .await
        .unwrap()
        .unwrap();

    let event = json!({
        "post_type": "meta_event",
        "meta_event_type": "heartbeat",
        "interval": 5000
    });
    write_masked_text(&mut peer, &event.to_string()).await;
    let received = tokio::time::timeout(Duration::from_secs(1), connection.next_event())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&received).unwrap(), event);

    let action = json!({
        "action": "send_group_msg",
        "params": { "group_id": 10001, "message": "pong" },
        "echo": "reverse-test-echo"
    })
    .to_string();
    let action_roundtrip =
        connection.send_text_await_echo(&action, "reverse-test-echo", Duration::from_secs(1));
    let peer_roundtrip = async {
        let received_action = read_unmasked_text(&mut peer).await;
        let payload: Value = serde_json::from_str(&received_action).unwrap();
        assert_eq!(payload["action"], "send_group_msg");
        assert_eq!(payload["echo"], "reverse-test-echo");

        let response = json!({
            "status": "ok",
            "retcode": 0,
            "data": {},
            "echo": "reverse-test-echo"
        });
        write_masked_text(&mut peer, &response.to_string()).await;
        response
    };

    let (response, expected) = tokio::join!(action_roundtrip, peer_roundtrip);
    let response: Value = serde_json::from_str(&response.unwrap()).unwrap();
    assert_eq!(response, expected);
}

async fn reverse_handshake(
    address: SocketAddr,
    path: &str,
    token: Option<&str>,
) -> (TcpStream, String) {
    let mut stream = TcpStream::connect(address).await.unwrap();
    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: MDEyMzQ1Njc4OWFiY2RlZg==\r\n"
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();

    let mut response = Vec::new();
    loop {
        let mut chunk = [0_u8; 256];
        let read = stream.read(&mut chunk).await.unwrap();
        assert!(read > 0, "server closed before sending an HTTP response");
        response.extend_from_slice(&chunk[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }

    (stream, String::from_utf8(response).unwrap())
}

async fn write_masked_text(stream: &mut TcpStream, text: &str) {
    let payload = text.as_bytes();
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.push(0x81);
    if payload.len() <= 125 {
        frame.push(0x80 | payload.len() as u8);
    } else {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    }
    let mask = [0x12, 0x34, 0x56, 0x78];
    frame.extend_from_slice(&mask);
    frame.extend(
        payload
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ mask[index % 4]),
    );
    stream.write_all(&frame).await.unwrap();
}

async fn read_unmasked_text(stream: &mut TcpStream) -> String {
    let mut header = [0_u8; 2];
    stream.read_exact(&mut header).await.unwrap();
    assert_eq!(header[0] & 0x0f, 0x1);
    assert_eq!(header[1] & 0x80, 0, "server frames must not be masked");

    let mut payload_len = (header[1] & 0x7f) as usize;
    if payload_len == 126 {
        let mut extended = [0_u8; 2];
        stream.read_exact(&mut extended).await.unwrap();
        payload_len = u16::from_be_bytes(extended) as usize;
    } else if payload_len == 127 {
        let mut extended = [0_u8; 8];
        stream.read_exact(&mut extended).await.unwrap();
        payload_len = u64::from_be_bytes(extended) as usize;
    }

    let mut payload = vec![0_u8; payload_len];
    stream.read_exact(&mut payload).await.unwrap();
    String::from_utf8(payload).unwrap()
}
