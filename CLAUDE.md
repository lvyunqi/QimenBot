# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

QimenBot is a multi-protocol bot framework in Rust (2024 Edition, MSRV 1.89). It supports OneBot 11 protocol with WebSocket (forward/reverse) and HTTP transports, multi-bot instances, and two plugin systems (static + dynamic).

## Build Commands

```bash
# Workspace (framework + static plugins + daemon)
cargo check                    # Type-check entire workspace
cargo build --release          # Build daemon (default-members = qimenbotd)
cargo run                      # Run daemon (reads config/base.toml)
cargo test                     # Run all workspace tests
cargo test -p qimen-runtime    # Test a single crate

# Dynamic plugins (independent workspace, NOT part of main workspace)
cd plugins/qimen-dynamic-plugin-<name>
cargo check                    # Check plugin
cargo build --release          # Build .so/.dll/.dylib
cargo test                     # Run plugin tests
# Output goes to plugins/bin/ for runtime loading
```

Dynamic plugins have their own `Cargo.toml` with `crate-type = ["cdylib"]` and are **not** members of the root workspace. Always `cd` into the plugin directory before building.

## Architecture

### Layered Crate Design (33 workspace members)

```
apps/qimenbotd              — Daemon entry point
crates/qimen-official-host  — Reference host (config, module orchestration)
crates/qimen-runtime        — Core event dispatch, plugin loading, rate limiting
crates/qimen-plugin-api     — Static plugin traits (CommandPlugin, SystemPlugin)
crates/qimen-plugin-derive  — Proc macros (#[module], #[command], #[notice])
crates/abi-stable-host-api  — Dynamic plugin FFI types (ABI-stable via abi_stable)
crates/qimen-dynamic-plugin-derive — Dynamic plugin proc macros (#[dynamic_plugin])
crates/qimen-adapter-onebot11     — OneBot 11 protocol adapter
crates/qimen-transport-ws/http    — WebSocket + HTTP transports
crates/qimen-message              — Message model (Segment enum, MessageBuilder)
crates/qimen-command-registry     — Command routing table with aliases/roles/scopes
crates/qimen-mod-{command,admin,scheduler,bridge} — Built-in modules
```

### Event Processing Pipeline

Events flow through: Protocol Decode → Message Dedup → Group Filter → Rate Limit → Pre-handle Interceptors → Permission Resolve → Command Dispatch → System Event Dispatch → Post-handle Interceptors.

### Two Plugin Systems

**Static plugins** — compiled with framework, full async support, `#[module]`/`#[command]` macros, access to `OneBotActionClient`. Located in `plugins/qimen-plugin-*`, registered via `inventory::submit!`.

**Dynamic plugins** — independent `cdylib`, loaded at runtime via `dlopen`, hot-reloadable. Use `#[dynamic_plugin]` macro, sync-only FFI callbacks, queue-based sending (`BotApi`/`SendBuilder`). Located in `plugins/qimen-dynamic-plugin-*` (own workspace). API version 0.3 (backward-compatible with 0.1/0.2).

### Key Runtime Files

- `crates/qimen-runtime/src/lib.rs` — Core event dispatch, interceptor chain
- `crates/qimen-runtime/src/dynamic_runtime.rs` — Dynamic plugin loading/execution
- `crates/qimen-command-registry/src/lib.rs` — Command routing with priority
- `crates/abi-stable-host-api/src/lib.rs` — FFI types for dynamic plugins

## Rust Conventions

- Edition 2024: use `#[unsafe(no_mangle)]` syntax (not bare `#[no_mangle]`)
- Workspace dependencies defined in root `Cargo.toml` under `[workspace.dependencies]`
- Dynamic plugin FFI functions use `pub unsafe extern "C" fn` with ABI-stable types
- Config: `config/base.toml` with env var substitution (`${VAR}`), per-bot overrides

## Configuration

- `config/base.toml` — Primary config (runtime, observability, official_host, bots)
- `config/plugin-state.toml` — Plugin enable/disable persistence
- `config/plugins/<plugin_id>.toml` — Per-plugin config (passed as JSON to dynamic plugin `#[init]`)
- `plugins/bin/` — Directory scanned for dynamic plugin binaries

## Reference Plugin Examples

Two example plugins serve as canonical references for how to write QimenBot plugins:

### Static Plugin Example (`plugins/qimen-plugin-example/`)

Full async plugin using `#[module]` + `#[command]` macros with `qimen-plugin-api`. Demonstrates:

- **`basic.rs`** — Commands with args, aliases, `role = "admin"`, `scope = "group"/"private"`, `CommandPluginSignal` (Reply/Block/Continue), `OneBotActionClient` calls (`get_group_info`, `set_group_ban`)
- **`message_demo.rs`** — `MessageBuilder` chain (`.text()`, `.at()`, `.face()`, `.image()`, `.share()`), message extraction methods (`.at_list()`, `.image_urls()`, `.has_reply()`), `KeyboardBuilder`
- **`event_demo.rs`** — `#[notice]` (GroupPoke, GroupIncreaseApprove, GroupRecall), `#[request]` (Friend, GroupInvite), `#[meta]` (Heartbeat), signals (`ApproveFriend`, `ApproveGroupInvite`)
- **`interceptor_demo.rs`** — `MessageEventInterceptor` trait (`pre_handle`/`after_completion`), stateful per-user cooldown with `Mutex<HashMap>`

### Dynamic Plugin Example (`plugins/qimen-dynamic-plugin-example/`)

Sync FFI plugin using `#[dynamic_plugin]` proc macro. Demonstrates:

- `#[init]` / `#[shutdown]` lifecycle hooks with `PluginInitConfig`
- `#[command]` with `name`, `description`, `aliases`, `category`, `role`, `scope` attributes
- `CommandResponse::builder()` chain (`.reply()`, `.at()`, `.text()`, `.face()`)
- `CommandResponse::text()` shorthand
- `BotApi::send_group_msg()` for proactive messaging
- `SendBuilder::private()` for rich-media outbound messages
- `#[pre_handle]` interceptor with `InterceptorRequest`/`InterceptorResponse`
- `#[route(kind = "notice", events = "...")]` for system event routing

## Reference Projects (`参考项目/`)

### Douluo Game References (斗罗插件原始参考实现)

These are the **original Java implementations** that the Rust `qimen-dynamic-plugin-douluo` is porting from. When implementing or fixing douluo plugin features, always cross-reference these for correct game mechanics.

**`dldlol-Bot-v1/`** — v1 Java/Maven/SpringBoot + Shiro + MySQL
- Registration command: **`开始穿越`** (format: `开始穿越 名字-性别`)
- Currency: **可自定义**，存储在 `command` 表 JSON 中（如 `{"点券": "null", "联邦币": "null"}`），玩家钱包为 `user.money` JSON 字段
- Key source: `src/main/java/com/mryunqi/qimenbot/Plugin/Register.java`（注册）, `Controller/Money.java`（货币）, `Controller/Battle.java`（战斗）
- Battle command: `挑战 <魂兽名>`（非"猎杀"）, attack cooldown 7秒

**`DouluoBot-v2/`** — v2 Java/Gradle/SpringBoot 3 + Shiro + SQLite + Liquibase
- Registration command: **`开始穿越`** (alias: `开始转生`，format: `开始穿越 角色名-性别`)
- Currency: **完全可自定义**，独立 `wallet` 表（`player_id` + `currency` 字符串 + `balance`），默认货币"金币"
- Transfer format: `转账 [币种][@对方/QQ]-[金额]`
- Has admin web dashboard (React), HTML-to-image rendering (Playwright)
- Game config fully database-driven (`game_config` table with categories)
- Key source: `src/main/java/com/mryunqi/douluobot/infrastructure/bot/command/handler/`

**Douluo plugin vs reference discrepancies (current known issues):**
- Rust plugin uses `注册` as command name → should be `开始穿越` (or alias)
- Rust plugin hardcodes `gold`/`coin` currency → should support customizable currency names
- Rust plugin uses `猎杀` for battle → v1 uses `挑战`, v2 also uses `挑战`

### Bot Framework References

**`Kovi/`** — Rust OneBot V11 plugin framework. Reference for Rust bot architecture patterns.

**`Shiro/`** — Kotlin/Java OneBot V11 framework (used by both douluo v1 and v2). Reference for dynamic plugin loading via JAR/ServiceLoader.

**`onebot-11/`** — OneBot V11 protocol specification. Authoritative reference for API, events, and communication formats.

**`shiro-docs/`** — Shiro framework documentation. Reference for OneBot framework documentation patterns.

## Framework Cleanliness Rules

**CRITICAL**: The framework codebase (root workspace under `crates/`, `apps/`, `plugins/qimen-plugin-*`) must remain plugin-agnostic. Follow these rules strictly:

1. **No plugin-specific content in framework code** — Test cases, comments, examples in `crates/` must use generic placeholders (e.g. `创建角色`, `echo`, `ping`), never reference any specific plugin (e.g. no `斗罗`, `douluo`, `开始穿越`, `猎杀`, `武魂` etc.)
2. **No plugin config files committed** — `config/plugins/*.toml` files are user-local plugin configs and must NOT be committed to git. Only `config/plugins/.gitkeep` should be tracked.
3. **No plugin binaries committed** — `plugins/bin/` is for runtime-loaded `.so/.dll/.dylib` files and must NOT be tracked.
4. **No plugin database files committed** — `*.db`, `*.db-shm`, `*.db-wal` must NOT be tracked.
5. **Commit messages for framework changes** — Must describe the framework feature generically. Never mention specific plugin names in commit messages for framework code.
6. **Dynamic plugin directories** (`plugins/qimen-dynamic-plugin-*`) are independent workspaces and should be committed/managed separately from the framework.

## Documentation

- `docs/` — VitePress site with guide, API, plugin, and advanced sections
- `.claude/commands/plugin.md` — Plugin development skill (use `/plugin` to invoke)
