# Development Memory

## Current State

- Dynamic plugin API 0.4 exposes bot-scoped real-time proactive sends while API 0.1-0.3 keep the callback-flush path.
- Host binding is synchronized so unbind waits for in-flight plugin callbacks.
- Enabled bots own bounded proactive queues and online protocol executors for OneBot 11 and QQ official sends.
- OneBot 11 and QQ official targets cover private, group, channel, and channel-private routes.
- Runtime shutdown rejects new sends, finishes the active send, drops queued work, then shuts down, unbinds, and unloads plugins.
- Framework source for 0.1.10 is pushed to upstream/main, but crates.io publish is blocked by a missing cargo registry token.
- The status plugin has a local API 0.4 proactive-send migration verified with command-level local crate patches; it is not pushed because the 0.1.10 crates are not published yet.

## Recent Completion

- Migrated C:\projects\newapi-status-bot locally to API 0.4 Host API sends with explicit [[push.targets]] and collector-driven push.
- Updated the status plugin README and config example to remove Heartbeat-driven push guidance.
- Verified the status plugin with fmt, unit tests, Clippy, and release build using local 0.1.10 crate patches.
- Bumped all workspace crates and both publishable dynamic-plugin crates to 0.1.10.
- Regenerated the root and independent dynamic-example lockfiles at 0.1.10.

## Next Step

- Publish abi-stable-host-api 0.1.10 with a cargo registry token, then publish qimen-dynamic-plugin-derive 0.1.10.

## Verification Baseline

- cargo fmt --all -- --check (C:\projects\newapi-status-bot)
- cargo test --config "patch.crates-io.abi-stable-host-api.path='C:/projects/QimenBot/crates/abi-stable-host-api'" --config "patch.crates-io.qimen-dynamic-plugin-derive.path='C:/projects/QimenBot/crates/qimen-dynamic-plugin-derive'" (C:\projects\newapi-status-bot)
- cargo clippy --config "patch.crates-io.abi-stable-host-api.path='C:/projects/QimenBot/crates/abi-stable-host-api'" --config "patch.crates-io.qimen-dynamic-plugin-derive.path='C:/projects/QimenBot/crates/qimen-dynamic-plugin-derive'" --all-targets -- -D warnings (C:\projects\newapi-status-bot)
- cargo build --release --config "patch.crates-io.abi-stable-host-api.path='C:/projects/QimenBot/crates/abi-stable-host-api'" --config "patch.crates-io.qimen-dynamic-plugin-derive.path='C:/projects/QimenBot/crates/qimen-dynamic-plugin-derive'" (C:\projects\newapi-status-bot)
- cargo check --offline (plugins/qimen-dynamic-plugin-example)
- cargo fmt --all -- --check
- cargo clippy --workspace --offline -- -D warnings
- cargo test --workspace --offline
- cargo check --workspace --offline
- git diff --check
