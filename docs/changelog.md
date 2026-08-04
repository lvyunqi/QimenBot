# 更新日志

## v0.1.18 (2026-08-04)

### 动态插件在线配置

- 新增动态插件 API 0.6。插件通过独立配置描述符声明 JSON Schema Draft 2020-12、可选 UI Schema、配置结构版本和 `live` / `reload` / `restart` 生效方式，不修改旧 `PluginDescriptor` 布局，API 0.1 至 0.5 动态库继续兼容。
- Web 管理面板的插件卡片显示配置能力和生效方式；可配置插件使用桌面右侧抽屉和手机全屏表单，支持对象、嵌套对象、数组、对象数组、枚举、多选、数值范围、条件 Schema、组合 Schema、本地 `$ref`、动态键、只读字段和默认值。
- 增加 `#[validate_config]` 业务校验和 `#[config_change]` 即时应用回调。`reload` 模式在新配置初始化失败时恢复旧文件并重新加载，`live` 模式在回调失败时恢复文件和插件内存状态，`restart` 模式统一标记宿主待重启。
- 插件密钥使用独立提交通道，GET 接口只返回是否已配置；对象数组删除或排序时通过同 Schema 字段的一对一引用保留密钥归属，密钥原文不会进入浏览器普通配置值。
- 插件配置保存增加 SHA-256 revision 冲突检查、每插件最多 20 份备份和管理审计；新增 `official_host.plugin_config_dir`，默认继续使用 `config/plugins`。
- 插件商城元数据允许声明 `dynamic_api = "0.6"`，宿主兼容矩阵同步识别 API 0.6，同时继续拒绝把新插件安装到只支持旧 ABI 的宿主。
- `abi-stable-host-api 0.1.13` 与 `qimen-dynamic-plugin-derive 0.1.13` 已发布到 crates.io；仓库外插件可直接使用 API 0.6，不再依赖临时 Git revision。

### 官方 QQ Bot 本地富媒体

- 宿主开始解释现有 `base64://` 图片段。QQ 群和 C2C 完整执行 `upload_prepare`、预签名分片 PUT、`upload_part_finish`、`/files(upload_id)` 和 `media.file_info` 发送，不再要求插件先搭建图床。
- 频道和 DMS 本地图片使用 `multipart/form-data` 的 `file_image`；被动回复和 API 0.4+ 主动发送共用同一媒体执行路径。
- 增加文件头、Base64、空数据和内存上限校验；图片、视频、语音和文件的内联上限分别为 20 MB、30 MB、20 MB、32 MB。预签名 PUT 不携带 QQ 鉴权头，日志不记录 Base64 正文。
- API 0.6 在不修改 FFI 结构的前提下补充语音、视频和文件的 URL/Base64 builder；已有 API 0.1 至 0.5 动态库继续兼容，使用 `image_base64()` 的旧插件无需升级 ABI。

### 示例、模板与文档

- 动态插件示例升级为 API 0.6，提供覆盖主要表单控件的 Schema、UI Schema、数组密钥、插件语义校验和后台线程即时重配实现。
- 独立动态插件模板增加在线配置文件和 `reload` 生效示例，并切换到已发布的两个 crates.io `0.1.13` 依赖，不要求开发者持有主框架源码。
- 新增动态插件在线配置完整教程，补齐 FFI、配置目录、Web 面板、商城兼容和插件开发 Skill。
- 移除运行时内置的 `ping` / `p` 兜底命令；`ping` 是否可用完全取决于当前启用的静态或动态插件。

---

## v0.1.17 (2026-08-03)

### 插件商城

- Web 管理面板新增插件商城，集中展示插件来源、许可证、信任级别、构建证明、版本记录，以及 OneBot 11 / 官方 QQ Bot 的场景、事件和发送能力。
- 动态插件支持在线安装、更新、关联本地文件、固定版本、回滚和卸载；静态插件只展示源码与兼容范围，仍需加入框架源码后重新构建。
- 版本选择同时检查 QimenBot SemVer、动态 ABI API、正式版或预发布通道、操作系统、CPU、完整 Rust target 和最低 glibc，避免安装与当前宿主不匹配的二进制。
- 安装前核对 GitHub 数字仓库 ID、Release tag、资产名称、文件大小和 SHA256；替换动态库时先等待现有回调结束，加载失败会恢复并重新加载上一版本。
- 商城缓存保存目录、下载资产和可回滚的历史版本，安装锁记录当前版本、target、SHA256、固定状态与回滚信息；手工复制的动态库只有通过完整校验后才能交由商城管理。

### 商城投稿与发布

- 主仓库新增 `marketplace/` 目录、Schema、投稿模板和 PR 规则，分别说明开源仓库要求、驱动兼容声明、构建产物命名、版本不可变规则及撤回流程。
- 提供六个 Rust target 的 GitHub Actions 发布示例，统一生成动态库、SHA256 和 Artifact Attestation；插件源码与 Release 资产始终保留在作者自己的公开仓库。
- 商城校验流水线检查永久插件 ID、历史版本不可变、GitHub 仓库身份、许可证、Release 元数据和可选构建证明，不会下载、编译或执行投稿插件。
- 商城 PR 合并到 `main` 后自动重新校验并生成 Pages 索引，部署成功后管理面板即可读取新版本，无需发布新的 QimenBot 安装包。

### 运行时与诊断

- `debug` 日志新增原始消息收发记录，覆盖 OneBot 11 WebSocket、HTTP API、HTTP POST，以及官方 QQ Bot Gateway 和 OpenAPI；只记录消息类事件与发送请求，便于插件开发时核对协议载荷。
- 动态插件热重载增加串行事务和加载暂停机制，文件替换前确认旧动态库不再被回调持有，失败时依次执行卸载、文件回滚和上一版本重载。
- 管理面板商城操作、版本固定、回滚、关联和卸载统一写入审计日志，目录刷新失败时可继续查看最近一次成功保存的缓存。

### 插件开发 Skill

- 插件开发 Skill 合并为一份工具无关的入口，静态插件、动态插件和运行排错分别维护，删除按不同 AI 工具复制的旧文档。
- 动态插件章节补齐脱离主框架源码的独立项目结构、crates.io 依赖、API 0.5、主动发送、Webhook、target 选择和加载诊断。
- 新增商城发布参考，覆盖仓库准备、兼容能力声明、多平台 Release、SHA256、构建证明和商城 PR 的完整流程。

---

## v0.1.16 (2026-08-02)

### 安装包与兼容性

- 发行包统一使用根目录的 `qimenbot` 作为唯一启动入口，内部核心移动到 `runtime/qimenbotd`。
- Release 不再单独发布启动器文件；普通用户只下载 `QimenBot-*` 完整包，已有安装继续通过 `qimenbotd-*` 资产自动更新核心。
- 完整安装包新增 SHA256，面板和部署文档统一使用 `qimenbot` 这一入口名称。
- GNU/Linux Release 固定使用 Rust 1.89.0 与 Debian Bullseye（glibc 2.31）构建，并在流水线拒绝更高的 GLIBC 运行时要求。
- Release 的原生构建、交叉构建和 Docker 构建统一固定使用 Rust 1.89.0，避免仓库工具链覆盖导致目标标准库缺失。
- ARM64 GNU/Linux 交叉构建补齐 glibc 开发头文件，修复 `ring` 编译时找不到 `bits/libc-header-start.h` 的问题。
- 动态插件加载失败时识别静态 musl 限制，并提示改用同 CPU 架构的 GNU 包或 Docker；文档补齐所有发行平台与插件 target 对照。
- 文本日志只在交互式终端输出 ANSI 颜色，并支持使用 `NO_COLOR=1` 强制关闭，避免服务日志出现控制字符。

---

## v0.1.15 (2026-08-02)

### 部署与更新

- 新增 `qimen-launcher` 二进制监督器，按系统架构检查 GitHub Release，校验 SHA256 后替换 `qimenbotd`，并通过健康检查决定确认更新或自动回滚。
- 管理面板新增版本更新页，区分 launcher 托管、Docker 托管和直接启动三种部署状态，支持检查、确认安装与优雅重启。
- 运行时统一处理面板关闭请求、`Ctrl+C` 和 Unix `SIGTERM`，新增无需管理 Token 的只读 `/healthz` 健康端点。
- 新增 Docker 多阶段构建、Compose 持久化部署和 Docker Hub 多架构发布工作流；Release 同时提供 launcher、原始更新资产和 SHA256 文件。
- 重写部署文档，补充 Docker Hub Token、GitHub Actions、systemd、Windows Service、镜像更新、二进制回滚和数据保留说明。

---

## v0.1.14 (2026-08-01)

### Web 管理面板

- 新增内嵌 Web 管理面板，默认监听 `127.0.0.1:3210`，发行版无需单独部署前端文件。
- 增加运行总览、Bot 启停与重连、结构化实时日志、动态插件启停与热重载、安全审计等管理页面。
- 配置表单按运行时、面板安全、模块与插件、Webhook、配置版本分区；状态、密钥和模块 ID 使用徽标展示，并适配桌面与手机视口。
- 配置保存前执行 TOML 解析和完整校验，写入前创建版本备份，支持历史版本回滚和并发修订检测。
- 管理 Token 和 Webhook Token 只写不读；非回环监听强制要求管理 Token，接口响应包含 CSP 等安全响应头。

### 构建与文档

- CI 和发行工作流在 Rust 构建前编译管理面板，并把静态资源嵌入 `qimenbotd`。
- 新增 Web 管理面板新手指南，补充配置项、远程访问、徽标含义、保存回滚和安全注意事项。

---

## v0.1.13 (2026-08-01)

### 官方 QQ Bot 消息接入

- 增加 `GROUP_MESSAGE_CREATE` 全量群消息支持，并保留 `GROUP_AT_MESSAGE_CREATE`、`C2C_MESSAGE_CREATE`、频道消息和频道私信的统一处理。
- 兼容当前 `<qqbot-at-user id="..." />`、旧版 `<@id>` / `<@!id>` 及 `mentions[].is_you`，按原顺序恢复 At/Text 段，修复全量群消息中 @ 机器人后命令不回复的问题。
- 官方用户、群、频道和消息 ID 全程按字符串保存；动态插件命令和拦截器不再丢失官方字符串消息 ID。
- 解析附件、`msg_elements`、`message_scene`、`msg_idx`、成员角色和 RFC 3339 时间；原始 Gateway `d` 对象保存在 `raw_json.qqbot_payload`。
- 扩充频道、群/C2C 管理、消息删除、互动、审核、论坛、音频和直播子频道成员事件映射；`READY`、`RESUMED` 作为 Meta 事件保留。

### 回复和 OpenAPI

- 群、C2C、频道和 DMS 回复进入同一命令、权限、限流、拦截器及静态/动态插件流水线，并按会话类型选择对应 OpenAPI endpoint。
- 群和 C2C 回复优先使用原始 `msg_id`，自动为同一来信分配递增 `msg_seq`；没有消息 ID 的事件使用 `event_id`，并校验 `msg_id`、`event_id`、`is_wakeup=true` 互斥。
- 支持群/C2C 文本、Markdown、Keyboard、媒体、群 Card 和 C2C Input notify；支持频道/DMS 文本、Markdown、Keyboard、Ark、Embed 和图片。
- 群/C2C 图片、语音、视频和文件先调用 `/files`，再使用返回的 `file_info` 发送 `msg_type = 7`；补齐群、C2C、频道和 DMS 消息撤回。
- `INTERACTION_CREATE` 中需要确认的互动由运行时自动调用 ACK 接口，避免客户端持续等待。
- OpenAPI 错误保留 HTTP 状态、官方错误码、`retry_after` 和 `trace_id`，并区分鉴权、权限、频控、请求、资源和服务端错误；429 按 Bot 与路由退避。

### Gateway 会话

- access token 按 `expires_in` 缓存并提前刷新，Gateway 地址通过 `/gateway/bot` 获取。
- 完整处理 Hello、Identify、READY、Heartbeat、Heartbeat ACK、Reconnect、Invalid Session 和 Resume。
- Dispatch 成功处理后再提交序号；断线后携带 `session_id` 和最后序号恢复，无效会话自动清空状态并重新 Identify。
- 校验 Gateway 建议分片数，当前单连接模式在平台建议多分片时给出明确警告。

### 运行时与文档

- QQ 全量消息去重键加入 `msg_idx` / `msg_seq`，允许同一消息 ID 的不同投递片段分别处理。
- 增加 Gateway 原始 payload 到命令回复的端到端回归测试，覆盖群 @、全量群消息、C2C、频道、DMS、富消息、媒体、撤回、互动 ACK、错误分类和会话恢复。
- 重写官方 QQ Bot 新手接入和插件适配文档，补齐平台权限、配置、测试顺序、字符串 ID、回复配额、媒体流程和排错方法；同步传输层、运行时、配置及 README。
- 动态插件 API 仍为 0.5，ABI 和 Host API 布局未改变；现有仓库外动态插件无需因本次 QQ 官方协议更新重新编译或升级 crates.io 依赖。

---

## v0.1.12 (2026-07-14)

### 主动发送稳定账号选择

- `[[bots]]` 新增可选 `account_id`，OneBot 可填写固定的 Bot QQ / `self_id`，与可调整的部署实例 `id` 分离。
- 新增 `BotApi::for_account(...)`、`SendBuilder::bot_account(...)` 和 `ProactiveSendRequest::for_account(...)`；原有按 `bot_id` 发送保持兼容。
- 稳定账号选择器编码在既有 `ProactiveSendRequest.bot_id` 字符串内，不改变 API 0.4/0.5 的 FFI 结构布局。
- Runtime 为启用 Bot 建立账号索引，入队后规范化为实际实例 `id`；补充未找到、禁用、重复账号和实例改名测试。
- 配置校验拒绝空账号、多个启用 Bot 的重复账号及占用宿主保留选择器前缀的实例 ID。
- 更新动态插件示例、后台推送配置、Webhook、FFI、配置和 API 0.4 文档。
- 动态插件示例默认声明 API 0.5；API 0.4 继续作为主动发送兼容版本保留。

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
