# Development Memory

## Current State

- OneBot 11 reverse WebSocket has a bidirectional runtime path with reconnect waiting.
- Enabled long-running bot transports are polled concurrently.

## Recent Completion

- Added reverse WebSocket bind/path validation, authentication, event routing, and Action echo correlation.
- Replaced the hard-coded host compatibility version with the compiled package version.
- Added TCP-level reverse WebSocket integration tests and a runtime liveness regression test.

## Next Step

- Validate the reverse WebSocket build against the deployed OneBot implementation.

## Verification Baseline

- `cargo test -p qimen-transport-ws --test ws_integration`
- `cargo test -p qimen-config`
- `cargo test -p qimen-runtime --lib`
- `cargo test --workspace`
- `cargo clippy -p qimen-transport-ws --all-targets -- -D warnings`
- `cargo clippy -p qimen-runtime -p qimen-config -p qimen-official-host --lib -- -D warnings`
- Daemon smoke test: reverse-only config stayed alive and logged the bound address/path.
