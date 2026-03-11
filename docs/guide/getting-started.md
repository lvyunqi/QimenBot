# 快速开始

本指南将帮助你从零开始运行一个 QimenBot 实例。

## 环境准备

### 必需工具

| 工具 | 版本要求 | 说明 |
|------|---------|------|
| Rust | 1.89+（2024 Edition） | 编译框架和静态插件 |
| OneBot 11 实现 | 任意兼容实现 | 与聊天平台的桥梁 |

### 推荐的 OneBot 11 实现

QimenBot 通过 OneBot 11 协议与聊天平台通信，你需要选择一个 OneBot 实现作为中间层：

| 实现 | 说明 |
|------|------|
| [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core) | 基于 NTQQ 协议的 C# 实现，稳定可靠 |
| [NapCat](https://github.com/NapNeko/NapCatQQ) | 基于 NTQQ 的 TypeScript 实现 |

::: tip 工作原理
```
用户消息 → 聊天平台 → OneBot 实现 → WebSocket/HTTP → QimenBot
```
QimenBot 不直接连接聊天平台，而是通过 OneBot 实现转发消息。你需要先部署一个 OneBot 实现，然后让 QimenBot 连接它。
:::

## 安装 Rust

如果你还没有安装 Rust，运行以下命令：

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 从 https://rustup.rs 下载安装器
```

安装完成后验证版本：

```bash
rustc --version
# 输出应该 >= 1.89.0
```

## 获取源码

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot
```

## 配置

### 最小配置

编辑 `config/base.toml`，修改以下关键项：

```toml
[runtime]
env = "dev"

[observability]
level = "info"

[official_host]
builtin_modules = ["command", "admin"]
plugin_modules  = ["example-plugin"]

[[bots]]
id        = "my-bot"              # Bot 唯一标识
protocol  = "onebot11"            # 通信协议
transport = "ws-forward"          # 传输方式
endpoint  = "ws://127.0.0.1:3001" # OneBot 实现的 WS 地址
enabled   = true
owners    = ["你的QQ号"]           # Bot 所有者（最高权限）
```

::: warning 必须修改的字段
- `endpoint` — 改为你的 OneBot 实现的 WebSocket 地址
- `owners` — 改为你自己的 QQ 号
:::

### 完整配置参考

参见 [配置详解](/guide/configuration) 了解所有配置项。

## 运行

```bash
cargo run
```

首次编译可能需要几分钟。编译完成后你会看到类似输出：

```
INFO  qimen_official_host > 加载内置模块: command, admin
INFO  qimen_official_host > 加载插件模块: example-plugin
INFO  qimen_transport_ws  > 正在连接 ws://127.0.0.1:3001 ...
INFO  qimen_transport_ws  > WebSocket 连接已建立
INFO  qimen_runtime       > Bot [my-bot] 已就绪
```

## 验证

向 Bot 发送消息测试：

| 发送内容 | 预期回复 | 说明 |
|---------|---------|------|
| `/ping` | `pong!` | 基础连通性测试 |
| `/echo hello` | `hello` | 回显命令 |
| `/help` | 命令列表 | 查看所有可用命令 |
| `/status` | 运行时状态 | 查看 Bot 状态信息 |

::: tip 命令触发方式
QimenBot 支持多种命令触发方式：
- **私聊直发** — 直接发送 `ping` 或 `/ping`
- **群聊前缀** — 群内发送 `/ping`
- **@提及** — 群内发送 `@Bot ping`
- **回复触发** — 回复 Bot 的消息并输入命令
:::

## 下一步

- 📖 [配置详解](/guide/configuration) — 了解所有配置选项
- 🏗️ [架构设计](/guide/architecture) — 理解框架的整体设计
- 🔌 [插件开发](/plugin/overview) — 开始写你的第一个插件
- 📡 [事件处理](/plugin/events) — 学习如何处理系统事件
