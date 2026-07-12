# Development Memory

## Current State

- OneBot 11 reverse WebSocket has a bidirectional runtime path with reconnect waiting.
- Enabled long-running bot transports are polled concurrently.
- Dynamic plugins can be developed outside the main repository using crates.io dependencies.
- Dynamic command dispatch logs identify actual plugin matches without labeling unknown commands as built-ins.

## Recent Completion

- Prepared the command dispatch observability fix as release `v0.1.7`.
- Added private-chat query support and correct multi-chunk routing in the standalone status plugin.
- Corrected the RFC 6455 accept GUID and prepared hotfix release `v0.1.6`.
- Prepared the verified reverse WebSocket and standalone plugin documentation batch as release `v0.1.5`.
- Documented and release-built a standalone dynamic plugin against both published `0.1.1` crates.

## Next Step

- Build and deploy the updated status plugin on Linux, then verify `/模型状态` in private chat.

## Verification Baseline

- `cargo test -p qimen-transport-ws --test ws_integration`
- `cargo test -p qimen-config`
- `cargo test -p qimen-runtime --lib`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo clippy -p qimen-transport-ws --all-targets -- -D warnings`
- `cargo clippy -p qimen-runtime -p qimen-config -p qimen-official-host --lib -- -D warnings`
- Standalone status plugin: `cargo test --offline`, `cargo clippy --offline --all-targets -- -D warnings`, and `cargo build --release --offline`.
- Standalone external `cdylib` release build using registry versions `abi-stable-host-api@0.1.1` and `qimen-dynamic-plugin-derive@0.1.1`.
- `cd docs && npm ci && npm run docs:build`
- Daemon smoke test: reverse-only config stayed alive and logged the bound address/path.
