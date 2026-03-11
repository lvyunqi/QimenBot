<div align="center">

<img src="logo.jpg" width="200" alt="QimenBot Logo">

# QimenBot

_✨ High-performance multi-protocol bot framework built with Rust ✨_

[![License](https://img.shields.io/github/license/lvyunqi/QimenBot?style=flat-square)](https://github.com/lvyunqi/QimenBot/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org/)
[![OneBot 11](https://img.shields.io/badge/OneBot-11-black?style=flat-square)](https://github.com/botuniverse/onebot-11)

[简体中文](README.md) | **English** | [日本語](README_JA.md)

</div>

---

QimenBot is a modular, extensible chatbot framework written in Rust. It separates a **reusable framework layer** from a **reference host implementation**, so you can either deploy the official host directly or build your own bot platform on top of the framework.

## Features

- **Multi-protocol** — OneBot 11 (production-ready), OneBot 12 / Satori (extension points reserved)
- **Multiple transports** — Forward WebSocket, Reverse WebSocket, HTTP API, HTTP POST
- **Declarative plugin development** — Write a full plugin in ~7 lines with `#[module]` / `#[commands]` / `#[notice]` macros
- **Interceptor chain** — `pre_handle` / `after_completion` hooks for blacklists, permission checks, shortcut rewriting, etc.
- **Flexible command system** — Aliases, examples, categories, role requirements, message filters, auto-generated `/help`
- **System event routing** — Group notices, friend requests, meta events, all dispatched via attribute routing
- **Runtime protection** — Token-bucket rate limiting, message dedup, group event filtering, plugin ACL
- **Dynamic plugins** — Load ABI-stable shared libraries via `dlopen` at runtime
- **Request automation** — Auto-approve/reject friend & group requests with whitelist/blacklist/keyword filters
- **Comprehensive OneBot 11 API** — 40+ wrapped operations: messaging, group admin, files, guilds, reactions, and more

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Application Layer (apps/)             │
│          qimenbotd (daemon)      qimenctl (CLI)        │
├─────────────────────────────────────────────────────┤
│                  Official Host Layer                   │
│    qimen-official-host · qimen-config · observability  │
├─────────────────────────────────────────────────────┤
│                Framework Layer (reusable)               │
│   runtime · plugin-api · plugin-host · message         │
│   protocol-core · transport-core · command-registry    │
├─────────────────────────────────────────────────────┤
│                  Adapters & Transports                  │
│   adapter-onebot11 · transport-ws · transport-http     │
├─────────────────────────────────────────────────────┤
│                  Built-in Modules                       │
│   mod-command · mod-admin · mod-scheduler · mod-bridge  │
└─────────────────────────────────────────────────────┘
```

## Quick Start

### Requirements

- Rust 1.89+ (2024 Edition)
- An OneBot 11 implementation (e.g., [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core), [NapCat](https://github.com/NapNeko/NapCatQQ), etc.)

### Build & Run

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot

# Edit config (change endpoint, owners, etc.)
vim config/base.toml

# Run
cargo run
```

### Configuration Example

```toml
[runtime]
env = "dev"

[official_host]
builtin_modules = ["command", "admin", "scheduler"]
plugin_modules  = ["example-plugin"]

[[bots]]
id        = "qq-main"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"
enabled   = true
owners    = ["123456"]

# Auto-reply to poke
auto_reply_poke_enabled = true
auto_reply_poke_message = "Stop poking me!"
```

Environment variable expansion is supported: `access_token = "${QQ_TOKEN}"`

## Plugin Development

QimenBot uses proc macros to minimize boilerplate:

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin", version = "0.1.0")]
#[commands]
impl MyPlugin {
    /// Command name is inferred from method name (my_cmd → "my-cmd")
    #[command("Say hello")]
    async fn hello(&self) -> &str {
        "Hello from QimenBot!"
    }

    /// Supports parameter injection: args, ctx, or both
    #[command("Echo message", aliases = ["e"])]
    async fn echo(&self, args: Vec<String>) -> Message {
        Message::text(args.join(" "))
    }

    /// System event routing
    #[notice(GroupPoke, PrivatePoke)]
    async fn on_poke(&self) -> Message {
        Message::text("Stop poking!")
    }
}
```

### Automatic Return Value Wrapping

Methods can return any of these types — the framework converts them automatically:

| Return Type | Behavior |
|------------|----------|
| `Message` | Reply with the message |
| `String` / `&str` | Reply with text |
| `CommandPluginSignal` | Full control (Reply / Continue / Block / Ignore) |
| `Result<T, E>` | Ok → normal handling, Err → reply `"Error: {e}"` |

### Interceptors

Pre/post-process events before they reach plugins:

```rust
pub struct MyInterceptor;

#[async_trait]
impl MessageEventInterceptor for MyInterceptor {
    async fn pre_handle(&self, _bot_id: &str, event: &NormalizedEvent) -> bool {
        // Return false to block the event, true to pass through
        true
    }

    async fn after_completion(&self, _bot_id: &str, _event: &NormalizedEvent) {
        // Runs after all plugins finish (in reverse order)
    }
}

#[module(id = "my-plugin", interceptors = [MyInterceptor])]
#[commands]
impl MyPlugin { /* ... */ }
```

### Event Processing Pipeline

```
Event received
  → System event dispatch (notice / request / meta)
  → Message deduplication
  → Group event filtering
  → Token-bucket rate limiting
  → Interceptor chain pre_handle
  → Permission resolution
  → Command matching & plugin dispatch
  → Interceptor chain after_completion
```

## Built-in Commands

| Command | Description |
|---------|-------------|
| `ping` / `/ping` | Returns pong |
| `echo <text>` / `/echo <text>` | Echoes text |
| `status` / `/status` | Runtime status |
| `help` / `/help` | Auto-generated help |
| `plugins` / `/plugins` | Loaded plugin list |

Trigger methods: direct message, `/prefix`, `@bot mention`, reply-based.

## Project Structure

```
QimenBot/
├── apps/
│   ├── qimenbotd/           # Bot daemon
│   └── qimenctl/            # CLI management tool
├── crates/
│   ├── qimen-plugin-api/    # Plugin API (CommandPlugin, SystemPlugin, Module)
│   ├── qimen-plugin-derive/ # Proc macros (#[module], #[commands], #[command]...)
│   ├── qimen-runtime/       # Event dispatch, plugin orchestration, interceptors
│   ├── qimen-message/       # Message model (Segment, MessageBuilder)
│   ├── qimen-adapter-onebot11/ # OneBot 11 adapter
│   ├── qimen-transport-ws/  # WebSocket transport (TLS, auto-reconnect)
│   ├── qimen-transport-http/# HTTP transport
│   ├── qimen-mod-command/   # Command detection & matching
│   ├── qimen-mod-admin/     # Permission management
│   ├── qimen-mod-scheduler/ # Cron-based task scheduling
│   └── ...                  # More core crates
├── plugins/
│   └── qimen-plugin-example/# Example plugin
└── config/
    ├── base.toml            # Main configuration
    ├── dev.toml             # Development overrides
    └── prod.toml            # Production overrides
```

## Protocol Support

| Protocol | Status | Transports |
|----------|--------|-----------|
| OneBot 11 | ✅ Production-ready | WS Forward, WS Reverse, HTTP API, HTTP POST |
| OneBot 12 | 🔲 Planned | — |
| Satori | 🔲 Planned | — |

## Acknowledgments

QimenBot's design draws inspiration from these excellent projects:

- [Shiro](https://github.com/MisakaTAT/Shiro) — Java-based OneBot framework; inspiration for interceptor and plugin model
- [Kovi](https://github.com/ThriceCola/Kovi) — Rust OneBot framework; reference for clean API design

## License

[MIT](LICENSE)
