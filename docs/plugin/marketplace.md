# 插件商城

QimenBot 的插件商城使用 GitHub 保存源码、Release 资产和审核目录，不需要单独部署商城服务器。插件作者向 [`lvyunqi/QimenBot`](https://github.com/lvyunqi/QimenBot) 主仓库提交目录 PR；合并后，管理面板从该仓库自动发布的 GitHub Pages 索引读取数据，安装时再到插件自己的 GitHub Release 下载对应二进制。

商城只自动管理动态插件。静态插件可以展示源码和兼容范围，但必须重新构建 `qimenbotd`，不能在运行中的宿主里一键安装。

本页面向安装和管理插件的用户。准备投稿的作者请从[商城规范总览](/marketplace/)开始。

## 先理解三个位置

| 位置 | 保存内容 | 谁负责 |
|---|---|---|
| QimenBot 主仓库 `marketplace/` | 插件身份、版本、兼容范围、资产名称和审核后的 SHA256 | QimenBot 维护者审核 PR |
| 插件作者的公开仓库 | 完整源码、许可证、README、Issue | 插件作者 |
| 插件仓库的 GitHub Releases | Windows、Linux、macOS 动态库 | 插件作者发布 |

主仓库不接收第三方 DLL、SO、dylib、配置或数据库。PR 流水线只检查 TOML 和 GitHub 元数据，不会下载、编译或执行投稿插件。

目录 PR 合并到 `main` 后会触发 Pages 流水线。流水线再次校验目录并生成静态索引，部署成功后客户端自动读取新内容，不需要发布新的 QimenBot 版本。这个过程不是严格实时，通常需要一到几分钟；部署失败时继续保留上一次成功发布的目录。

## 在管理面板安装

启动 QimenBot 后打开管理面板，进入“插件商城”。页面顶部会显示当前宿主的四项信息：

- QimenBot 版本；
- Rust 目标平台，例如 `x86_64-unknown-linux-gnu`；
- 宿主支持的动态 ABI API；
- 动态加载和 glibc 状态。

安装步骤：

1. 在目录列表中选择插件。
2. 查看源码仓库、许可证和信任级别。
3. 查看列表中的 `OneBot 11`、`官方 QQ Bot` 徽标，确认插件支持正在使用的驱动。
4. 在详情中选择版本，核对“场景、事件、发送”兼容矩阵。
5. 确认页面显示“当前宿主兼容”，再检查 target、最低 glibc、资产大小和 SHA256。
6. 点击“安装此版本”，阅读确认框后再安装。

安装过程中宿主会依次完成：

```text
读取静态目录
  -> 核对 GitHub 数字仓库 ID
  -> 读取指定 Release 和资产
  -> 下载到商城缓存目录
  -> 校验字节数与主目录 SHA256
  -> 读取动态插件描述符
  -> 安全卸载现有动态库
  -> 替换 plugins/bin 中的活动文件
  -> 重新扫描并执行 init
  -> 成功后写入安装锁
```

任一步失败都会停止安装。文件已经替换但插件加载失败时，宿主会恢复旧文件并重新加载旧版本。

## 同一个插件有多个版本

商城把 `plugin_id` 当作永久身份，把 SemVer 当作不可修改的发布记录。例如：

```text
group-tools
├── 1.0.0
├── 1.1.0
└── 2.0.0-beta.1
```

客户端不会简单选择目录中最大的版本，而是按下面的顺序过滤：

1. 是否允许预发布版本；
2. 版本是否已经撤回；
3. 当前 QimenBot 是否满足 `qimenbot` 兼容范围；
4. 动态 ABI API 是否受宿主支持；
5. OS、CPU 和 GNU/MSVC target 是否完全一致；
6. 当前 glibc 是否不低于资产的 `min_glibc`；
7. 过滤完成后选择最高 SemVer。

驱动矩阵用于展示和审核，不参与二进制 target 选择。一个 QimenBot 宿主可以同时配置 OneBot 11 和官方 QQ Bot，管理员仍要确认目标机器人位于所选版本声明的驱动与场景中。

同一 `plugin_id` 和同一版本只能有一个审核后的 SHA256。作者不能在 GitHub Release 中悄悄替换同名资产，再让用户继续把它当作原版本。需要修复二进制时必须发布新版本。

SemVer 的 `+build` 元数据不表示更高版本。商城不会把 `1.0.0+build.2` 当作 `1.0.0+build.1` 的升级，也不允许同一插件同时登记两个优先级相同、只在构建元数据上不同的版本。

### 稳定版和预发布版

默认只选择 `channel = "stable"`。需要测试 beta 或 rc 时，在“配置 -> 插件商城”打开“接收预发布版本”。这个开关只让预发布版本参与选择，不会自动更新插件。

### 固定版本

已安装插件右侧的图钉按钮可以固定当前版本。固定后仍能看到目录信息，但更新操作不可用。取消固定后，商城才会提示更高的兼容版本。

第三方插件自动更新当前不开放。新版本代码会进入 QimenBot 进程，默认要求管理员逐次检查并确认。

## 更新、回滚和卸载

### 更新

选择高于当前安装的兼容版本，点击“更新到此版本”。商城会保留上一版审核二进制，`plugins/bin` 中只放当前活动文件，因此运行时不会同时扫描到两个版本。

商城不允许把“安装”按钮当作任意降级入口。需要恢复上一版时使用回滚操作。

### 回滚

只有同时满足以下条件时，回滚按钮才可用：

- 本地保留了上一版审核二进制；
- 当前版本登记了 `rollback_safe = true`；
- 活动插件属于商城管理。

插件升级可能修改 SQLite 表、配置结构或其他持久化数据。二进制可以换回去，不代表旧代码还能读取新数据。作者存在不可逆迁移时必须填写 `rollback_safe = false`，并增加 `data_schema_version`。

### 卸载

卸载会安全停止插件并移除 `plugins/bin` 中的活动二进制，不会删除：

- `config/plugins/<plugin_id>.toml`；
- 插件数据库和数据目录；
- 商城缓存中的审核版本；
- 机器人配置。

确认不再需要数据时，由管理员在停止 QimenBot 后自行备份和清理。商城不会猜测第三方插件把数据放在哪里。

## 关联手工复制的插件

手工放进 `plugins/bin` 的动态库没有安装来源，商城会把它显示为“待关联”或“校验不符”，不会自动接管。

只有以下信息全部匹配时才能关联：

- 描述符中的 `plugin_id`；
- 描述符版本；
- 当前 target 的 Release 资产；
- 文件字节数；
- SHA256。

关联成功后，商城会把当前文件复制到审核缓存、写入安装锁，并默认固定版本。SHA256 不一致时不能仅凭文件名或版本号强行关联。

## 信任级别

| 标记 | 含义 |
|---|---|
| 社区 | 目录和 GitHub 元数据已通过规则检查，代码安全由用户自行评估 |
| 构建已验证 | Release 资产提供了审核认可的构建证明 |
| 官方 | 由 QimenBot 项目维护的插件 |

信任级别不是权限沙箱。动态插件在宿主进程内运行，可以访问宿主账号能够访问的文件和网络。安装前仍要检查源码和权限需求。

## 配置

默认配置：

```toml
[marketplace]
enabled = true
cache_dir = "cache/marketplace"
lock_path = "config/marketplace-lock.toml"
request_timeout_secs = 30
allow_prerelease = false
auto_update = false
```

| 字段 | 说明 |
|---|---|
| `enabled` | 是否允许面板读取商城目录和执行商城安装 |
| `cache_dir` | 目录缓存、下载资产、历史审核版本和临时事务目录 |
| `lock_path` | 本地安装锁，保存版本、仓库 ID、target、SHA256、固定和回滚状态 |
| `request_timeout_secs` | 目录、GitHub API 和下载请求的超时，允许 1-300 秒 |
| `allow_prerelease` | 是否让预发布版本参与兼容性选择 |
| `auto_update` | 当前必须为 `false`，第三方插件更新需要人工确认 |

商城来源固定为 QimenBot 主仓库发布的官方目录。配置文件和管理面板都不提供来源切换，旧配置中的 `catalog_url` 会被忽略，并在下次通过管理面板保存配置时移除。

### Docker 持久化

官方容器把 `/data` 作为工作目录。已有部署通常已经映射：

```yaml
volumes:
  - ./data/config:/data/config
  - ./data/plugins:/data/plugins
  - ./data/logs:/data/logs
  - ./data/marketplace:/data/cache/marketplace
```

默认 `cache_dir = "cache/marketplace"` 对应容器内的 `/data/cache/marketplace`。官方 Compose 已把它映射到宿主机 `./data/marketplace`；`lock_path` 位于已映射的 `/data/config`。升级容器前应同时备份配置、插件和商城缓存，否则回滚所需的上一版二进制可能丢失。

## 作者提交插件

商城登记结构：

```text
marketplace/plugins/<plugin-id>/
├── plugin.toml
└── versions/
    ├── 1.0.0.toml
    └── 1.1.0.toml
```

第一次提交从主仓库中的两个模板开始：

- `marketplace/templates/plugin.toml`
- `marketplace/templates/version.toml`

### `plugin.toml`

```toml
schema_version = 1
id = "group-tools"
name = "群管理工具"
summary = "提供可审计的群管理命令。"
type = "dynamic"
repository = "owner/qimen-plugin-group-tools"
repository_id = 123456789
license = "MIT"
authors = ["作者名称"]
categories = ["management"]
keywords = ["group", "moderation"]
trust = "community"
```

`repository_id` 是 GitHub 仓库的数字 ID，不是作者账号 ID。访问下面的 API，把仓库名替换成真实值：

```text
https://api.github.com/repos/OWNER/REPOSITORY
```

响应顶层的 `id` 就是要填写的数字。仓库以后改名或转移时，这个数字仍能阻止同名仓库冒充更新来源。

### 版本文件

```toml
schema_version = 1
version = "1.0.0"
released_at = "2026-08-03T00:00:00Z"
release_tag = "v1.0.0"
channel = "stable"
qimenbot = ">=0.1.16, <0.2.0"
dynamic_api = "0.5"
yanked = false
data_schema_version = 1
rollback_safe = true
changelog = "首次发布。"

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

[[assets]]
target = "x86_64-unknown-linux-gnu"
asset_name = "libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so"
sha256 = "完整的 64 位十六进制 SHA256"
size_bytes = 123456
min_glibc = "2.31"
github_attestation = false
```

动态插件至少提供一个资产。每个 target 在同一版本中只能出现一次。GNU/Linux 必须填写最低 glibc；Windows 和 macOS 不能填写 `min_glibc`。

`drivers` 也是版本级必填项。`onebot11` 表示 OneBot 11 普通消息驱动，`qq-official` 表示 QQ 开放平台驱动；场景要区分普通群聊 `group` 与官方群内 @ `group-at`。完整字段见[驱动兼容声明](/marketplace/driver-compatibility)。

`github_attestation = true` 时，PR 流水线会按资产 SHA256 查询 GitHub Attestations API。没有对应证明的版本不会进入目录；这个证明仍不能替代安装时的 SHA256 校验。

静态插件的版本文件不能填写 `dynamic_api` 和 `assets`。面板只提供源码仓库链接。

### 本地验证

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check marketplace
```

同时核对 GitHub 公开状态、许可证、数字仓库 ID 和 Release 资产：

```bash
cargo run --locked -p qimen-plugin-marketplace --bin qimen-marketplace-index -- --check --verify-github marketplace
```

完整作者资料：

- [开源仓库规范](/marketplace/repository-rules)
- [驱动兼容声明](/marketplace/driver-compatibility)
- [构建产物命名](/marketplace/artifact-naming)
- [GitHub Actions 发布流水线](/marketplace/release-workflow)
- [PR 与版本规则](/marketplace/pr-rules)

仓库中的原始投稿教程位于 [`marketplace/CONTRIBUTING.md`](https://github.com/lvyunqi/QimenBot/blob/main/marketplace/CONTRIBUTING.md)。

## 常见问题

### 页面提示没有兼容版本

展开版本详情查看具体原因。最常见的是 CPU 不一致、Windows/Linux 混用、GNU/musl 混用、glibc 太旧，或插件要求更高的 QimenBot/ABI。

### 页面显示官方 QQ Bot，是否代表所有官方消息都能用

不是。选择具体版本后查看驱动矩阵。`group-at`、`channel-at`、`channel-private` 和 `private` 是不同场景，事件回复、主动发送和富媒体也分别声明。没有出现在矩阵中的能力不能从一个“支持官方 QQ Bot”徽标推断出来。

### Linux 显示动态加载不可用

当前运行的是 musl 静态包。改用同 CPU 的 `*-unknown-linux-gnu` 完整包或官方 Docker 镜像。给 musl 宿主复制 `.so` 不会解决问题。

### 目录刷新失败但还能看到插件

面板正在使用上次成功保存的本地缓存，可以继续查看。安装或关联新插件前必须重新确认最新目录，并访问 GitHub 核对仓库和 Release；无法联网时不会用旧缓存执行安装。卸载和本地历史回滚不依赖目录服务。

### 安装后初始化失败

商城会恢复旧活动文件。到“实时日志”和“插件”页检查第一条 init、配置或依赖错误。插件自己的配置仍位于 `config/plugins/<plugin_id>.toml`。

### Release 中换了文件，为什么安装被拒绝

版本资产的大小或 SHA256 与主目录审核记录不同。同一版本不可覆盖发布，作者应恢复原资产或发布新的 SemVer 并提交新版本文件。
