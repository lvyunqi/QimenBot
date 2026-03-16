# 更新日志

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
