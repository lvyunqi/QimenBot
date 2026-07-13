# 快速开始

本指南说明 QimenBot 的安装、OneBot 连接配置和首次启动流程。

## 环境准备

QimenBot 不直接连接 QQ 等聊天平台，而是通过 **OneBot 协议** 与一个"中间人"通信。整体结构如下：

```
用户消息 → QQ → OneBot 实现 → WebSocket → QimenBot → 处理 → 回复
```

运行环境包含以下组件：

| 组件 | 说明 | 准备工作 |
|------|------|----------|
| **Rust 编译器** | 编译 QimenBot 框架 | 安装 Rust 1.89+ |
| **OneBot 11 实现** | 充当 QQ 与 QimenBot 之间的桥梁 | 选择一个实现并部署 |

### 推荐的 OneBot 11 实现

| 名称 | 语言 | 特点 |
|------|------|------|
| [Lagrange.OneBot](https://github.com/LagrangeDev/Lagrange.Core) | C# | 基于 NTQQ 协议，稳定可靠 |
| [NapCat](https://github.com/NapNeko/NapCatQQ) | TypeScript | 基于 NTQQ，配置简单 |

::: tip OneBot 的作用
OneBot 是聊天平台实现与 Bot 框架之间的标准协议。Lagrange、NapCat 等实现负责登录 QQ 和收发消息，并按统一格式与 QimenBot 通信。
:::

::: tip 使用官方 QQ Bot？
官方 QQ Bot 不需要 OneBot 实现端，它通过官方 Gateway 和 OpenAPI 接入。请按 [官方 QQ Bot 接入](/guide/qq-official-quickstart) 配置 `protocol = "qq-official"`。
:::

## 获取 QimenBot

### 方式一：下载预编译版本（推荐新手）

前往 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases) 下载对应平台的压缩包，解压即可运行。

支持平台：Linux x86_64、macOS x86_64/ARM64、Windows x86_64。

下载后跳到 [第 3 步：修改配置](#第-3-步-修改配置) 继续。

### 方式二：从源码编译

## 第 1 步：安装 Rust

未安装 Rust 时，执行：

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

编辑 `config/base.toml` 中的以下两项：

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
endpoint  = "ws://127.0.0.1:3001"  # ← OneBot WebSocket 地址
enabled   = true
owners    = ["管理员QQ号"]           # ← 具有最高权限的 QQ 号
```

::: warning 必须修改
- **`endpoint`** — OneBot 实现提供的 WebSocket 地址
- **`owners`** — 具有最高权限的 QQ 号，使用字符串格式
:::

::: tip 不确定 OneBot 的地址？
不同 OneBot 实现的默认端口不同，常见端口包括 `3001`、`6700` 和 `8080`。具体值以对应实现的文档为准。
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

| 发送内容 | Bot 回复 | 说明 |
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

### 不连接 QQ 客户端进行内部链路测试

如果当前 Bot 使用 `ws-reverse`，可以让 `qimenctl` 模拟 OneBot 11 客户端发送标准消息事件，并自动确认框架返回的 Action：

```bash
cargo run -p qimenctl -- simulate-onebot11 \
  --bot qq-reverse \
  --message /ping \
  --user-id 10000 \
  --self-id 10001
```

测试前请断开真实 OneBot 客户端，或使用独立测试 Bot。若 CLI 能打印 `send_msg` Action 并显示 `acknowledged`，说明 WebSocket、事件解码、命令匹配、插件回调和发送响应链路均已通过。群聊测试增加 `--group-id <群号>`；原始事件重放、Token 和超时选项见[传输层：使用 qimenctl 模拟 OneBot 11 客户端](/advanced/transport#使用-qimenctl-模拟-onebot-11-客户端)。

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
- 对于 `ws-reverse`，使用 `qimenctl simulate-onebot11` 区分客户端连接问题、命令未注册和插件回调问题

## 下一步

完成首次启动后，可继续阅读：

- [插件开发概览](/plugin/overview) — 静态插件结构与注册流程
- [配置详解](/guide/configuration) — 配置项和覆盖规则
- [架构设计](/guide/architecture) — 框架分层与事件链路
- [事件处理](/plugin/events) — 通知、请求和元事件处理
