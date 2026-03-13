# 更新日志

## v0.1.0 (开发中)

### 核心功能

- **多协议框架** — 支持 OneBot 11 协议，OneBot 12 / Satori 预留扩展点
- **多传输模式** — 正向 WebSocket、反向 WebSocket、HTTP API、HTTP POST
- **声明式插件系统** — `#[module]` / `#[commands]` / `#[command]` / `#[notice]` / `#[request]` / `#[meta]` 宏
- **拦截器链** — `pre_handle` / `after_completion`，支持自定义预处理逻辑
- **命令系统** — 别名、示例、分类、权限等级、消息过滤器、**作用域声明**（`scope`）
- **系统事件路由** — 群通知、好友请求、Meta 事件全部通过注解分发

### 命令作用域（CommandScope）

- **声明式过滤** — 命令可通过 `scope = "group"` / `"private"` 声明仅在群聊或私聊中生效
- **分发层自动过滤** — 不匹配的环境下命令静默跳过，无需在回调内手动判断
- **静态 + 动态插件均支持** — 静态插件用 `#[command(scope = "group")]`，动态插件用 `scope = "group"` 属性

### 运行时保护

- **令牌桶限流** — 每 Bot 独立的消息频率限制
- **消息去重** — 基于 message_id 的滑动窗口去重
- **群事件过滤** — 白名单/黑名单机制
- **插件 ACL** — 运行时启用/禁用插件

### 动态插件 (v0.3 FFI)

- **`#[dynamic_plugin]` 过程宏** — 声明式定义动态插件，自动生成 FFI 导出代码
- **ABI 稳定** — 基于 `abi_stable` crate 的跨库安全传递
- **多命令/多路由** — 单个动态库可注册多个命令和事件路由
- **ReplyBuilder** — 流式构建富媒体回复（`CommandResponse::builder().text(...).at(...).build()`）
- **生命周期钩子** — `#[init]` / `#[shutdown]` 支持插件初始化（含配置加载）和资源清理
- **CommandRequest v0.3** — 新增 `sender_nickname`、`message_id`、`timestamp` 字段
- **作用域支持** — `CommandDescriptorEntry` 新增 `scope` 字段
- **热重载** — `/plugins reload` 无需重启
- **熔断器保护** — 连续 3 次失败自动隔离 60 秒
- **拦截器支持** — `#[pre_handle]` / `#[after_completion]` 宏属性，动态插件可注册消息拦截器，运行时自动包装为 `MessageEventInterceptor` 注入拦截器链
- **向后兼容** — v0.1 / v0.2 符号名和字段仍然支持

### 请求自动化

- **好友请求** — 白名单/黑名单/关键词过滤自动审批
- **群邀请** — 白名单/黑名单/关键词过滤自动审批

### OneBot 11 API

- 40+ API 操作封装：消息、群管理、文件、频道、表情回应等
- 完整的消息模型：文本、图片、@、表情、分享、按钮等
