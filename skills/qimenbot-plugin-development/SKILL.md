---
name: qimenbot-plugin-development
description: Develops, reviews, builds, publishes, deploys, and troubleshoots QimenBot static and dynamic Rust plugins, including commands, events, messages, interceptors, proactive sending, webhooks, ABI compatibility, the GitHub plugin marketplace, and standalone crates.io projects. Use when a task mentions QimenBot plugins, #[module], #[dynamic_plugin], cdylib, plugin hot reload, marketplace publishing, plugin configuration, or dynamic plugin loading errors.
---

# QimenBot 插件开发

## 权威入口

- 仓库：<https://github.com/lvyunqi/QimenBot>
- 用户文档：<https://lvyunqi.github.io/QimenBot/>
- 静态示例：<https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-plugin-example>
- 动态示例：<https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example>
- 独立动态模板：<https://github.com/lvyunqi/QimenBot/tree/main/templates/dynamic-plugin>
- Release：<https://github.com/lvyunqi/QimenBot/releases>
- 官方 QQ Bot Markdown 消息格式：<https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html>

仓库内开发以当前源码、示例和测试为第一事实来源；外部开发以已发布 crates.io API 和线上文档为准。不要凭记忆补不存在的宏、字段或兼容性。

本 Skill 服务于插件项目的开发、发布和排错，不是 QimenBot 框架或商城后端的架构规范。用户要求修改主框架时，以仓库代码和 `AGENTS.md` 为准，不要把插件投稿流程误当成框架实现方案。

## 先选择插件类型

| 条件 | 选择 |
|---|---|
| 能修改并重新编译 QimenBot，需要 async 或完整 `OneBotActionClient` | 静态插件 |
| 没有主框架源码、需要独立仓库发布、热重载或第三方分发 | 动态插件 |
| 需要 API 0.5+ Webhook、后台主动发送或 API 0.6 在线配置，但不想改主程序 | 动态插件 |
| 需要直接调用大量 OneBot Action 或深度参与宿主生命周期 | 静态插件 |

用户已指定类型时不要擅自改型。信息不足且两种实现差异会改变交付物时才询问；否则根据部署条件选择并说明依据。

## 必须执行的流程

1. 确认 QimenBot 版本、协议、操作系统、CPU、GNU/musl、部署方式和是否拥有主框架源码。
2. 静态插件完整阅读 [references/static-plugins.md](references/static-plugins.md)；动态插件完整阅读 [references/dynamic-plugins.md](references/dynamic-plugins.md)。
3. 涉及配置、加载、在线表单、官方 QQ Bot 或错误诊断时，再阅读 [references/runtime-and-troubleshooting.md](references/runtime-and-troubleshooting.md) 和 [references/online-configuration.md](references/online-configuration.md)；涉及商城收录或 GitHub Release 分发时，完整阅读 [references/marketplace-publishing.md](references/marketplace-publishing.md)。
4. 先检查目标项目现有 `Cargo.toml`、示例和配置，沿用当前 API，不做无关重构。
5. 为命令、事件、配置解析、生命周期或 target 兼容性添加与风险相称的测试或可重复验证。
6. 完成编译、产物检查、加载验证和部署说明；不能运行宿主时明确列出未验证项。

## 共同约束

- 插件 ID 一经发布应保持稳定；配置文件名和启停状态都依赖它。
- 跨协议 ID 一律优先按字符串处理。官方 QQ Bot 的 openid 不能强转为传统 QQ 数字。
- 普通回复优先使用 QimenBot 通用消息模型；`OneBotActionClient` 只适用于 OneBot Action，不等同于官方 QQ OpenAPI。
- 官方 QQ 本地媒体通过通用 Base64 消息段交给宿主处理；插件不读取 Bot 凭据、不自行实现分片上传，也不把 Base64 正文写入日志。
- 官方 QQ Markdown 使用平台扩展语法；标题、文字样式、链接、图片、列表、引用、分隔线和换行等以官方 Markdown 文档为准。原生 HTML 标签（例如 `<br>`、`<font>`）不要按浏览器兼容性推断，需在目标聊天场景实测。
- 不把 Secret、Token、用户配置、数据库或编译后的动态库提交到 QimenBot 主仓库。
- 主框架代码必须保持插件无关；具体插件逻辑只能进入插件目录或独立插件仓库。
- Runtime 不附带 `ping`、`echo`、`status` 等业务命令。宿主默认保留可逐项关闭的管理员命令 `plugins`、`registry`、`dynamic-errors`，固定优先级为 `10`；插件优先级更高时可接管同名命令。宿主 `help` 是可关闭、可被插件接管的分页兜底。
- 命令前缀、私聊裸命令、@ 和回复入口由 `[official_host.commands]` 控制。插件不要自行删除协议 @ 标签或硬编码 `/`。
- 动态插件回调是同步 FFI，不直接使用 `async fn`；跨 FFI 边界只用 Host API 提供的 ABI 稳定类型。
- API 0.6 配置必须使用独立 Schema 描述符；不要把字段追加到旧 `PluginDescriptor`，也不要让插件提供 HTML/JavaScript。
- 密钥只能使用 Schema 的 `writeOnly` / `x-qimen-secret` / `format = "password"` 标记；不要在普通配置值、日志、默认值或 README 中放凭据。
- `live` 配置必须有可逆的 `#[config_change]`；后台线程在替换前停止并 `join`，`reload` 插件的 `init` 必须支持重复调用。
- 动态插件二进制必须匹配宿主的操作系统、CPU 和 C 运行时；musl 发行包不支持动态加载。
- 商城驱动兼容必须按版本声明，分别填写 `onebot11` 与 `qq-official` 实际测试过的场景、事件和发送能力。
- 商城 Release 资产名必须包含完整 target；主目录固定仓库数字 ID、target、资产名、大小和 SHA256。
- 商城版本一经发布不可原地替换元数据或资产；修复时新增 SemVer，历史版本只允许改为 `yanked = true`。

## 完成交付前检查

- 已说明静态或动态插件的选择理由及所需源码边界。
- 已给出插件目录、依赖、配置、构建命令、产物路径和加载方式。
- 已区分框架版本、crates.io 包版本和动态 ABI API 版本。
- API 0.6 插件同时使用已发布的 `abi-stable-host-api 0.1.13` 与 `qimen-dynamic-plugin-derive 0.1.13`，没有混用 `0.1.12`、浮动 Git 分支或本地 path。
- 在线配置已覆盖 Schema、UI Schema、密钥保留、revision 冲突和 `live/reload/restart` 生效语义。
- 已覆盖权限、作用域、字符串 ID、错误处理和资源清理。
- 已确认命令在目标宿主入口配置下可触发，帮助描述、隐藏状态和分页展示符合预期。
- 动态后台线程能在 `#[shutdown]` 中停止并 `join`；Webhook 已考虑鉴权、签名、超时和重放。
- 商城投稿已核对公开仓库、许可证、驱动矩阵、固定资产名、target、glibc、大小、SHA256 和构建证明。
- 已运行最小相关检查，并报告实际结果。
