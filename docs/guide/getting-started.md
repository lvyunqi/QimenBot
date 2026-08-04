# 快速开始

本指南说明 QimenBot 的安装、机器人连接配置和首次启动流程。生产服务器的 Docker、systemd、Windows Service、备份和升级步骤见[完整部署指南](/guide/deployment)。

## 环境准备

QimenBot 支持两条接入路径。使用 QQ 官方 Bot 时直接连接官方 Gateway 和 OpenAPI；使用个人 QQ 或其他 OneBot 场景时，通过 OneBot 实现转发事件和操作。

```
QQ 官方 Bot：用户消息 → QQ Gateway → QimenBot → OpenAPI 回复
OneBot 11： 用户消息 → QQ → OneBot 实现 → WebSocket → QimenBot → OneBot 操作
```

按接入方式准备组件：

| 组件 | 说明 | 准备工作 |
|------|------|----------|
| **QimenBot** | Bot 宿主与管理面板 | Docker、Release 或源码三选一 |
| **QQ 官方 Bot 凭据** | 官方 Gateway 和 OpenAPI 鉴权 | 仅官方 Bot 需要 |
| **OneBot 11 实现** | 充当聊天平台与 QimenBot 之间的桥梁 | 仅 OneBot 接入需要 |
| **Rust 与 Node.js** | 编译 daemon 和管理面板 | 仅源码构建需要 |

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

### 方式一：Docker Compose

Linux 服务器已经安装 Docker 时，QQ 官方机器人可以一键安装：

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/lvyunqi/QimenBot/main/deploy/docker/install.sh)
```

脚本会询问 AppID 和 Secret，自动生成管理 Token，并创建配置、插件和日志目录。NAS、Portainer、OneBot 以及自定义数据盘的步骤见[Docker Compose 部署](/guide/deployment#手动使用-docker-compose)。

### 方式二：下载预编译版本（推荐新手）

前往 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases) 下载对应平台的压缩包，解压即可运行。

支持平台：Linux x86_64/ARM64、macOS x86_64/ARM64、Windows x86_64。

下载后保留完整目录。普通用户只运行根目录的 `qimenbot`，不要单独运行 `runtime/qimenbotd`。然后跳到[修改配置](#修改配置)继续。

### 方式三：从源码编译

源码用户继续完成下面两项准备；使用 Release 的用户可以直接跳到[修改配置](#修改配置)。

#### 安装构建工具

只有源码构建需要这一步。安装 Node.js 22 和 Rust 1.89+；未安装 Rust 时执行：

```bash
# Linux / macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows：从 https://rustup.rs 下载安装器
```

验证安装：

```bash
rustc --version
# 输出应该 >= 1.89.0
node --version
# 输出应该是 v22.x
```

#### 获取源码

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot
```

## 修改配置

Release 压缩包先复制示例文件；源码仓库已经包含 `config/base.toml`。下面两条复制命令只适用于 Release 压缩包：

```bash
cp -n config/base.toml.example config/base.toml
cp -n config/qimenbot.toml.example config/qimenbot.toml
```

Windows PowerShell 可使用 `Copy-Item`。如果当前目录中没有 `config/qimenbot.toml.example`，说明正在使用源码仓库，直接编辑已有的 `config/base.toml`，源码开发时运行 `cargo run --package qimenbotd`。

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

## 启动

下载预编译版本时启动 QimenBot：

```bash
# Linux / macOS
chmod +x qimenbot
./qimenbot run

# Windows PowerShell
.\qimenbot.exe run
```

从源码开发时先构建管理面板，再启动 daemon：

```bash
npm --prefix web/admin ci
npm --prefix web/admin run build
cargo run --package qimenbotd
```

::: info 首次编译
首次运行需要下载 npm 和 Cargo 依赖并编译，耗时取决于网络与机器配置。后续增量构建会快很多。
:::

看到类似这样的日志就说明启动成功了：

```
INFO  qimen_official_host > 加载内置模块: command, admin
INFO  qimen_official_host > 加载插件模块: example-plugin
INFO  qimen_transport_ws  > 正在连接 ws://127.0.0.1:3001 ...
INFO  qimen_transport_ws  > WebSocket 连接已建立
INFO  qimen_runtime       > Bot [my-bot] 已就绪
```

## 验证

先确认 `plugin_modules` 中启用了示例插件 `example-basic`，再向 Bot 发送消息：

| 发送内容 | Bot 回复 | 说明 |
|--------|---------|------|
| `/ping` | `pong!` | 基础连通性测试 |
| `/echo hello` | `hello` | 回显命令 |
| `/help` | 第 1 页命令目录 | 页数较多时使用 `/help 2` |
| `/whoami` | 当前身份 | 由示例插件返回会话和权限信息 |

`ping`、`echo` 和 `whoami` 都来自示例插件，不是 Runtime 内置命令。删除或停用 `example-basic` 后，它们不会回复，也不会继续出现在帮助目录中。

::: tip 命令触发方式
QimenBot 支持多种方式触发命令：

| 方式 | 示例 | 适用场景 |
|------|------|---------|
| **斜杠前缀** | `/ping` | 群聊和私聊 |
| **直接输入** | `ping` | 仅私聊 |
| **@提及** | `@Bot ping` | 群聊 |
| **回复消息** | (回复 Bot 消息) `ping` | 群聊和私聊 |

这些入口可以在 Web 管理面板“配置 → 命令入口”中分别开关。前缀也可从 `/` 改为多个自定义值。
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

- 检查负责该命令的插件是否已启用并成功加载
- 确认 OneBot 实现是否正常登录
- 查看终端日志是否有错误信息
- `example-basic` 已启用时，可在私聊直接发送 `ping`；若关闭了“私聊直接输入”，请发送 `/ping`
- 对于 `ws-reverse`，使用 `qimenctl simulate-onebot11` 区分客户端连接问题、命令未注册和插件回调问题

## 下一步

完成首次启动后，可继续阅读：

- [插件开发概览](/plugin/overview) — 静态插件结构与注册流程
- [配置详解](/guide/configuration) — 配置项和覆盖规则
- [架构设计](/guide/architecture) — 框架分层与事件链路
- [事件处理](/plugin/events) — 通知、请求和元事件处理
