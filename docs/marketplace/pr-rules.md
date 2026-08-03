# PR 与版本规则

商城 PR 是对“某个公开仓库的某个不可修改版本”做登记。PR 中不放插件源码和动态库，只提交 `marketplace/plugins/` 下的 TOML 元数据。

## 投稿前准备

先确认以下项目已经完成：

- 源码仓库公开，许可证文件和 README 齐全；
- 插件 ID 已确定，动态描述符使用同一 ID；
- 版本 tag 和 GitHub Release 已发布；
- 驱动、场景、事件和发送能力已经测试；
- 每个登记 target 的文件名、大小、SHA256 和最低 glibc 已核对；
- 数据迁移与 `rollback_safe` 结论明确；
- 仓库中没有 Secret、用户配置、数据库和编译产物。

商城原始模板：

- [`marketplace/templates/plugin.toml`](https://github.com/lvyunqi/QimenBot/blob/main/marketplace/templates/plugin.toml)
- [`marketplace/templates/version.toml`](https://github.com/lvyunqi/QimenBot/blob/main/marketplace/templates/version.toml)

## 首次收录

插件 ID 为 `group-tools`、版本为 `1.0.0`：

```text
marketplace/plugins/group-tools/plugin.toml
marketplace/plugins/group-tools/versions/1.0.0.toml
```

`plugin.toml` 示例：

```toml
schema_version = 1
id = "group-tools"
name = "群管理工具"
summary = "提供可审计的群管理命令。"
description = "说明主要功能、权限和数据行为。"
type = "dynamic"
repository = "owner/qimen-plugin-group-tools"
repository_id = 123456789
license = "MIT"
authors = ["作者名称"]
categories = ["management"]
keywords = ["group", "moderation"]
homepage = "https://github.com/owner/qimen-plugin-group-tools"
trust = "community"
```

目录名、`id` 和动态描述符 `plugin_id` 必须完全相同。插件 ID 只能使用小写 ASCII 字母、数字和单个连字符，长度 2 到 64，不能以连字符开头或结尾。

投稿者使用 `trust = "community"`。`verified-build` 和 `official` 由维护者根据仓库归属与构建证明调整，不能自行申请一个字段值绕过审核。

## 提交新版本

已有插件升级到 `1.1.0` 时，只新增：

```text
marketplace/plugins/group-tools/versions/1.1.0.toml
```

不要复制一份新的 `plugin.toml`，也不要修改 `1.0.0.toml`。客户端会保留多个版本，先按 QimenBot、ABI、target、glibc 和撤回状态过滤，再选择最高兼容 SemVer。

SemVer 的 `+build` 元数据不参与版本优先级。不能用 `1.0.0+build.2` 代替补丁升级；修复已发布资产时使用 `1.0.1`。

## 撤回有问题的版本

唯一允许的历史版本修改是：

```toml
yanked = true
```

不要删除版本文件或 Release，也不要替换二进制。撤回后新安装不会选择该版本，已经安装的用户仍能看到来源和校验记录。已经撤回的版本不能原地恢复；修复后发布新版本。

## 永久不变的身份

首次合并后不能修改：

- `plugin.toml` 的 `id`；
- 插件 `type`；
- GitHub 数字 `repository_id`。

仓库正常改名或转移时数字 ID 不变，可以在新版本 PR 中更新可读的 `repository = "owner/name"`。如果数字 ID 变了，商城会把它视为另一个来源，不能沿用原插件身份。

## 本地校验

在 QimenBot 仓库根目录运行：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check marketplace
```

这一步检查：

- 目录名、插件 ID 和版本文件名；
- TOML 字段、Schema 和未知字段；
- SemVer、通道、QimenBot 范围和动态 ABI；
- 驱动、场景、事件和发送能力；
- target、固定资产名、SHA256、字节数和最低 glibc；
- 重复版本与构建元数据优先级冲突。

再核对 GitHub 公开数据：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check --verify-github marketplace
```

它会检查仓库公开状态、数字 ID、许可证、Release tag、资产名、字节数和可选 Attestation，不会下载或执行插件。

GitHub 匿名 API 额度不足时，可以临时设置只读 Token：

```bash
export GITHUB_TOKEN="<temporary-token>"
```

Windows PowerShell：

```powershell
$env:GITHUB_TOKEN = "<temporary-token>"
```

Token 不要写进 TOML、README、脚本或终端截图。检查完成后从当前 shell 清除。

## PR 范围

一个 PR 只做下面一项：

1. 首次收录一个插件。
2. 为一个插件增加一个版本。
3. 撤回一个版本。

不要把多个无关插件、框架功能修改和商城投稿放在同一 PR。这样审核失败时可以独立修正，合并后也能准确追踪是谁登记了哪个版本。

使用仓库的[商城 PR 模板](https://github.com/lvyunqi/QimenBot/blob/main/.github/PULL_REQUEST_TEMPLATE/marketplace.md)，说明：

- 插件用途、主要命令和事件；
- 仓库、数字 ID、许可证和 Release；
- QimenBot、动态 ABI 和驱动场景；
- target 与最低 glibc；
- 文件、数据库、网络、后台线程和 Webhook；
- 数据迁移和回滚限制；
- 两条校验命令的结果。

## 流水线通过后还会检查什么

自动校验只能证明元数据内部一致、公开来源存在。维护者还会查看：

- 权限是否超过功能需要；
- 源码 tag 与 Release 是否对应；
- 依赖是否公开、固定、可追踪；
- 后台线程和 Webhook 是否能安全停止；
- 数据迁移是否可能破坏回滚；
- 驱动声明是否有测试和 README 依据；
- 高风险网络、文件和命令执行行为是否明确披露。

商城收录不是安全担保。插件在用户机器上运行前，管理员仍应检查源码、权限和版本变更。

## 会被直接拒绝的投稿

- 私有仓库、闭源二进制或许可证不明确；
- 动态插件依赖本地 path 或私有 registry；
- Release 文件缺失、重名、大小或 SHA256 不一致；
- 把 OneBot 11 测试结果写成官方 QQ Bot 全场景兼容；
- 登记 musl 动态库；
- 修改或删除已发布版本；
- 提交真实凭据、用户配置、数据库或插件二进制；
- 投稿者自行声明 `verified-build` 或 `official`。
