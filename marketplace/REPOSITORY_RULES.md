# 插件开源仓库规范

商城目录只保存索引数据。插件作者自己的 GitHub 仓库必须让用户能够看懂、构建、验证和追踪每一个 Release 资产。

## 基本要求

- 仓库必须公开，不能依赖申请权限后才能查看的子模块或下载地址。
- 根目录必须有明确的开源许可证文件，并与 `plugin.toml` 中的 SPDX 标识一致。
- 源码必须覆盖 Release 中的实际功能，不能只公开接口壳或过期版本。
- Git tag、Cargo 包版本和动态插件描述符版本应保持一致。
- Issue 或其他公开反馈入口应可用，README 中应说明安全问题的报告方式。

`repository_id` 使用 GitHub 仓库的数字 ID。访问：

```text
https://api.github.com/repos/OWNER/REPOSITORY
```

读取响应顶层的 `id`，不要填写账号 ID、Release ID 或仓库名称。仓库改名或转移后数字 ID保持不变，可以防止同名仓库冒充更新来源。

## README 至少说明

1. 插件用途、命令、事件和主要使用场景。
2. 支持的 QimenBot、动态 ABI API、OneBot 11 和官方 QQ Bot 场景。
3. 官方 QQ Bot 所需 Intents、接口权限和消息发送限制。
4. 配置文件路径、完整字段、默认值和最小示例。
5. 文件、数据库、缓存、网络请求、后台线程和 Webhook 行为。
6. 构建命令、支持 target、最低 glibc 和部署方式。
7. 升级迁移、数据备份、回滚限制和卸载后残留数据。
8. 许可证、维护状态、问题反馈和安全报告入口。

不要在示例中放真实 AppID、Secret、Token、Cookie、openid、群号或用户数据。占位值应一眼能看出需要替换。

## 动态插件仓库

动态插件是独立 Rust `cdylib`，应能在没有 QimenBot 主仓库源码的环境中构建：

```toml
[package]
name = "qimen-dynamic-plugin-group-tools"
edition = "2024"
version = "1.0.0"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "<已发布版本>"
qimen-dynamic-plugin-derive = "<与 Host API 配套的已发布版本>"
```

要求：

- 不使用指向作者电脑或 QimenBot 源码树的 `path` 依赖；
- crates.io 依赖版本明确，Git 依赖固定到公开 commit；
- 提交 `Cargo.lock`，Release 使用 `cargo build --locked`；
- `#[dynamic_plugin]` 的 ID 和版本与目录登记一致；
- 后台线程能在 `#[shutdown]` 中停止并 `join`；
- Webhook 有鉴权、签名、时间戳、超时和重放防护；
- 不把宿主回调跨线程长期保存，不在 FFI 边界传递非 ABI 稳定类型。

推荐从 QimenBot 的 `templates/dynamic-plugin` 建立独立仓库。模板是起点，最终仍要按插件实际能力删改配置、权限和兼容声明。

## 静态插件仓库

静态插件可以登记源码，但商城不能在线安装。仓库必须额外说明：

- 需要加入 QimenBot 的哪些依赖或模块注册位置；
- 支持的 QimenBot 源码版本或 commit；
- 构建 `qimenbotd` 的完整命令；
- 是否修改框架代码，以及如何保持改动插件无关；
- 升级 QimenBot 时需要重新合并或重新编译的步骤。

静态版本文件不能填写 `dynamic_api` 和 `assets`，但仍要填写版本级 `drivers`，让用户知道源码插件支持哪些消息入口。

## Release 和构建记录

- 每个商城版本对应一个公开、不可覆盖的 GitHub Release tag。
- Release 页面写明变更、兼容范围、数据迁移和已知限制。
- 动态库由公开 GitHub Actions 流水线构建，优先生成 Artifact Attestation。
- 同一个版本的二进制不得原地替换；修复后发布新的 SemVer。
- 构建日志、使用的 Rust 版本、runner 系统和依赖锁应可追踪。

商城流水线不会 checkout 或执行投稿仓库。它只核对公开元数据、资产名称、字节数、SHA256 和可选构建证明。源码可复现性仍由仓库结构和公开流水线保证。

## 数据与权限

在 README 中明确列出插件会访问的资源。至少包括：

| 类型 | 应说明的内容 |
|---|---|
| 文件 | 读写路径、用途、清理方式 |
| 数据库 | 文件位置、表结构迁移、备份和回滚 |
| 网络 | 访问域名、发送的数据、超时和代理要求 |
| 后台任务 | 启动条件、频率、停止方式 |
| Webhook | URL、鉴权、签名、请求体上限 |
| Bot 权限 | 管理员权限、OneBot Action、官方 QQ OpenAPI 权限 |

权限说明必须与代码一致。用“可能需要全部权限”代替逐项说明，或者把敏感行为藏在首次运行之后，都会阻止收录。
