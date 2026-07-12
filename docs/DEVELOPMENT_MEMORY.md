# Development Memory

## Current State

- Dynamic plugin API 0.4 exposes bot-scoped real-time proactive sends while API 0.1-0.3 keep the callback-flush path.
- Host binding is synchronized so unbind waits for in-flight plugin callbacks.
- Enabled bots own bounded proactive queues and online protocol executors for OneBot 11 and QQ official sends.
- OneBot 11 and QQ official targets cover private, group, channel, and channel-private routes.
- Runtime shutdown rejects new sends, finishes the active send, drops queued work, then shuts down, unbinds, and unloads plugins.

## Recent Completion

- Added API 0.4 background-thread examples for private, group, channel, and channel-private sends.
- Updated the dynamic plugin template to bind Host API v1 and join its worker during shutdown.
- Documented proactive queue configuration, enqueue statuses, protocol target mapping, and FFI lifecycle rules.
- Updated the Chinese, English, and Japanese standalone-plugin dependency examples for crates.io 0.1.10.
- Added the v0.1.10 changelog entry and VitePress navigation.

## Next Step

- Bump the workspace and publishable crates to version 0.1.10.

## Verification Baseline

- cargo fmt --check (plugins/qimen-dynamic-plugin-example)
- cargo test --offline (plugins/qimen-dynamic-plugin-example)
- cargo clippy --offline --all-targets -- -D warnings (plugins/qimen-dynamic-plugin-example)
- cargo build --release --offline (plugins/qimen-dynamic-plugin-example)
- cargo test -p qimen-config --offline
- cargo check --workspace --offline
- npm run docs:build (docs)
- git diff --check
