# 运行时原理

本页说明 QimenBot 运行时管理 Bot 实例、分发事件和执行保护策略的内部机制。

## Runtime 核心引擎

`Runtime` 是框架的核心，负责协调所有组件工作：

```
Runtime
├── CommandDispatcher      命令路由
├── OneBotSystemDispatcher 系统事件路由
├── NormalizedActionExecutor 协议动作执行
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
        → 根据 protocol + transport 建立会话
          → onebot11 + ws-forward/ws-reverse/http
          → qq-official + gateway
        → 进入事件循环
```

## 事件循环

每个 Bot 实例运行一个独立的事件循环。OneBot 11 的事件来自 OneBot 实现端，官方 QQ Bot 的消息和通知来自 Gateway Dispatch，二者都会先归一化成 `NormalizedEvent`：

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

### 多协议消息流水线

消息事件会进入共享的 `handle_normalized_event` 流水线：

```
NormalizedEvent::Message
  → 消息去重
  → 群事件过滤
  → 令牌桶限流
  → pre_handle 拦截器
  → 权限解析
  → 命令匹配
  → 静态/动态插件执行
  → 协议动作执行器发送回复
  → after_completion 拦截器
```

OneBot 11 和官方 QQ Bot 共用这条消息流水线。差异只保留在协议边界：

| 协议 | 事件入口 | 回复出口 | 说明 |
|------|----------|----------|------|
| `onebot11` | WebSocket/HTTP OneBot 事件 | OneBot action | 支持 OneBot 11 完整动作模型 |
| `qq-official` | 官方 Gateway Dispatch | 官方 OpenAPI | 支持 QQ 群 @/全量消息、QQ 单聊、频道消息、频道私信 |

官方 QQ Bot 的发送失败会返回 `ActionStatus::Failed` 并记录错误分类。OpenAPI 发送错误不会重启 Gateway 会话；429 频控会按 bot + route 做短期 backoff。

### 官方 QQ 消息归一化

Gateway Dispatch 到达后，`qimen-adapter-qqbot` 按事件类型建立稳定的消息上下文：

| 原始事件 | `chat.kind` | `actor.id` | `chat.id` |
|----------|-------------|------------|-----------|
| `GROUP_AT_MESSAGE_CREATE` / `GROUP_MESSAGE_CREATE` | `group` | `member_openid` | `group_openid` |
| `C2C_MESSAGE_CREATE` | `private` | `user_openid` | `user_openid` |
| `AT_MESSAGE_CREATE` / `MESSAGE_CREATE` | `channel` | 频道用户 ID | `channel_id` |
| `DIRECT_MESSAGE_CREATE` | `channel_private` | 频道用户 ID | `guild_id` |

官方 ID 和消息 ID 都按字符串保留。`NormalizedEvent::message_id_str()` 可以无损读取；`message_id()`、`user_id()`、`group_id_i64()` 只在原始字段确实是数字时有值。

归一化后的 `raw_json` 包含通用字段：

```json
{
  "post_type": "message",
  "message_type": "group",
  "message_id": "ROBOT1.0_...",
  "user_id": "member_openid",
  "group_id": "group_openid",
  "to_me": true,
  "qqbot_payload": {}
}
```

原始 Gateway `d` 对象完整放在 `qqbot_payload`。`event_type`、`event_id`、Gateway `sequence`、`msg_idx`、`message_scene` 等常用值还会复制到 `extensions`，插件不必反复解析原始结构。

### 群消息中的 @

全量群消息不能只靠事件名判断是否指向机器人。运行时接受适配器计算的 `to_me`：

1. `GROUP_AT_MESSAGE_CREATE` 和 `AT_MESSAGE_CREATE` 直接视为指向机器人。
2. `GROUP_MESSAGE_CREATE` 检查 `mentions[].is_you`。
3. 文本中的 `<qqbot-at-user id="..." />`、`<@id>` 和 `<@!id>` 转成有序 At 段。
4. 命令前的机器人提及和空白从纯文本命令中清理。

`NormalizedEvent::is_at_self()` 同时检查 `raw_json.to_me` 和消息段中的自身 ID，供插件统一使用。

### 官方回复执行

插件产生回复后，`QqOfficialRuntimeContext` 负责构建 action：

```text
chat.kind=group           -> send_group_msg
chat.kind=private         -> send_private_msg
chat.kind=channel         -> send_channel_msg
chat.kind=channel_private -> send_dms
```

有 `message_id` 时使用 `msg_id`；只有没有消息 ID 的事件才使用 `event_id`。群和 C2C 在实际发送前按 `bot + route + msg_id` 分配递增 `msg_seq`，同一状态一小时未更新后清理。这样一个命令产生多条回复时，不会重复使用 `msg_seq = 1`。

`msg_id`、`event_id` 和 `is_wakeup=true` 在编码层再次检查互斥关系。媒体回复和动态插件主动发送调用同一个执行函数，避免两种入口出现不同的平台行为：

- 群/C2C 公网 URL：调用 `/files`，再把响应中的 `file_info` 放入 `msg_type=7` 消息。
- 群/C2C Base64：解码和限长后完成 `upload_prepare`、预签名分片 PUT、`upload_part_finish` 与 `/files` 合并。
- 频道/DMS 公网图片：使用消息体的 `image` URL。
- 频道/DMS Base64 图片：发送 `multipart/form-data` 的 `file_image`。

Base64 解码前先按编码长度估算内存，解码后再次校验实际大小和文件头。图片、视频、语音和普通文件分别使用独立上限；错误消息只记录类型、大小和阶段，不记录 Base64 正文。分片 PUT 不经过 QQ 鉴权中间件，防止把机器人 token 发送到预签名对象存储地址。

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

| 排序层级 | 规则 |
| --- | --- |
| 管理员命令优先级 | `0-1000`，数值越大越先匹配；静态插件默认 `30`，动态插件默认 `20`，宿主管理命令固定为 `10` |
| 插件声明优先级 | 管理员数值相同时，静态插件的 `CommandPlugin::priority()` 数值越小越先匹配；动态描述符使用兼容值 `200` |
| 稳定排序 | 前两项仍相同时，按来源和插件 ID 的字典序排序 |

管理员优先级保存在 `plugin-state.toml` 的 `[priorities]` 表中。Web 面板保存后更新 Runtime 内存，并让已启用的 Bot 重连以重建 `CommandDispatcher`。

Runtime 不向注册表写入 `ping`、`echo`、`status` 等业务命令。默认启用的 `/plugins`、`/registry` 和 `/dynamic-errors` 会以 `builtin` 来源、优先级 `10` 写入同一个注册表，不会在注册表前抢占消息；三者均要求管理员或所有者权限。管理员可以在 Web“配置 → 命令入口”逐项关闭，关闭后对应名称与别名不再占用。插件优先级高于 `10` 时可以正常接管同名命令。宿主 `help` 仅在插件匹配失败后兜底，因此插件声明 `help` 或 `h` 时仍由插件处理。

Web 面板的插件页按管理员优先级显示排名，同名命令的确定性排序仍记录在 `CommandRegistry` 诊断中。排查冲突时可临时启用 debug 日志，并对照插件页的优先级与插件 ID。

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

### 官方 QQ Bot 非消息事件

官方 QQ Bot 的非消息 Dispatch 会先转换为 `EventKind::Notice`、`EventKind::Meta` 或 `EventKind::Internal`，再进入通用系统事件 dispatcher。`raw_json.notice_type` 使用稳定的小写名称，`raw_json.qqbot_payload` 保留官方原始 `d` 对象。

当前 Notice 覆盖：

| 类别 | Gateway 事件 |
|------|--------------|
| 频道和子频道 | `GUILD_CREATE/UPDATE/DELETE`、`CHANNEL_CREATE/UPDATE/DELETE` |
| 频道成员 | `GUILD_MEMBER_ADD/UPDATE/REMOVE` |
| 消息删除和回应 | `MESSAGE_DELETE`、`PUBLIC_MESSAGE_DELETE`、`DIRECT_MESSAGE_DELETE`、`MESSAGE_REACTION_ADD/REMOVE` |
| QQ 群机器人状态 | `GROUP_ADD_ROBOT`、`GROUP_DEL_ROBOT`、`GROUP_MSG_REJECT/RECEIVE` |
| C2C 和好友状态 | `FRIEND_ADD/DEL`、`GROUP_MEMBER_ADD/REMOVE`、`C2C_MSG_REJECT/RECEIVE` |
| 互动和审核 | `INTERACTION_CREATE`、`MESSAGE_AUDIT_PASS/REJECT`、`SUBSCRIBE_MESSAGE_STATUS` |
| 音频和直播 | `AUDIO_START/FINISH/ON_MIC/OFF_MIC`、音视频或直播成员进出 |
| 论坛 | 私域 Forum 和 Open Forum 的主题、帖子、回复创建/更新/删除事件 |

`READY` 和 `RESUMED` 映射为 Meta；未知的官方事件不会被伪装成已有 OneBot notice，而是保留为 `EventKind::Internal(原事件名)`。

`INTERACTION_CREATE` 的类型 11、12 在插件分发前自动调用官方 ACK 接口，避免客户端一直显示等待。ACK 失败只记录警告，原事件仍继续进入系统分发。

通用 dispatcher 可以把稳定 route 交给静态或动态系统插件。OneBot 专属的好友申请、群邀请审批 action 不会自动套用到官方 QQ Bot；对应信号只记录为当前协议没有自动动作映射。

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

基于 Bot、协议、会话和 `message_id` 的滑动窗口去重：

```
收到 msg_id=12345
    → 检查缓存 → 不存在 → 处理 + 加入缓存
收到 msg_id=12345 (重复)
    → 检查缓存 → 已存在 → 丢弃
```

官方 QQ 全量消息还可能提供 `msg_idx` 或 `msg_seq`。运行时优先把该投递索引加入去重键：

```text
bot + protocol + chat.kind + chat.id + message_id + msg_idx
```

因此同一 `message_id`、不同 `msg_idx` 的投递可以分别处理；完全相同的重投仍会被丢弃。没有投递索引时退回普通消息 ID 去重。

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

### 插件状态

插件的启用状态和管理员命令优先级保存在同一个文件中：

```toml
# config/plugin-state.toml (自动管理)
[modules]
"example-plugin" = true
"spam-plugin" = false

[priorities]
"example-plugin" = 80
"spam-plugin" = 15
```

启用状态、动态重载和命令优先级统一通过 Web 管理面板或带管理 Token 的 API 操作。宿主保存时会先生成 `.bak` 备份，再原子替换当前文件。

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

管理员在 Web 插件页点击“重新扫描”，或调用 `POST /api/v1/plugins/reload`：

```
1. 管理 API 完成 Token 鉴权
2. 卸载所有已加载的动态库 (dlclose)
3. 重新扫描 plugin_bin_dir 目录
4. 加载新发现的动态库 (dlopen)
5. 获取每个库的 PluginDescriptor
6. 重建命令注册表
7. 重建系统事件路由表
8. API 返回扫描结果，插件页刷新实际状态
```

重新扫描会先停止接受新的动态回调并等待在途调用结束，替换运行时后让已启用的 Bot 重连，以便每个会话重建命令和事件路由。短暂重连是预期行为；不要在插件仍执行长任务时反复点击。

## 会话管理

每个 Bot 实例维护一个**会话（Session）**，代表与 OneBot 实现的一次连接周期：

```
会话生命周期:
  连接建立
    → 事件循环
    → [断开] → 自动重连 → 新会话
    → [动态重载/命令配置更新] → 主动重连 → 新会话
    → [停止信号] → 优雅关闭
```

官方 QQ Bot 的 Gateway 会话还会保存 `session_id`、`seq`、`shard` 和 heartbeat 状态。收到 `Reconnect` 时优先 Resume，收到 `InvalidSession` 时清空会话并重新 Identify。

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
