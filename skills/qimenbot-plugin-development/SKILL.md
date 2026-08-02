---
name: qimenbot-plugin-development
description: Develops, reviews, builds, deploys, and troubleshoots QimenBot static and dynamic Rust plugins, including commands, events, messages, interceptors, proactive sending, webhooks, ABI compatibility, and standalone crates.io projects. Use when a task mentions QimenBot plugins, #[module], #[dynamic_plugin], cdylib, plugin hot reload, plugin configuration, or dynamic plugin loading errors.
---

# QimenBot 插件开发

## 权威入口

- 仓库：<https://github.com/lvyunqi/QimenBot>
- 用户文档：<https://lvyunqi.github.io/QimenBot/>
- 静态示例：<https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-plugin-example>
- 动态示例：<https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example>
- 独立动态模板：<https://github.com/lvyunqi/QimenBot/tree/main/templates/dynamic-plugin>
- Release：<https://github.com/lvyunqi/QimenBot/releases>

仓库内开发以当前源码、示例和测试为第一事实来源；外部开发以已发布 crates.io API 和线上文档为准。不要凭记忆补不存在的宏、字段或兼容性。

## 先选择插件类型

| 条件 | 选择 |
|---|---|
| 能修改并重新编译 QimenBot，需要 async 或完整 `OneBotActionClient` | 静态插件 |
| 没有主框架源码、需要独立仓库发布、热重载或第三方分发 | 动态插件 |
| 需要 API 0.5 Webhook、后台主动发送，但不想改主程序 | 动态插件 |
| 需要直接调用大量 OneBot Action 或深度参与宿主生命周期 | 静态插件 |

用户已指定类型时不要擅自改型。信息不足且两种实现差异会改变交付物时才询问；否则根据部署条件选择并说明依据。

## 必须执行的流程

1. 确认 QimenBot 版本、协议、操作系统、CPU、GNU/musl、部署方式和是否拥有主框架源码。
2. 静态插件完整阅读 [references/static-plugins.md](references/static-plugins.md)；动态插件完整阅读 [references/dynamic-plugins.md](references/dynamic-plugins.md)。
3. 涉及配置、加载、发布、官方 QQ Bot 或错误诊断时，再阅读 [references/runtime-and-troubleshooting.md](references/runtime-and-troubleshooting.md)。
4. 先检查目标项目现有 `Cargo.toml`、示例和配置，沿用当前 API，不做无关重构。
5. 为命令、事件、配置解析、生命周期或 target 兼容性添加与风险相称的测试或可重复验证。
6. 完成编译、产物检查、加载验证和部署说明；不能运行宿主时明确列出未验证项。

## 共同约束

- 插件 ID 一经发布应保持稳定；配置文件名和启停状态都依赖它。
- 跨协议 ID 一律优先按字符串处理。官方 QQ Bot 的 openid 不能强转为传统 QQ 数字。
- 普通回复优先使用 QimenBot 通用消息模型；`OneBotActionClient` 只适用于 OneBot Action，不等同于官方 QQ OpenAPI。
- 不把 Secret、Token、用户配置、数据库或编译后的动态库提交到 QimenBot 主仓库。
- 主框架代码必须保持插件无关；具体插件逻辑只能进入插件目录或独立插件仓库。
- 动态插件回调是同步 FFI，不直接使用 `async fn`；跨 FFI 边界只用 Host API 提供的 ABI 稳定类型。
- 动态插件二进制必须匹配宿主的操作系统、CPU 和 C 运行时；musl 发行包不支持动态加载。

## 完成交付前检查

- 已说明静态或动态插件的选择理由及所需源码边界。
- 已给出插件目录、依赖、配置、构建命令、产物路径和加载方式。
- 已区分框架版本、crates.io 包版本和动态 ABI API 版本。
- 已覆盖权限、作用域、字符串 ID、错误处理和资源清理。
- 动态后台线程能在 `#[shutdown]` 中停止并 `join`；Webhook 已考虑鉴权、签名、超时和重放。
- 已运行最小相关检查，并报告实际结果。
