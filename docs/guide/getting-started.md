# 快速开始

本指南手把手教你从零开始运行 QimenBot。全程大约 10 分钟。

## 你需要准备什么？

QimenBot 不直接连接 QQ 等聊天平台，而是通过 **OneBot 协议** 与一个"中间人"通信。整体结构如下：

```
用户消息 → QQ → OneBot 实现 → WebSocket → QimenBot → 处理 → 回复
```

所以你需要两样东西：

| 组件 | 说明 | 你需要做的 |
|------|------|----------|
| **Rust 编译器** | 编译 QimenBot 框架 | 安装 Rust 1.89+ |
| **OneBot 11 实现** | 充当 QQ 与 QimenBot 之间的桥梁 | 选择一个实现并部署 |

### 推荐的 OneBot 11 实现

| 名称 | 语言 | 特点 |
|------|------|------|
| [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core) | C# | 基于 NTQQ 协议，稳定可靠 |
| [NapCat](https://github.com/NapNeko/NapCatQQ) | TypeScript | 基于 NTQQ，配置简单 |

::: tip 不了解 OneBot？
简单来说，OneBot 是一个**标准协议**。不同的 OneBot 实现（Lagrange、NapCat 等）负责登录 QQ、收发消息，然后按统一格式把消息转发给你的 Bot 程序。这样你只需要关心业务逻辑，不需要操心底层通信。
:::

## 第 1 步：安装 Rust

如果你还没有安装 Rust：

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows：从 https://rustup.rs 下载安装器
```

验证安装：

```bash
rustc --version
# 输出应该 >= 1.89.0
```

## 第 2 步：获取源码

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot
```

## 第 3 步：修改配置

编辑 `config/base.toml`，只需要改**两个地方**：

```toml
[runtime]
env = "dev"

[observability]
level = "info"

[official_host]
builtin_modules = ["command", "admin"]
plugin_modules  = ["example-plugin"]

[[bots]]
id        = "my-bot"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "ws://127.0.0.1:3001"  # ← 改成你的 OneBot WS 地址
enabled   = true
owners    = ["你的QQ号"]             # ← 改成你自己的 QQ 号
```

::: warning 必须修改
- **`endpoint`** — 你的 OneBot 实现的 WebSocket 地址（部署 OneBot 时会看到）
- **`owners`** — 你自己的 QQ 号（字符串格式），拥有最高权限
:::

::: tip 不确定 OneBot 的地址？
不同的 OneBot 实现默认端口不同。常见的有 `3001`、`6700`、`8080` 等。请查看你所用 OneBot 实现的文档。
:::

## 第 4 步：启动

```bash
cargo run
```

::: info 首次编译
首次运行需要下载依赖并编译，可能需要 **3-5 分钟**（取决于网速和机器配置）。后续启动会快很多。
:::

看到类似这样的日志就说明启动成功了：

```
INFO  qimen_official_host > 加载内置模块: command, admin
INFO  qimen_official_host > 加载插件模块: example-plugin
INFO  qimen_transport_ws  > 正在连接 ws://127.0.0.1:3001 ...
INFO  qimen_transport_ws  > WebSocket 连接已建立
INFO  qimen_runtime       > Bot [my-bot] 已就绪
```

## 第 5 步：验证

向 Bot 发送消息，看看它是否正常工作：

| 你发送 | Bot 回复 | 说明 |
|--------|---------|------|
| `/ping` | `pong!` | 基础连通性测试 |
| `/echo hello` | `hello` | 回显命令 |
| `/help` | 命令列表 | 查看所有可用命令 |
| `/status` | 运行状态 | 查看 Bot 信息 |

::: tip 命令触发方式
QimenBot 支持多种方式触发命令：

| 方式 | 示例 | 适用场景 |
|------|------|---------|
| **斜杠前缀** | `/ping` | 群聊和私聊 |
| **直接输入** | `ping` | 仅私聊 |
| **@提及** | `@Bot ping` | 群聊 |
| **回复消息** | (回复 Bot 消息) `ping` | 群聊和私聊 |
:::

## 常见问题

### 连接失败：`WebSocket 连接失败`

- 检查 OneBot 实现是否已启动
- 检查 `endpoint` 地址和端口是否正确
- 检查 OneBot 实现是否开启了 WebSocket 服务

### 编译错误：`Rust 版本过低`

QimenBot 需要 Rust 1.89+（2024 Edition）。运行 `rustup update` 更新 Rust。

### 发送命令无回复

- 检查 `owners` 是否填写了正确的 QQ 号
- 确认 OneBot 实现是否正常登录
- 查看终端日志是否有错误信息
- 尝试在私聊中直接发送 `ping`（无需斜杠前缀）

## 下一步

恭喜你成功运行了 QimenBot！接下来你可以：

- 🔌 [编写第一个插件](/plugin/overview) — 5 分钟上手插件开发
- 📖 [配置详解](/guide/configuration) — 了解所有配置选项
- 🏗️ [架构设计](/guide/architecture) — 理解框架的整体设计
- 📡 [事件处理](/plugin/events) — 学习处理戳一戳、入群等系统事件
