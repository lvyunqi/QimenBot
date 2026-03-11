//! Integration tests for WebSocket transport.
//! These tests require a running OneBot11 implementation at ws://127.0.0.1:3001
//! Run with: cargo test -p qimen-transport-ws --test ws_integration -- --ignored

use qimen_transport_ws::OneBot11ForwardWsClient;

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
    let result =
        OneBot11ForwardWsClient::connect("ws://127.0.0.1:3001", Some("test-token")).await;
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
