# Development Memory

## Current State

- Dynamic plugin API 0.4 exposes bot-scoped real-time proactive sends while API 0.1-0.3 keep the callback-flush path.
- Host binding is synchronized so unbind waits for in-flight plugin callbacks.
- Enabled bots own bounded proactive queues and online protocol executors for OneBot 11 and QQ official sends.
- OneBot 11 and QQ official targets cover private, group, channel, and channel-private routes.
- Runtime shutdown rejects new sends, finishes the active send, drops queued work, then shuts down, unbinds, and unloads plugins.

## Recent Completion

- Bumped all workspace crates and both publishable dynamic-plugin crates to 0.1.10.
- Regenerated the root and independent dynamic-example lockfiles at 0.1.10.
- Passed the GitHub CI-equivalent workspace Clippy command with warnings denied.
- Passed all workspace unit, integration, and documentation tests.
- Confirmed the independent API 0.4 example still resolves the local 0.1.10 crates.

## Next Step

- Package and publish abi-stable-host-api 0.1.10, then qimen-dynamic-plugin-derive 0.1.10.

## Verification Baseline

- cargo check --offline (plugins/qimen-dynamic-plugin-example)
- cargo fmt --all -- --check
- cargo clippy --workspace --offline -- -D warnings
- cargo test --workspace --offline
- cargo check --workspace --offline
- git diff --check
