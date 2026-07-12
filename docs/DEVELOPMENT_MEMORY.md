# Development Memory

## Current State

- OneBot 11 reverse WebSocket has a bidirectional runtime path with reconnect waiting.
- Enabled long-running bot transports are polled concurrently.
- Dynamic plugins can be developed outside the main repository using crates.io dependencies.
- Dynamic command dispatch logs identify actual plugin matches without labeling unknown commands as built-ins.
- Loaded dynamic plugins remain resident until explicit reload so background plugin threads cannot outlive unloaded code.
- Dynamic callback signatures match the proc-macro exports, and queued send actions are copied into host-owned ABI strings.
- Dynamic plugin API 0.4 types and bot-scoped proactive send helpers are implemented with synchronized host binding.
- The dynamic plugin macro emits Host API bind/unbind symbols only when a plugin explicitly declares API 0.4.

## Recent Completion

- Added an optional dynamic plugin API declaration with a compatibility default of API 0.3.
- Generated API 0.4 Host API bind/unbind exports and descriptor metadata.
- Added API 0.4 proactive request/status types and a versioned Host API table.
- Added synchronized bind/unbind handling that waits for in-flight plugin sends.
- Added explicit bot, channel, channel-private, and guild-channel plugin send builders.

## Next Step

- Implement per-bot proactive send queues and online protocol executors in the runtime.

## Verification Baseline

- `cargo test -p qimen-transport-ws --test ws_integration`
- `cargo test -p qimen-config`
- `cargo test -p qimen-runtime --lib`
- `cargo check --workspace --offline`
- `CARGO_INCREMENTAL=0 RUSTFLAGS="-C debuginfo=0" cargo test --workspace --offline -j 2`
- `cargo clippy --workspace --offline -- -D warnings`
- Independent dynamic-library runtime fixture: command execution, queued-send flush, explicit unload, then safe send read/drop.
- Standalone real dynamic plugin: 9 multi-part sends remained readable and safely droppable after explicit DLL unload.
- `cargo clippy -p qimen-transport-ws --all-targets -- -D warnings`
- `cargo clippy -p qimen-runtime -p qimen-config -p qimen-official-host --lib -- -D warnings`
- Standalone status plugin: `cargo test --offline`, `cargo clippy --offline --all-targets -- -D warnings`, and `cargo build --release --offline`.
- Standalone external `cdylib` release build using registry versions `abi-stable-host-api@0.1.1` and `qimen-dynamic-plugin-derive@0.1.1`.
- `cargo test -p abi-stable-host-api --offline`
- `cargo clippy -p abi-stable-host-api --all-targets --offline -- -D warnings`
- `cargo test -p qimen-dynamic-plugin-derive -p abi-stable-host-api --offline`
- `cargo clippy -p qimen-dynamic-plugin-derive -p abi-stable-host-api --all-targets --offline -- -D warnings`
- `cd docs && npm ci && npm run docs:build`
- Daemon smoke test: reverse-only config stayed alive and logged the bound address/path.
