//! Live integration test against a running OneBot11 implementation.
//! Requires: ws://127.0.0.1:3001
//! Run with: cargo test -p qimen-runtime --test live_ws_integration -- --ignored

use qimen_adapter_onebot11::OneBot11Adapter;
use qimen_protocol_core::{EventKind, IncomingPacket, ProtocolAdapter, ProtocolId, TransportMode};
use qimen_transport_ws::OneBot11ForwardWsClient;

/// Test the full event receive -> decode -> dispatch pipeline.
/// Connects to a live OneBot11 WebSocket server, waits for an event (typically
/// a heartbeat meta event), decodes it through the adapter, and verifies the
/// resulting NormalizedEvent is valid.
#[tokio::test]
#[ignore]
async fn receive_and_decode_live_events() {
    let mut client = OneBot11ForwardWsClient::connect("ws://127.0.0.1:3001", None)
        .await
        .expect("failed to connect to ws://127.0.0.1:3001");

    // Wait for first event (likely heartbeat)
    let timeout = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        client.next_event(),
    )
    .await;

    match timeout {
        Ok(Some(raw_json)) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&raw_json).expect("received non-JSON data");
            println!(
                "received event: {}",
                serde_json::to_string_pretty(&parsed).unwrap()
            );

            // Build an IncomingPacket and decode through the adapter
            let packet = IncomingPacket {
                protocol: ProtocolId::OneBot11,
                transport_mode: TransportMode::WsForward,
                bot_instance: "test".to_string(),
                payload: parsed.clone(),
                raw_bytes: None,
            };

            let adapter = OneBot11Adapter;
            let event = adapter
                .decode_event(packet)
                .await
                .expect("failed to decode event");

            println!("decoded event kind: {:?}", event.kind);
            assert_eq!(event.protocol, ProtocolId::OneBot11);

            // Heartbeat events from OneBot11 have post_type "meta_event"
            if parsed
                .get("post_type")
                .and_then(serde_json::Value::as_str)
                == Some("meta_event")
            {
                assert_eq!(event.kind, EventKind::Meta);
            }
        }
        Ok(None) => {
            panic!("connection closed without receiving any events");
        }
        Err(_) => {
            panic!("timed out waiting for event (10s)");
        }
    }
}

/// Test that multiple events can be received in sequence.
#[tokio::test]
#[ignore]
async fn receive_multiple_events() {
    let mut client = OneBot11ForwardWsClient::connect("ws://127.0.0.1:3001", None)
        .await
        .expect("failed to connect to ws://127.0.0.1:3001");

    let adapter = OneBot11Adapter;
    let mut event_count = 0;

    // Try to collect up to 3 events within 15 seconds
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);

    while event_count < 3 {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, client.next_event()).await {
            Ok(Some(raw_json)) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw_json) {
                    let packet = IncomingPacket {
                        protocol: ProtocolId::OneBot11,
                        transport_mode: TransportMode::WsForward,
                        bot_instance: "test".to_string(),
                        payload: parsed,
                        raw_bytes: None,
                    };

                    if let Ok(event) = adapter.decode_event(packet).await {
                        event_count += 1;
                        println!("event #{}: kind={:?}", event_count, event.kind);
                    }
                }
            }
            Ok(None) => break,
            Err(_) => break,
        }
    }

    assert!(
        event_count >= 1,
        "expected at least 1 event, got {event_count}"
    );
    println!("successfully received and decoded {event_count} events");
}
