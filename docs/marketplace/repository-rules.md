# 开源仓库规范

插件商城不托管第三方源码。用户点击“源码仓库”后，应该能够确认代码、许可证、权限、构建方式和 Release 之间的关系，而不是只看到一个二进制下载页。

## 必须满足的条件

- GitHub 仓库公开，源码无需申请权限即可读取。
- 根目录有明确的开源许可证文件。
- `plugin.toml` 使用对应的 SPDX 标识，例如 `MIT`、`Apache-2.0`。
- Release 中功能对应的完整源码已经提交并打 tag。
- 依赖可以从 crates.io 或公开、固定 commit 的 Git 仓库获取。
- 仓库没有真实 Secret、Token、Cookie、用户配置和数据库。
- 有公开的 Issue 或其他反馈入口，并说明安全问题的报告方式。

`UNLICENSED`、`Proprietary`、`NOASSERTION` 或只有 README 中一句“可以自由使用”都不算明确的开源许可证。

## GitHub 数字仓库 ID

商城固定数字 `repository_id`，不只依赖可改名、可复用的 `owner/name`。

访问：

```text
https://api.github.com/repos/OWNER/REPOSITORY
```

响应顶层示例：

```json
{
  "id": 123456789,
  "name": "qimen-plugin-group-tools",
  "full_name": "owner/qimen-plugin-group-tools"
}
```

版本文件填写 `123456789`。不要使用作者账号 ID、Release ID 或网页上看见的其他数字。仓库以后改名或转移时，数字 ID 仍保持不变。

## README 要让用户看懂什么

### 功能

列出主要命令、事件、拦截器、后台任务和 Webhook。每个管理类命令应说明所需角色或 Bot 权限。

### 兼容性

写清 QimenBot 版本、动态 ABI API、OneBot 11 服务端、官方 QQ Bot 场景和实际测试平台。官方 QQ 还应列出需要开启的 Intents 与 OpenAPI 权限。

### 配置

给出 `config/plugins/<plugin-id>.toml` 的完整字段、默认值和最小示例。Secret 使用明显占位符，不要提交测试账号的真实值。

### 数据和网络

说明插件读写哪些路径、数据库如何迁移、访问哪些域名、发送什么数据、后台任务多久执行一次，以及卸载后哪些文件会保留。

### 构建和部署

给出 Rust 版本、`cargo build --locked --release` 命令、产物位置、支持 target、最低 glibc 和复制到 `plugins/bin` 的方式。

### 升级和回滚

有数据库或配置迁移时说明备份、迁移方向和旧版本是否还能读取新数据。不要因为二进制可以替换，就把 `rollback_safe` 一律写成 `true`。

## 动态插件必须独立构建

商城一键安装只服务于动态 `cdylib`。独立仓库不应要求用户先下载 QimenBot 主框架源码：

```toml
[package]
name = "qimen-dynamic-plugin-group-tools"
version = "1.0.0"
edition = "2024"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]

[dependencies]
abi-stable-host-api = "<crates.io 已发布版本>"
qimen-dynamic-plugin-derive = "<配套的 crates.io 已发布版本>"
```

发布仓库应提交 `Cargo.lock`，流水线使用 `--locked`。以下依赖不会通过审核：

```toml
# 作者电脑路径
abi-stable-host-api = { path = "../QimenBot/crates/abi-stable-host-api" }

# 无法公开访问的 registry
some-crate = { version = "1", registry = "company-private" }
```

公开 Git 依赖应固定 commit，不能长期跟随可变分支。crate 版本、tag 和 `#[dynamic_plugin]` 描述符版本应一致。

后台线程必须在 `#[shutdown]` 中停止并 `join`，否则 Windows 热更新可能因为 DLL 仍被占用而失败。商城会为不同资产分配带 SHA256 的独立活动文件名，以避免 Linux 延迟卸载旧 Rust 动态库时误用旧版本；这不能替代插件自己的关闭逻辑。Webhook 必须考虑鉴权、签名、时间戳、超时、请求体上限和重放。

## 静态插件需要额外说明

静态插件依赖 QimenBot 源码和重新编译。仓库 README 应写明：

- 支持的 QimenBot tag 或 commit；
- 需要加入的 Cargo 依赖与 feature；
- 模块注册步骤；
- 完整构建命令；
- 升级 QimenBot 后如何重新合并和验证。

静态插件版本不能提供 `dynamic_api` 与 `assets`。商城只展示源码和版本级驱动范围，不会修改用户的 `qimenbotd`。

## 权限清单示例

| 资源 | 示例写法 |
|---|---|
| 文件 | “写入 `data/group-tools/state.json`，卸载不会自动删除” |
| 数据库 | “SQLite 位于 `data/group-tools/main.db`，1.2.0 升级 schema 2” |
| 网络 | “只请求 `api.example.com`，发送群 ID 和查询关键字，10 秒超时” |
| 后台任务 | “启用后每 60 秒刷新一次，shutdown 等待线程退出” |
| Webhook | “`POST /webhooks/group-tools/events`，要求 HMAC 和 5 分钟时间窗” |
| Bot 权限 | “OneBot 需要群管理员；官方 QQ 需要群消息和主动消息权限” |

含糊写成“插件可能访问网络”没有审核价值。权限应与当前版本代码相符，新增高风险行为时在 Release 说明中单独列出。

## Release 可追踪性

每个商城版本对应一个公开 Git tag 和 GitHub Release。推荐使用公开 GitHub Actions 构建动态库并生成 Artifact Attestation，让用户可以从资产追溯到仓库、commit 和工作流。

构建证明不能替代 SHA256，也不能证明代码没有安全问题。商城分别核对仓库数字 ID、Release tag、资产名、字节数、SHA256 和证明，任何一项不一致都会停止安装。
