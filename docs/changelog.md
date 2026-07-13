# 更新日志

## v0.1.12（待发布）

### 主动发送稳定账号选择

- `[[bots]]` 新增可选 `account_id`，OneBot 可填写固定的 Bot QQ / `self_id`，与可调整的部署实例 `id` 分离。
- 新增 `BotApi::for_account(...)`、`SendBuilder::bot_account(...)` 和 `ProactiveSendRequest::for_account(...)`；原有按 `bot_id` 发送保持兼容。
- 稳定账号选择器编码在既有 `ProactiveSendRequest.bot_id` 字符串内，不改变 API 0.4/0.5 的 FFI 结构布局。
- Runtime 为启用 Bot 建立账号索引，入队后规范化为实际实例 `id`；补充未找到、禁用、重复账号和实例改名测试。
- 配置校验拒绝空账号、多个启用 Bot 的重复账号及占用宿主保留选择器前缀的实例 ID。
- 更新动态插件示例、后台推送配置、Webhook、FFI、配置和 API 0.4 文档。

---

## v0.1.11 (2026-07-13)

### 动态插件 Webhook Gateway

- 新增动态插件 API 0.5 和 `#[webhook(method = "...", path = "...")]`，由 Runtime 统一提供 HTTP 监听和精确路由。
- 新增插件 URL 命名空间、可选 Bearer token、请求体大小、并发数和同步回调超时限制。
- 新增 ABI 稳定的 `WebhookRequest`、`WebhookResponse` 和独立 Webhook 描述符导出，不修改 API 0.1 至 0.4 使用的旧 `PluginDescriptor` 布局。
- Webhook 回调在 blocking 线程执行，响应数据在离开 FFI 前复制为宿主持有内存；动态库生命周期锁防止超时回调或热重载造成提前卸载。
- 热重载现在会重新绑定 Host API、读取插件配置并执行 `init`，只恢复初始化成功插件的命令、事件和 Webhook 路由。
- 更新动态插件示例、默认配置、架构、FFI 和部署安全文档。
- `abi-stable-host-api 0.1.11` 与 `qimen-dynamic-plugin-derive 0.1.11` 已发布到 crates.io，仓库外插件可以直接使用 API 0.5。

---

## v0.1.10 (2026-07-13)

### 动态插件实时主动推送

- 新增动态插件 API 0.4 和 Host API v1，使后台线程无需命令、事件或 Heartbeat 驱动即可实时提交主动发送。
- 新增按 Bot 隔离的有界队列、离线 TTL 和在线执行器，覆盖 OneBot 11 与 QQ 官方的私聊、群聊、频道和频道私信目标。
- 新增 `BotApi::for_bot`、`SendBuilder::bot`、`try_send` 及稳定状态码，同时保留 API 0.1 至 0.3 的回调后 flush 路径。
- 加固插件 shutdown、后台线程 join、Host API unbind 与动态库卸载顺序，避免热重载期间出现悬空回调。
- 更新动态插件示例、项目模板、配置参考和独立 crates.io 插件开发文档。
- `abi-stable-host-api 0.1.10` 与 `qimen-dynamic-plugin-derive 0.1.10` 已发布到 crates.io，仓库外插件无需依赖本地主框架源码。

### 内部链路诊断

- 新增 `qimenctl simulate-onebot11`，可模拟标准 OneBot 11 反向 WebSocket 客户端发送私聊、群聊或原始 JSON 事件。
- 模拟器自动完成 Token 鉴权、`lifecycle.connect` 上报、Action 展示和同 echo 成功回包，用于隔离客户端连接、命令注册、动态插件回调及发送链路问题。
- 增加真实 WebSocket 握手、事件、Action 和 echo 往返测试，并支持脱离 `config/base.toml` 的显式端点模式。

### 修复

- 修复 official host 预扫描 API 0.4 动态插件时丢弃多命令和路由描述符的问题；`commands`、`aliases` 和事件 routes 现在会正确注册到 Runtime。

---

## v0.1.9 (2026-07-12)

### 修复

- 动态命令及事件回调按宏实际导出的引用参数 ABI 调用，避免请求字段错位和未定义行为。
- 插件发送队列在离开动态库调用前复制为宿主持有的 ABI 字符串，避免异步发送或热重载后的跨库析构风险。
- 增加发送动作字段完整性回归测试，并用独立动态库验证显式卸载后仍可安全读取和释放发送结果。

---

## v0.1.8 (2026-07-12)

### 修复

- 动态插件加载后保持驻留，不再因 300 秒空闲而被运行时隐式卸载。
- 避免带后台线程的插件在动态库代码被卸载后继续运行并触发进程段错误。
- 动态库仍可通过显式热重载流程执行 `shutdown` 后安全卸载。

---

## v0.1.7 (2026-07-12)

### 修复

- 动态命令匹配日志现在明确记录 Bot、插件和命令，便于定位插件分发问题。
- 未注册命令不再被错误记录为已命中的内置命令。

---

## v0.1.6 (2026-07-12)

### 修复

- 修正反向 WebSocket 握手使用的 RFC 6455 GUID，确保 `Sec-WebSocket-Accept` 能被标准客户端校验通过。
- 增加 RFC 6455 官方测试向量和真实 TCP 握手响应回归测试。

---

## v0.1.5 (2026-07-12)

### 反向 WebSocket 运行时完善

- 接通 OneBot 11 反向 WebSocket 的监听、路径校验、Token 鉴权、事件分发和 Action `echo` 响应链路。
- 反向连接断开后继续监听并等待重连，空闲监听期间 daemon 不再提前退出。
- 多 Bot 长连接改为并发运行，单个长连接不再阻塞后续 Bot 实例启动。
- 框架兼容性报告使用实际编译版本，不再输出硬编码旧版本。

### 动态插件文档

- 补充完全脱离主仓库、通过 crates.io `0.1.1` 依赖开发动态插件的流程，并验证独立 `cdylib` release 构建。
- 明确 crate 发布版本与动态插件 ABI API `0.3` 的区别，以及跨机器部署的目标平台要求。

---

## v0.1.4 (2026-05-04)

### 修复

- 修复官方 QQ Bot 自定义 Keyboard 的按钮结构，将 `KeyboardBuilder` 输出转换为官方 inline keyboard 需要的 `id`、`render_data`、`action` 嵌套 payload。
- 保持模板 Keyboard ID 透传，继续支持官方后台创建的键盘模板。

---

## v0.1.3 (2026-05-03)

### 官方 QQ Bot 适配预览

- 新增 `qq-official` 协议和 `gateway` 传输模式，支持官方 QQ Bot Gateway 接入
- 新增 `qimen-adapter-qqbot`，将 QQ 群 @、QQ 单聊 C2C、频道 @、频道私信消息归一化为 `NormalizedEvent`
- 新增 `qimen-transport-qqbot`，封装 AppID + Secret access token、Gateway 会话、Heartbeat、Resume 和 OpenAPI 发送
- Runtime 消息流水线抽象为协议无关处理，官方 Bot 复用命令、权限、限流、去重、拦截器和插件执行
- 支持官方 Bot 文本、图片、Markdown、Keyboard、语音/视频 media 上传、频道撤回和发送失败降级
- 官方 OpenAPI 429 会按 bot + route 做短期 backoff，发送失败不再打断 Gateway 会话
- 补充官方 QQ Bot 接入教程、配置说明和传输说明

---

## v0.1.2 (2026-03-16)

### 动态插件执行隔离 + 超时保护

- **Per-library 独立锁** — 将 `DynamicPluginRuntime` 从单一全局 Mutex 重构为 per-library `Arc<Mutex>>`，一个插件挂起/死锁不再阻塞其他插件的命令、事件和拦截器
- **spawn_blocking** — 所有动态插件 FFI 调用统一通过 `tokio::task::spawn_blocking` 执行，不再阻塞 Tokio async 运行时
- **超时保护** — 新增 `dynamic_plugin_timeout_secs` 配置项（默认 30 秒），FFI 调用超时后自动触发熔断器
- **初始化超时** — 插件 `#[init]` 生命周期钩子使用 2× 超时（默认 60 秒），允许较慢的初始化过程
- **熔断器增强** — 超时也计入失败次数，3 次失败后自动隔离 60 秒

### 配置项

```toml
[official_host]
dynamic_plugin_timeout_secs = 30  # 默认值，单位秒
```

### crates.io 发布

- `abi-stable-host-api` 和 `qimen-dynamic-plugin-derive` 发布到 crates.io
- 动态插件模板依赖改为 crates.io 版本号引用

---

## v0.1.1 (2026-03-15)

### 首次启动体验

- **自动复制配置模板** — 首次启动时自动从 `templates/` 复制 `base.toml` 和 `plugin-state.toml` 到 `config/`，无需手动创建
- **Windows CMD ANSI 修复** — 在 Windows CMD 环境下自动启用虚拟终端序列，日志着色正常显示

### 修复

- 修复 CI clippy 警告（`collapsible_if`、`unnecessary_map_or` 等）
- 修复 `ReplyBuilder` doc-test 缺少 `use` 导入

---

## v0.1.0 (2026-03-11 ~ 2026-03-15)

首个公开版本，包含完整的多协议 Bot 框架和双插件系统。

### 核心框架

- **多协议架构** — 支持 OneBot 11 协议，OneBot 12 / Satori 预留扩展点
- **多传输模式** — 正向 WebSocket、反向 WebSocket、HTTP API、HTTP POST
- **多 Bot 实例** — 单进程运行多个 Bot，独立配置、独立限流
- **分层 Crate 设计** — 33 个 workspace 成员，职责清晰解耦

### 静态插件系统

- **声明式宏** — `#[module]` / `#[command]` / `#[notice]` / `#[request]` / `#[meta]` 注解式开发
- **inventory 自动注册** — 基于 `inventory` crate 自动收集插件，消除手动 match 分支
- **拦截器链** — `MessageEventInterceptor` trait，支持 `pre_handle` / `after_completion`
- **完整 async 支持** — 插件回调完全异步，可调用 `OneBotActionClient` 40+ API

### 动态插件系统 (FFI v0.3)

- **`#[dynamic_plugin]` 过程宏** — 声明式定义动态插件，自动生成 FFI 导出代码
- **ABI 稳定** — 基于 `abi_stable` crate 的跨库安全传递
- **多命令/多路由** — 单个动态库可注册多个命令和多个事件路由
- **CommandResponse / ReplyBuilder** — 流式构建富媒体回复（`.text()` `.at()` `.face()` `.image()`）
- **BotApi / SendBuilder** — 队列模式主动发送消息到任意目标（群聊/私聊）
- **生命周期钩子** — `#[init]`（含 TOML→JSON 配置桥接）/ `#[shutdown]` 资源清理
- **拦截器支持** — `#[pre_handle]` / `#[after_completion]` 宏，动态插件注册消息拦截器
- **CommandRequest v0.3** — 包含 `sender_nickname`、`message_id`、`timestamp` 字段
- **热重载** — `/plugins reload` 运行时重新扫描插件目录，无需重启
- **熔断器保护** — 连续 3 次失败自动隔离 60 秒
- **向后兼容** — v0.1 / v0.2 符号名和字段仍然支持

### 命令系统

- **命令注册表** — 支持别名、分类、权限等级（owner/admin/user）、消息过滤器
- **作用域声明（CommandScope）** — `scope = "group"` / `"private"` 声明命令仅在特定环境生效，分发层自动过滤
- **中文命令前缀匹配** — 支持 `创建角色小明-男` 自动解析为命令 `创建角色` + 参数 `小明-男`，最长匹配优先

### 运行时保护

- **令牌桶限流** — 每 Bot 独立的消息频率限制
- **消息去重** — 基于 message_id 的滑动窗口去重
- **群事件过滤** — 白名单/黑名单机制
- **插件 ACL** — 运行时启用/禁用插件

### 请求自动化

- **好友请求** — 白名单/黑名单/关键词过滤自动审批
- **群邀请** — 用户白名单/群白名单/关键词过滤自动审批

### OneBot 11 API

- 40+ API 操作封装：消息、群管理、文件、频道、表情回应等
- 完整的消息模型：文本、图片、@、表情、分享、按钮等

### 工程化

- **CI/CD** — GitHub Actions 自动构建 + VitePress 文档部署
- **配置系统** — `config/base.toml` 支持环境变量替换（`${VAR}`）、per-bot 覆盖
- **VitePress 文档站** — 包含指南、插件开发教程、API 参考、进阶主题
