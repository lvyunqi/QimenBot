# 商城 PR 规则

商城 PR 只登记公开源码仓库和已经发布的版本，不接收插件源码、动态库、配置、数据库或用户数据。

## 一个 PR 只做一件事

以下三种投稿应分开提交：

1. 首次收录一个插件。
2. 为已收录插件增加一个版本。
3. 撤回一个有问题的历史版本。

不要在同一个 PR 中登记多个无关插件，也不要夹带 QimenBot 框架功能修改。这样审核记录、回退和责任边界才清楚。

## 首次收录

新增以下文件：

```text
marketplace/plugins/<plugin-id>/plugin.toml
marketplace/plugins/<plugin-id>/versions/<version>.toml
```

`plugin-id` 必须同时等于：

- 目录名；
- `plugin.toml` 的 `id`；
- 动态插件描述符的 `plugin_id`；
- `config/plugins/<plugin-id>.toml` 使用的文件名主体。

首次发布后，插件 ID、插件类型和 GitHub 数字仓库 ID 不能更换。需要完全不同的项目时使用新的插件 ID。

## 提交新版本

只新增一份不可修改的版本文件：

```text
marketplace/plugins/<plugin-id>/versions/1.2.0.toml
```

版本文件名使用标准 SemVer，不带 `v`。`release_tag` 可以是 `v1.2.0`。提交 PR 前，Release、二进制资产和构建证明必须已经公开可访问，不能先合并目录再补文件。

已经合并的版本元数据不可原地修正，包括兼容范围、驱动声明、资产名、大小和 SHA256。发现错误时发布新版本。唯一允许的历史修改是把 `yanked = false` 改成 `yanked = true`。

## 撤回版本

撤回只修改目标版本的 `yanked`：

```toml
yanked = true
```

不要删除版本文件或 Release，不要换回新的二进制。撤回后客户端不会把该版本作为新的安装候选，本地已经安装的用户仍能看到准确的来源记录。

## PR 说明必须包含

- 插件 ID、名称、用途和主要命令或事件；
- 公开源码仓库、数字仓库 ID、许可证和 Release 链接；
- 本次版本号、QimenBot 范围和动态 ABI API；
- 支持的 OneBot 11 / 官方 QQ Bot 场景；
- 实际发布的 target 和 GNU/Linux 最低 glibc；
- 文件、数据库、网络、后台线程和 Webhook 使用情况；
- 数据结构变化、迁移方式以及能否安全回滚；
- 本地目录校验和 GitHub 元数据校验结果。

可以直接使用 [商城 PR 模板](../.github/PULL_REQUEST_TEMPLATE/marketplace.md)。

## 不接受的内容

- 私有仓库、闭源二进制或没有明确开源许可证的项目；
- 只上传压缩包而没有可审查源码的项目；
- 依赖作者电脑本地路径、私有 Cargo registry 或不可访问依赖的动态插件；
- 包含真实 Token、Bot Secret、Cookie、用户配置或数据库的仓库；
- Release 资产名、target、大小或 SHA256 与版本文件不一致；
- 把 OneBot 11 测试结果直接写成官方 QQ Bot 兼容；
- musl 动态插件资产；
- 投稿者自行设置 `verified-build` 或 `official` 信任级别。

## 本地检查

只检查目录结构、TOML 和内部一致性：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check marketplace
```

同时检查 GitHub 仓库公开状态、数字 ID、许可证、Release 资产和构建证明：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check --verify-github marketplace
```

第二条命令需要访问 GitHub API。遇到匿名额度限制时设置只读 `GITHUB_TOKEN`，不要把 Token 写入文件或提交到仓库。

## 审核结果

流水线通过只说明元数据和公开来源符合规则，不代表代码安全。维护者仍会检查权限范围、依赖来源、版本历史、构建方式和高风险行为。动态插件与 QimenBot 运行在同一进程中，最终安装决定由管理员负责。
