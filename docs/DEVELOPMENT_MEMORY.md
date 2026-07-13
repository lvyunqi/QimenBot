# Development Memory

## Current State

- Dynamic plugin API 0.4 exposes bot-scoped real-time proactive sends while API 0.1-0.3 keep the callback-flush path.
- Host binding is synchronized so unbind waits for in-flight plugin callbacks.
- Enabled bots own bounded proactive queues and online protocol executors for OneBot 11 and QQ official sends.
- OneBot 11 and QQ official targets cover private, group, channel, and channel-private routes.
- Runtime shutdown rejects new sends, finishes the active send, drops queued work, then shuts down, unbinds, and unloads plugins.
- `abi-stable-host-api 0.1.10` and `qimen-dynamic-plugin-derive 0.1.10` are published and visible on crates.io.
- The registry-only external plugin verification and Linux dynamic-library lifecycle smoke both pass.
- The status plugin API 0.4 migration is pushed to its main branch with explicit `[[push.targets]]` and collector-driven real-time sends.
- Framework release commits are on upstream/main; only the final `v0.1.10` tag remains.

## Recent Completion

- Published both public dynamic-plugin crates at 0.1.10 and confirmed they are not yanked.
- Verified a crates.io-only external `cdylib` can load, bind, send immediately after init, shut down, unbind, and release its Windows loader handle.
- Migrated the status plugin to API 0.4 and pushed explicit multi-bot proactive targets, documentation, and crates.io lockfile updates to its main branch.
- Added an Ubuntu FFI smoke that loads the real plugin `.so`, receives a proactive send without events, shuts down, unbinds, rejects post-unbind callbacks, and closes the loader handle.
- Completed the status plugin CI across Check, Linux, Windows, and macOS in GitHub Actions run 29217151875.

## Next Step

- Create and push the framework `v0.1.10` tag after confirming all release worktrees are clean.

## Verification Baseline

- cargo metadata --locked (C:\projects\newapi-status-bot)
- cargo fmt --all -- --check (C:\projects\newapi-status-bot)
- cargo test --locked (C:\projects\newapi-status-bot; 48 passed, 1 ignored)
- cargo clippy --locked --all-targets -- -D warnings (C:\projects\newapi-status-bot)
- cargo build --release --locked (C:\projects\newapi-status-bot)
- cargo check --manifest-path tools/ffi-smoke/Cargo.toml --locked (C:\projects\newapi-status-bot)
- cargo clippy --manifest-path tools/ffi-smoke/Cargo.toml --locked -- -D warnings (C:\projects\newapi-status-bot)
- GitHub Actions run 29217151875: Check, Linux FFI smoke, Windows build, and macOS build passed.
- registry-only API 0.4 load/send/unbind/unload verification passed with crates.io 0.1.10 dependencies.
- cargo check --offline (plugins/qimen-dynamic-plugin-example)
- cargo fmt --all -- --check
- cargo clippy --workspace --offline -- -D warnings
- cargo test --workspace --offline
- cargo check --workspace --offline
- git diff --check
