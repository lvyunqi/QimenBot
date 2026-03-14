# 运行时原理

本页深入讲解 QimenBot 运行时的内部工作机制，帮助你理解框架如何管理 Bot 实例、分发事件、保护系统稳定。

## Runtime 核心引擎

`Runtime` 是框架的核心，负责协调所有组件工作：

```
Runtime
├── CommandDispatcher      命令路由
├── OneBotSystemDispatcher 系统事件路由
├── InterceptorChain       拦截器链
├── TokenBucketLimiter     令牌桶限流
├── MessageDedup           消息去重
├── GroupEventFilter       群事件过滤
├── PluginAclManager       插件 ACL
├── PermissionResolver     权限解析
└── DynamicPluginRuntime   动态插件执行
```

## 启动流程

```
main()
  → run_official_host("config/base.toml")
    → 加载配置 (AppConfig::load_from_path)
    → 初始化日志 (observability)
    → 注册内置模块 (command, admin, scheduler, bridge)
    → 注册插件模块 (plugin_modules)
    → 扫描动态插件 (plugin_bin_dir)
    → 调用动态插件 #[init] 钩子（TOML→JSON 配置传入）
    → 对每个 [[bots]] 配置:
        → 创建 Runtime 实例
        → 建立传输连接 (WebSocket / HTTP)
        → 进入事件循环
```

## 事件循环

每个 Bot 实例运行一个独立的事件循环：

```rust
loop {
    // 1. 从传输层接收原始数据
    let raw = transport.next_event().await;

    // 2. 协议适配
    let event = adapter.decode_event(raw)?;

    // 3. 根据事件类型分发
    match event.kind() {
        EventKind::Message => handle_message(event).await,
        EventKind::Notice  => dispatch_notice(event).await,
        EventKind::Request => dispatch_request(event).await,
        EventKind::Meta    => dispatch_meta(event).await,
    }
}
```

## 命令路由

### CommandDispatcher

命令调度器维护一个 `CommandRegistry`（命令注册表），负责将用户输入匹配到对应的插件：

```
用户输入: "/echo hello world"
    ↓
CommandRegistry.match_command("echo")
    ↓ 找到匹配的 CommandPlugin
CommandPlugin.on_command(ctx, invocation)
    ↓
CommandPluginSignal::Reply(message)
```

### 命令优先级

当多个插件注册了同名命令时，按**优先级**决定哪个生效：

| 来源 | 优先级值 | 说明 |
|------|---------|------|
| 内置命令 | 0 | 框架自带的命令（如 /help） |
| 静态插件 | 100 | 通过 `#[command]` 宏注册的命令 |
| 动态插件 | 200 | 通过 FFI 注册的命令 |

数值越小优先级越高。使用 `/plugins` 可以查看命令冲突诊断。

## 系统事件路由

### OneBotSystemDispatcher

系统事件调度器将 OneBot 事件映射到框架内部的路由枚举：

```
OneBot 事件 JSON:
{
    "post_type": "notice",
    "notice_type": "notify",
    "sub_type": "poke"
}
    ↓ 路由解析
GroupPoke（群戳一戳）
    ↓ 查找注册的 SystemPlugin
SystemPlugin.on_notice(ctx, SystemNoticeRoute::GroupPoke)
```

#### 路由映射规则

```
notice_type + sub_type → SystemNoticeRoute
─────────────────────────────────────────
"group_upload"                       → GroupUpload
"group_admin" + "set"                → GroupAdminSet
"group_admin" + "unset"              → GroupAdminUnset
"group_decrease" + "leave"           → GroupDecreaseLeave
"group_decrease" + "kick"            → GroupDecreaseKick
"group_decrease" + "kick_me"         → GroupDecreaseKickMe
"group_increase" + "approve"         → GroupIncreaseApprove
"group_increase" + "invite"          → GroupIncreaseInvite
"group_ban" + "ban"                  → GroupBanBan
"group_ban" + "lift_ban"             → GroupBanLiftBan
"friend_add"                         → FriendAdd
"group_recall"                       → GroupRecall
"friend_recall"                      → FriendRecall
"notify" + "poke" + (group_id存在)   → GroupPoke
"notify" + "poke" + (无group_id)     → PrivatePoke
"notify" + "lucky_king"              → NotifyLuckyKing
"notify" + "honor"                   → NotifyHonor
```

## 运行时保护机制

### 令牌桶限流

每个 Bot 可以配置独立的令牌桶限流器：

```
令牌桶 (capacity=10, rate=5.0/s)
├── 初始: 10 个令牌
├── 每 200ms 恢复 1 个令牌
├── 每条消息消耗 1 个令牌
└── 桶空时: 直接丢弃 (timeout=0) 或等待
```

### 消息去重

基于 `message_id` 的滑动窗口去重：

```
收到 msg_id=12345
    → 检查缓存 → 不存在 → 处理 + 加入缓存
收到 msg_id=12345 (重复)
    → 检查缓存 → 已存在 → 丢弃
```

### 拦截器链

拦截器按优先级排列，形成处理链：

```
→ Interceptor[0].pre_handle()  → true
→ Interceptor[1].pre_handle()  → true
→ Interceptor[2].pre_handle()  → false (拦截!)
    ↓ 不再继续

// after_completion 按逆序执行
← Interceptor[1].after_completion()
← Interceptor[0].after_completion()
```

### 插件 ACL

插件访问控制列表管理插件的启用/禁用状态：

```toml
# config/plugin-state.toml (自动管理)
[plugins]
"example-plugin" = true
"spam-plugin" = false
```

通过 `/plugins enable/disable` 命令管理，状态持久化到文件。

## 动态插件运行时

### DynamicPluginRuntime

动态插件有独立的运行时，负责：

1. **库管理** — `dlopen` 加载 / `dlclose` 卸载
2. **符号查找** — 查找回调函数符号
3. **安全调用** — 通过 `catch_unwind` 捕获 panic
4. **熔断保护** — 失败计数和自动隔离

### 生命周期钩子

动态插件支持 `#[init]` 和 `#[shutdown]` 两个生命周期钩子：

- **init** — `boot()` 时框架自动调用 `call_plugin_init()`，读取 `config/plugins/<plugin_id>.toml` 并将 TOML 转换为 JSON 后传入插件的 `#[init]` 函数。插件可在此阶段完成数据库连接、配置加载等初始化工作。
- **shutdown** — 框架关闭时通过 `call_plugin_shutdown()` 通知插件清理资源（如关闭数据库连接、刷写缓存等）。

```
boot()
  → 扫描 plugin_bin_dir, dlopen 所有 .so/.dll/.dylib
  → 对每个插件:
      → 读取 config/plugins/<plugin_id>.toml
      → TOML → JSON 转换
      → 调用 plugin_init(json_config)
  → 继续启动 Bot 实例...

shutdown()
  → 对每个插件:
      → 调用 plugin_shutdown()
  → dlclose 卸载动态库
```

### 热重载流程

当用户发送 `/plugins reload`：

```
1. 框架收到 reload 命令
2. 卸载所有已加载的动态库 (dlclose)
3. 重新扫描 plugin_bin_dir 目录
4. 加载新发现的动态库 (dlopen)
5. 获取每个库的 PluginDescriptor
6. 重建命令注册表
7. 重建系统事件路由表
8. 发送确认消息
```

整个过程**不断开** WebSocket 连接，也不影响正在处理的其他消息。

## 会话管理

每个 Bot 实例维护一个**会话（Session）**，代表与 OneBot 实现的一次连接周期：

```
会话生命周期:
  连接建立
    → 事件循环
    → [断开] → 自动重连 → 新会话
    → [热重载] → 重建调度器 → 继续
    → [停止信号] → 优雅关闭
```

### 重连策略

WebSocket 连接断开时，框架使用**指数退避**策略自动重连：

```
第 1 次重试: 等待 1 秒
第 2 次重试: 等待 2 秒
第 3 次重试: 等待 4 秒
第 4 次重试: 等待 8 秒
...
最大等待: 60 秒
```

连接稳定一段时间后，退避计数器会自动重置。

## 多 Bot 并发

多个 Bot 实例通过 Tokio 的异步任务并发运行：

```rust
let mut tasks = vec![];
for bot_config in &config.bots {
    let task = tokio::spawn(async move {
        run_bot_session(bot_config).await
    });
    tasks.push(task);
}
// 所有 Bot 并发运行，互不阻塞
futures::future::join_all(tasks).await;
```

每个 Bot 有独立的：
- Tokio 任务
- WebSocket 连接
- 事件缓冲区
- 限流器状态
- 拦截器链
