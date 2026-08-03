# 插件商城发布

## 目录

- [适用范围](#适用范围)
- [先判断插件类型](#先判断插件类型)
- [准备公开仓库](#准备公开仓库)
- [固定插件身份和版本](#固定插件身份和版本)
- [按版本声明驱动](#按版本声明驱动)
- [构建动态资产](#构建动态资产)
- [发布 Release](#发布-release)
- [填写商城文件](#填写商城文件)
- [验证并提交 PR](#验证并提交-pr)
- [发布检查表](#发布检查表)

## 适用范围

仅在开发、发布或维护准备进入 QimenBot 商城的插件时使用本参考。它指导插件作者准备公开仓库、Release 和商城 PR，不指导实现 QimenBot 商城后端、管理面板或索引器。

权威入口：

- 作者规范：<https://lvyunqi.github.io/QimenBot/marketplace/>
- 用户安装教程：<https://lvyunqi.github.io/QimenBot/plugin/marketplace.html>
- 投稿教程：<https://github.com/lvyunqi/QimenBot/blob/main/marketplace/CONTRIBUTING.md>
- 驱动声明：<https://github.com/lvyunqi/QimenBot/blob/main/marketplace/DRIVER_COMPATIBILITY.md>
- 产物命名：<https://github.com/lvyunqi/QimenBot/blob/main/marketplace/ARTIFACT_NAMING.md>
- 发布工作流：<https://github.com/lvyunqi/QimenBot/blob/main/marketplace/examples/release.yml>

提交前读取目标分支中的模板和 Schema。线上文档可能对应已发布版本，仓库内开发以当前分支校验器为准。

## 先判断插件类型

- 动态插件可以登记 Release 动态库，支持商城安装、更新、回滚和卸载。
- 静态插件只能登记源码、版本与驱动范围；用户必须把它加入 QimenBot 源码并重新构建 `qimenbotd`。
- 不要为了获得一键安装而把需要 async、完整 `OneBotActionClient` 或深度宿主生命周期的静态插件强改成动态插件。

动态插件投稿可以完全在独立公开仓库完成，不需要 QimenBot 主框架源码。使用 crates.io 发布的 `abi-stable-host-api` 与 `qimen-dynamic-plugin-derive`，不能依赖作者电脑上的 QimenBot `path`。

## 准备公开仓库

确认：

1. GitHub 仓库公开，根目录有明确的开源许可证文件。
2. README 说明功能、命令、事件、配置、权限、数据、网络、后台线程、Webhook、构建和部署。
3. README 分别说明 OneBot 11 与官方 QQ Bot 的真实测试范围和平台权限。
4. 动态插件提交 `Cargo.lock`，Release 使用 `cargo build --locked`。
5. 依赖来自 crates.io 或固定到公开 commit 的 Git 仓库，不使用本地 path 与私有 registry。
6. 仓库没有 Secret、Token、Cookie、用户配置、数据库或编译后的动态库。

获取 GitHub 数字仓库 ID：

```text
GET https://api.github.com/repos/OWNER/REPOSITORY
```

读取响应顶层 `id`。不要使用账号 ID、Release ID 或可复用的 `owner/name` 代替数字身份。

## 固定插件身份和版本

目录结构：

```text
marketplace/plugins/<plugin-id>/
├── plugin.toml
└── versions/
    ├── 1.0.0.toml
    └── 1.1.0.toml
```

保持以下值一致：

- 目录 `<plugin-id>`；
- `plugin.toml` 的 `id`；
- 动态描述符 `plugin_id`；
- `config/plugins/<plugin-id>.toml` 文件名主体。

首次合并后，插件 ID、插件类型和 `repository_id` 不可更换。版本使用标准 SemVer。版本文件名不带 `v`，`release_tag` 可以带 `v`。

已合并的版本不可修改或删除。唯一允许的历史修改是 `yanked = false` 改成 `true`。修复资产、兼容范围或驱动声明时发布新的 SemVer。

不要用构建元数据制造升级：`1.0.0+build.2` 不高于 `1.0.0+build.1`。使用补丁版本或新的预发布标识。

## 按版本声明驱动

每份版本文件必须有一个或两个 `[[drivers]]`。只允许：

- `onebot11`：OneBot 11 普通消息驱动；
- `qq-official`：QQ 开放平台 Gateway/OpenAPI 驱动。

消息场景：

| 值 | 含义 |
|---|---|
| `private` | OneBot 私聊或官方 QQ 单聊/C2C |
| `group` | OneBot 普通群消息 |
| `group-at` | 官方 QQ 群内 @ |
| `channel` | OneBot 实现的频道普通消息扩展 |
| `channel-at` | 官方 QQ 频道 @ |
| `channel-private` | 频道私信 |

OneBot 11 不能声明 `group-at` 或 `channel-at`。官方 QQ 的 openid、群 ID、频道 ID 按字符串处理，不强转为传统 QQ 数字。

事件值：`message`、`notice`、`request`、`meta`。

发送值：`reply`、`proactive`、`rich-message`。声明 `rich-message` 时同时声明 `reply` 或 `proactive`。

示例：

```toml
[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message", "notice"]
outbound = ["reply", "proactive", "rich-message"]

[[drivers]]
driver = "qq-official"
scenes = ["private", "group-at", "channel-at", "channel-private"]
events = ["message", "notice"]
outbound = ["reply", "proactive", "rich-message"]
```

不要根据共用处理函数推断跨驱动兼容。逐驱动验证事件解析、字符串 ID、回复窗口、主动发送权限和实际使用的富媒体消息段。

驱动矩阵用于商城展示、搜索和审核，不是运行时沙箱或权限开关。

## 构建动态资产

商城支持：

| target | Release 资产名，示例 ID `group-tools` |
|---|---|
| `x86_64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll` |
| `aarch64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-aarch64-pc-windows-msvc.dll` |
| `x86_64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so` |
| `aarch64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-aarch64-unknown-linux-gnu.so` |
| `x86_64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-x86_64-apple-darwin.dylib` |
| `aarch64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-aarch64-apple-darwin.dylib` |

命名算法：

1. 把插件 ID 的 `-` 换成 `_`。
2. 使用 `qimen_dynamic_plugin_<id>` 库名主体。
3. Windows 不加 `lib`，Unix 加 `lib`。
4. 在扩展名前加入 `-<完整 target>`。

Cargo 原始产物不带 target 后缀。只在上传 GitHub Release 前复制或重命名，不要把 target 写进 Rust `[lib].name`。

每个动态 `[[assets]]` 填写：

- 完整 `target`；
- 固定 `asset_name`；
- 动态库本身的 64 位 SHA256；
- 动态库本身的准确 `size_bytes`；
- GNU/Linux 的真实 `min_glibc`；
- 是否存在对应 SHA256 的 GitHub Artifact Attestation。

不要登记压缩包或 musl 动态库。musl QimenBot 包不支持动态加载。

GNU/Linux 优先在 Debian 11 / glibc 2.31 或更旧的可信环境构建。不能在 glibc 2.39 上构建后把 `min_glibc` 手工写成 2.31。用 `readelf --version-info`、`ldd` 和实际旧系统验证。

## 发布 Release

使用仓库提供的 `marketplace/examples/release.yml` 作为插件仓库工作流起点。修改：

```yaml
env:
  PLUGIN_ID: <稳定插件 ID>
  LIB_STEM: qimen_dynamic_plugin_<下划线 ID>
```

保留：

- Rust 1.89 格式、Clippy 和测试；
- `cargo build --locked`；
- 对应 OS/CPU 的原生 runner；
- Debian 11 Linux 容器；
- 固定资产重命名；
- SHA256；
- `actions/attest-build-provenance`；
- GitHub Release 上传。

发布前让 `Cargo.toml`、动态描述符、Git tag 和商城版本一致。所有准备登记的 target 构建成功后再提交商城 PR。

## 填写商城文件

从目标 QimenBot 分支复制：

```text
marketplace/templates/plugin.toml
marketplace/templates/version.toml
```

动态版本至少提供一个资产。静态版本不能填写 `dynamic_api` 与 `assets`，但仍必须填写 `drivers`。

`data_schema_version` 记录持久化数据结构。升级存在不可逆迁移时递增它，并设置 `rollback_safe = false`。二进制可替换不等于数据可回滚。

## 验证并提交 PR

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check marketplace
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check --verify-github marketplace
```

第二条命令读取 GitHub 仓库和 Release 元数据，不 checkout 或执行投稿插件。匿名额度不足时临时设置只读 `GITHUB_TOKEN`，不要提交或展示 Token。

一个 PR 只做首次收录、增加一个版本或撤回一个版本。PR 说明列出：

- 插件功能、命令和事件；
- 仓库、数字 ID、许可证和 Release；
- QimenBot、动态 API 和驱动场景；
- target、glibc、大小和 SHA256；
- 文件、数据库、网络、后台线程、Webhook；
- 数据迁移和回滚限制；
- 两条校验命令的真实结果。

流水线通过不代表代码安全。动态插件与宿主在同一进程运行，最终仍要由用户检查源码和权限。

## 发布检查表

- 插件类型选择符合源码和部署边界。
- 独立动态仓库不依赖 QimenBot 本地 path。
- 仓库公开、许可证明确、README 披露权限和数据行为。
- 插件 ID、版本、tag、描述符和目录一致。
- OneBot 11 与官方 QQ Bot 能力按版本、按真实场景分别声明。
- 每个资产名包含完整 target，并匹配 OS、CPU、GNU/MSVC 和扩展名。
- GNU/Linux 使用真实 glibc 基线，不登记 musl。
- 版本文件固定资产大小和 SHA256，证明状态真实。
- 不修改历史版本；修复时新增 SemVer。
- 本地检查和 GitHub 元数据检查均通过。
