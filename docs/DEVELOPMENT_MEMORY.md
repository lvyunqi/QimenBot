# Development Memory

## Current State

- OneBot 11 reverse WebSocket has a bidirectional runtime path with reconnect waiting.
- Enabled long-running bot transports are polled concurrently.
- Dynamic plugins can be developed outside the main repository using crates.io dependencies.
- Dynamic command dispatch logs identify actual plugin matches without labeling unknown commands as built-ins.
- Loaded dynamic plugins remain resident until explicit reload so background plugin threads cannot outlive unloaded code.
- Dynamic callback signatures match the proc-macro exports, and queued send actions are copied into host-owned ABI strings.

## Recent Completion

- Prepared the dynamic FFI ownership hardening as release `v0.1.9`.
- Matched command and route callbacks to their reference-parameter exports.
- Copied queued send actions into host ownership before asynchronous processing or unload.
- Prepared the dynamic-library residency fix as release `v0.1.8`.
- Removed unsafe idle eviction for loaded dynamic libraries and added explicit-residency lifecycle coverage.

## Next Step

- Deploy `v0.1.9` on Linux and verify a multi-part dynamic command after more than five minutes of idle time.

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
- `cd docs && npm ci && npm run docs:build`
- Daemon smoke test: reverse-only config stayed alive and logged the bound address/path.
