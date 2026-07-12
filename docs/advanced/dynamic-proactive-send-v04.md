# Dynamic Plugin Proactive Send API 0.4

## Delivery target

API 0.4 adds real-time proactive sends from dynamic plugin background workers. Requests select a concrete bot and carry a protocol-neutral target plus JSON routing extensions. API 0.1-0.3 plugins keep the callback-flush path.

## Implementation sequence

- [x] Add the versioned request, status codes, host function table, and bot-scoped plugin API.
- [x] Generate API 0.4 bind/unbind exports while leaving the macro default at API 0.3.
- [x] Add per-bot bounded queues, online executors, WebSocket sender handles, TTL, and shutdown behavior.
- [x] Cover private, group, channel, and channel-private routing for OneBot 11 and QQ official bots.
- [ ] Update examples, templates, configuration, and public documentation.
- [ ] Verify registry-only plugin builds, publish the two dependency crates, and tag v0.1.10.

## Fixed behavior

- Queue capacity defaults to 256 and offline TTL defaults to 60 seconds.
- A request must name a bot; the host derives the protocol from that bot configuration.
- Each bot preserves send order. Requests wait only before the first network attempt and are not retried after an attempted send.
- Host callbacks copy plugin strings before returning. Unbind waits for in-flight callbacks before the host context or library is released.
