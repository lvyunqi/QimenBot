# 提交插件到 QimenBot 商城

这份教程负责带你走完一次投稿。字段和审核细则分别放在：

- [开源仓库规范](REPOSITORY_RULES.md)
- [驱动兼容声明](DRIVER_COMPATIBILITY.md)
- [构建产物命名](ARTIFACT_NAMING.md)
- [商城 PR 规则](PR_RULES.md)
- [GitHub Actions 发布示例](examples/release.yml)

## 提交前确认

商城只接受满足以下条件的插件：

1. 插件源码位于公开 GitHub 仓库，仓库中有明确的开源许可证文件。
2. 动态插件可以独立构建，不依赖投稿者电脑上的 QimenBot 本地路径。
3. Release 中已经上传与版本文件完全一致的二进制资产。
4. 插件 ID、版本号、Release tag、目标平台和 SHA256 都是真实值。
5. 仓库不包含 Token、Bot Secret、用户配置、数据库或其他隐私数据。
6. OneBot 11 和官方 QQ Bot 的场景、事件及发送能力分别完成实际测试。

开源只代表代码可以检查，不代表插件一定安全。动态插件与 QimenBot 运行在同一进程中，拥有与宿主相同的文件和网络权限。商城收录不是安全担保。

## 第一次提交

从模板复制两个文件：

```text
marketplace/templates/plugin.toml
marketplace/templates/version.toml
```

建立目录：

```text
marketplace/plugins/<plugin-id>/plugin.toml
marketplace/plugins/<plugin-id>/versions/<version>.toml
```

例如插件 ID 是 `group-tools`，版本是 `1.0.0`：

```text
marketplace/plugins/group-tools/plugin.toml
marketplace/plugins/group-tools/versions/1.0.0.toml
```

目录名、`plugin.toml` 中的 `id`、动态库描述符中的 `plugin_id` 必须完全一致。版本文件名必须使用标准 SemVer，不加 `v` 前缀；`release_tag` 可以填写 `v1.0.0`。

## 仓库数字 ID

`repository_id` 不是仓库名称，也不是作者账号 ID。打开下面的地址，把 `OWNER/REPOSITORY` 换成真实仓库：

```text
https://api.github.com/repos/OWNER/REPOSITORY
```

响应顶层的 `id` 数字就是 `repository_id`。商城固定这个数字，即使仓库以后改名或转移，也不会把更新误认成另一个同名仓库。

## 版本和升级规则

- `plugin_id` 是插件的永久身份，发布后不能换给另一个项目使用。
- 版本使用 SemVer，例如 `1.4.2`、`2.0.0-beta.1`。
- 已合并的版本文件不可修改。修复二进制时必须发布新版本。
- 同一插件、同一版本出现不同 SHA256 会被拒绝。
- 一个插件可以登记多个版本，客户端会先过滤兼容性，再选择最高版本。
- SemVer 的 `+build` 元数据不参与优先级；不能用不同构建元数据重复登记同一优先级，修复时应提升预发布号或补丁号。
- `channel = "stable"` 不能使用带预发布后缀的版本号。
- 撤回有问题的版本时提交 PR 将 `yanked` 改为 `true`，不要删除历史文件。
- `data_schema_version` 用于说明插件数据结构变化；存在不可逆迁移时必须设置 `rollback_safe = false`。

用户固定版本后不会自动升级。第三方插件的自动更新默认关闭，手动升级也会再次校验仓库 ID、资产大小和 SHA256。

## 驱动兼容性

驱动兼容信息属于版本文件。即使插件 ID 没变，新版本增加官方 QQ Bot 支持时也应只在新版本的 `drivers` 中声明。

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

只填写真实测试过的能力。OneBot 11 的普通群消息不能直接当作官方 QQ Bot 的群内 @ 测试；官方 QQ 的 openid、群 ID 和频道 ID 也不能按传统 QQ 数字处理。所有可用值和含义见 [驱动兼容声明](DRIVER_COMPATIBILITY.md)。

## 动态插件资产

在线安装只支持动态插件。每个目标平台最多登记一个资产：

| 宿主 | `target` | 文件 |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `.dll` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `.dll` |
| Linux x64 GNU | `x86_64-unknown-linux-gnu` | `.so` |
| Linux ARM64 GNU | `aarch64-unknown-linux-gnu` | `.so` |
| macOS Intel | `x86_64-apple-darwin` | `.dylib` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `.dylib` |

Release 文件名必须包含完整 target。插件 ID 为 `group-tools` 时，示例是：

```text
qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll
libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
libqimen_dynamic_plugin_group_tools-aarch64-apple-darwin.dylib
```

Linux musl 发行包不能加载动态库，因此商城不接受 musl 动态插件资产。GNU/Linux 资产必须填写 `min_glibc`，并在不高于该版本的构建环境中产出。仅执行 `rustup target add` 不能代替对应 CPU 的 linker 和 libc。

计算 SHA256：

```bash
# Linux / macOS
sha256sum libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
```

```powershell
# Windows
Get-FileHash .\qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll -Algorithm SHA256
```

`size_bytes` 填写 Release 资产的准确字节数。推荐使用 GitHub Actions 构建并为 Release 生成 Artifact Attestation；有证明时将该资产的 `github_attestation` 设为 `true`。这个字段只表示存在可验证的构建证明，不会替代主目录中的 SHA256。

PR 流水线会按资产 SHA256 查询 GitHub Attestations API。填写 `true` 但查不到对应证明时，目录校验不会通过。

完整命名公式、六个平台的固定名称和 Cargo 产物重命名方法见 [构建产物命名](ARTIFACT_NAMING.md)。可以把 [发布流水线示例](examples/release.yml) 复制到插件仓库的 `.github/workflows/release.yml`，再替换插件 ID 和库名。

## 静态插件

静态插件可以进入商城作为源码项目展示，但不能提供 `dynamic_api` 和 `assets`。管理面板会明确提示用户：静态插件必须加入 QimenBot 源码依赖、重新构建 `qimenbotd` 并重启，不支持一键安装或热更新。

## 本地检查

在 QimenBot 仓库根目录执行：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check marketplace
```

需要同时核对 GitHub 仓库和 Release 元数据时：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check --verify-github marketplace
```

检查过程不会下载或执行插件代码。PR 流水线会重复执行相同检查。

## PR 范围

一个 PR 只提交一个插件或一个插件的新版本，方便独立审核和回退。PR 说明中写清：

- 插件用途和主要命令或事件；
- 源码仓库、许可证和 Release 链接；
- QimenBot 兼容范围、动态 ABI API；
- 实际构建的 target 和 Linux 最低 glibc；
- 是否读写文件、访问网络、启动后台线程或提供 Webhook；
- 数据迁移以及回滚限制。

`trust = "community"` 是投稿默认值。`verified-build` 和 `official` 由维护者根据构建证明、代码归属和审核结果调整，投稿者不能自行声明。

提交前使用 [.github/PULL_REQUEST_TEMPLATE/marketplace.md](../.github/PULL_REQUEST_TEMPLATE/marketplace.md) 逐项核对。历史版本不可修改、撤回规则和不接受的投稿类型见 [商城 PR 规则](PR_RULES.md)。
