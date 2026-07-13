# 架构设计

QimenBot 采用分层架构，将可复用的框架层与具体的 Host 实现分离。

## 整体架构

```
┌─────────────────────────────────────────────────────────┐
│                     应用层 (apps/)                        │
│          qimenbotd (守护进程)    qimenctl (CLI)           │
├─────────────────────────────────────────────────────────┤
│                  Official Host 层                         │
│    qimen-official-host · qimen-config · observability     │
├─────────────────────────────────────────────────────────┤
│                  Framework 层（可复用）                     │
│   runtime · plugin-api · plugin-host · message            │
│   protocol-core · transport-core · command-registry       │
├─────────────────────────────────────────────────────────┤
│                    适配器 & 传输                           │
│   adapter-onebot11 · adapter-qqbot                        │
│   transport-ws · transport-http · transport-qqbot         │
├─────────────────────────────────────────────────────────┤
│                     内置模块                               │
│   mod-command · mod-admin · mod-scheduler · mod-bridge     │
└─────────────────────────────────────────────────────────┘
```

### 层级说明

| 层级 | 说明 | 可复用性 |
|------|------|---------|
| **应用层** | 可执行文件入口，直接部署 | 可替换 |
| **Official Host 层** | 配置加载、模块注册、启动编排 | 参考实现 |
| **Framework 层** | 事件分发、插件系统、消息模型 | ✅ 核心复用 |
| **适配器 & 传输** | 协议解析、网络通信 | 按协议选择 |
| **内置模块** | 框架提供的功能模块 | 按需加载 |

## Crate 依赖关系

```
qimenbotd (应用入口)
  └── qimen-official-host (模块编排)
        ├── qimen-config (配置解析)
        ├── qimen-runtime (事件循环)
        │     ├── qimen-plugin-api (插件接口)
        │     │     ├── qimen-message (消息模型)
        │     │     ├── qimen-protocol-core (协议抽象)
        │     │     └── qimen-plugin-derive (过程宏)
        │     ├── qimen-command-registry (命令注册表)
        │     ├── qimen-mod-command (命令检测)
        │     ├── qimen-mod-admin (权限管理)
        │     └── qimen-host-types (宿主类型)
        ├── qimen-adapter-onebot11 (OneBot 11 协议适配)
        ├── qimen-adapter-qqbot (官方 QQ Bot 协议适配)
        ├── qimen-transport-ws (通用 WebSocket)
        ├── qimen-transport-http (HTTP)
        ├── qimen-transport-qqbot (官方 QQ Bot Gateway/OpenAPI)
        └── abi-stable-host-api (动态插件 FFI)
```

## 核心组件详解

### Runtime（运行时）

`Runtime` 是框架的核心引擎，负责：

1. **事件循环** — 从传输层接收事件，分发到插件处理
2. **命令路由** — 将命令匹配到对应的 `CommandPlugin`
3. **系统事件路由** — 将通知/请求/元事件分发到 `SystemPlugin`
4. **插件编排** — 管理插件的加载、卸载、优先级排序
5. **Webhook Gateway** — 把 HTTP 请求安全地路由到 API 0.5 动态插件
6. **运行时保护** — 限流、去重、ACL、FFI 生命周期屏障等安全机制

### Webhook Gateway

API 0.5 Webhook 不经过 Bot 事件解码流水线，而是由 Runtime 的独立 HTTP 入口直接调度：

```text
HTTP request
    → Bearer token / body / concurrency checks
    → method + namespaced exact-path lookup
    → spawn_blocking synchronous FFI callback
    → host-owned response copy
    → HTTP response
```

插件只声明局部路径；宿主自动挂载为 `{base_path}/{plugin_id}{path}`。同步回调执行期间会持有动态库生命周期读锁，热重载和关闭取得写锁前必须等待所有回调真正返回，因此 HTTP 超时不会造成动态库被提前卸载。Webhook 中的 Bot 发送仍进入每 Bot 独立的主动发送队列，协议由 `bot_id` 对应配置决定。

### ProtocolAdapter（协议适配器）

协议适配器将特定协议的数据格式转换为框架统一的 `NormalizedEvent`：

```
原始 OneBot11 JSON     → OneBot11Adapter → NormalizedEvent
官方 QQ Bot Gateway  → QqBotAdapter    → NormalizedEvent
                                               ↓
                                         框架统一处理
```

添加新协议（如官方 QQ Bot、OneBot 12、Satori）时，实现对应适配器和传输封装即可，无需改写命令、权限、限流、拦截器和插件流水线。

### NormalizedActionExecutor（动作执行器）

消息处理完成后，插件回复会生成统一的 `NormalizedActionRequest`。不同协议负责把它转换成自己的动作通道：

```
CommandPluginSignal::Reply
        ↓
NormalizedActionRequest
        ├── OneBot11ActionExecutor → WebSocket action
        └── QqOfficialActionExecutor → 官方 OpenAPI HTTP 请求
```

官方 QQ Bot 的事件来自 Gateway WebSocket，但发送消息、上传媒体、撤回消息都走 HTTP OpenAPI，因此它拥有独立的 `qimen-transport-qqbot`。发送失败会归一化成 `ActionStatus::Failed`，不会打断 Gateway 会话。

### CommandRegistry（命令注册表）

命令注册表是全局的命令路由表，负责：

- 注册命令名和别名
- 基于优先级的命令查找
- 冲突检测和诊断
- 按分类分组展示

### Message（消息模型）

消息由多个 `Segment`（消息段）组成，每个段表示一种内容类型：

```
Message
  ├── Segment::Text("Hello ")
  ├── Segment::At(123456)
  ├── Segment::Face(1)
  └── Segment::Image("https://example.com/img.png")
```

消息模型与 OneBot 协议的消息段格式兼容，支持双向转换。

## 事件处理流程

从收到消息到最终回复，事件经过以下处理步骤：

```
┌──────────────────────────────────────────────────┐
│                   收到事件                         │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│           协议适配（decode_event）                  │
│     原始 JSON → NormalizedEvent                    │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│             事件类型判断                            │
│   Message? Notice? Request? Meta?                  │
└───────┬─────────────┬─────────────┬──────────────┘
        ▼             ▼             ▼
   消息事件流程    系统事件分发      元事件分发
        │         (SystemPlugin)   (SystemPlugin)
        ▼
┌──────────────────────────────────────────────────┐
│              消息去重（MessageDedup）                │
│          检查 message_id 是否已处理                  │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│            群事件过滤（GroupEventFilter）             │
│           检查群号是否在白名单/黑名单中                │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│           令牌桶限流（TokenBucketLimiter）           │
│             检查是否超过频率限制                       │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│          拦截器链 pre_handle                        │
│    LoggingInterceptor → CooldownInterceptor → ... │
│    任何拦截器返回 false 则中止                        │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│              权限解析                               │
│    判断发送者的角色：Owner / Admin / Anyone           │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│         命令匹配 & 插件分发                          │
│    CommandRegistry.match_command()                  │
│    CommandPlugin.on_command()                       │
└─────────────────────┬────────────────────────────┘
                      ▼
┌──────────────────────────────────────────────────┐
│          拦截器链 after_completion                   │
│    ... → CooldownInterceptor → LoggingInterceptor │
│    （逆序执行）                                      │
└──────────────────────────────────────────────────┘
```

## 多 Bot 实例管理

QimenBot 支持在一个进程中运行多个 Bot 实例：

```
┌─────────────────────────────────────┐
│         QimenBot 进程                │
│                                     │
│  ┌─────────────┐ ┌─────────────┐   │
│  │  Bot: qq-1   │ │  Bot: qq-2   │   │
│  │  WS 正向     │ │  WS 反向     │   │
│  │  模块: A,B,C │ │  模块: A,B   │   │
│  └──────┬──────┘ └──────┬──────┘   │
│         │               │           │
│  ┌──────▼──────┐ ┌──────▼──────┐   │
│  │ OneBot 实现1 │ │ OneBot 实现2 │   │
│  └─────────────┘ └─────────────┘   │
└─────────────────────────────────────┘
```

每个 Bot 实例有独立的：
- 传输连接（WebSocket / HTTP）
- 模块启用列表
- 权限配置（owners / admins）
- 审批策略
- 限流器设置

但它们共享同一套模块代码，节约内存。

## 项目目录结构

```
QimenBot/
├── apps/
│   ├── qimenbotd/               # Bot 守护进程入口
│   └── qimenctl/                # CLI 管理工具
├── crates/
│   ├── qimen-plugin-api/        # 插件 API（核心接口）
│   ├── qimen-plugin-derive/     # 过程宏
│   ├── qimen-runtime/           # 事件分发与插件编排
│   ├── qimen-message/           # 消息模型
│   ├── qimen-adapter-onebot11/  # OneBot 11 协议适配
│   ├── qimen-adapter-qqbot/     # 官方 QQ Bot 协议适配
│   ├── qimen-transport-ws/      # WebSocket 传输
│   ├── qimen-transport-http/    # HTTP 传输
│   ├── qimen-transport-qqbot/   # 官方 QQ Bot Gateway/OpenAPI
│   ├── qimen-mod-command/       # 命令检测模块
│   ├── qimen-mod-admin/         # 权限管理模块
│   ├── qimen-mod-scheduler/     # 定时任务模块
│   ├── qimen-mod-bridge/        # 消息桥接模块
│   ├── qimen-command-registry/  # 命令注册表
│   ├── qimen-config/            # 配置解析
│   ├── qimen-official-host/     # Official Host 实现
│   ├── qimen-protocol-core/     # 协议核心抽象
│   ├── qimen-host-types/        # 宿主类型定义
│   ├── qimen-error/             # 统一错误类型
│   └── abi-stable-host-api/     # 动态插件 FFI
├── plugins/
│   ├── qimen-plugin-example/    # 静态插件示例
│   └── qimen-dynamic-plugin-example/ # 动态插件示例
└── config/
    ├── base.toml                # 主配置文件
    └── bots/                    # Bot 独立配置参考
```
