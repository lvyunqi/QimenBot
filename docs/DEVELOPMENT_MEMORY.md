# Development Memory

## Current State

- Dynamic plugin API 0.4 exposes bot-scoped real-time proactive sends while API 0.1-0.3 keep the callback-flush path.
- Host binding is synchronized so unbind waits for in-flight plugin callbacks.
- Enabled bots own bounded proactive queues and online protocol executors for OneBot 11 and QQ official sends.
- OneBot 11 and QQ official targets cover private, group, channel, and channel-private routes.
- Runtime shutdown rejects new sends, finishes the active send, drops queued work, then shuts down, unbinds, and unloads plugins.

## Recent Completion

- Added per-bot bounded proactive queues with offline TTL, queue-full reporting, strict bot isolation, and shutdown wakeups.
- Added cloneable OneBot 11 WebSocket action senders for forward and reverse sessions.
- Added protocol-neutral target mapping for all planned OneBot 11 and QQ official message targets.
- Registered online executors independently from inbound events, including QQ official OpenAPI clients.
- Kept dynamic libraries resident when plugin shutdown or Host API unbind cannot complete safely.

## Next Step

- Update API 0.4 dynamic plugin examples, configuration samples, and documentation.

## Verification Baseline

- cargo test -p qimen-config --offline
- cargo test -p qimen-transport-ws --offline
- cargo test -p qimen-runtime --lib --offline
- cargo clippy -p qimen-runtime -p qimen-config -p qimen-transport-ws --offline -- -D warnings
- cargo check --workspace --offline
- cargo fmt --all
- git diff --check
