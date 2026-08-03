# 插件商城规范

QimenBot 插件商城使用 GitHub 完成源码公开、版本发布和目录审核，不需要插件作者部署商城服务器。QimenBot 主仓库只保存经过审核的索引数据，插件源码和二进制始终留在作者自己的公开仓库。

这组文档面向准备投稿的插件作者。只想安装插件的管理员请阅读[插件商城使用教程](/plugin/marketplace)。

## 商城保存什么

一个商城条目由三部分组成：

| 位置 | 内容 | 维护者 |
|---|---|---|
| QimenBot 主仓库 `marketplace/` | 插件身份、版本、驱动兼容、资产大小和 SHA256 | QimenBot 维护者审核 PR |
| 插件公开仓库 | 完整源码、许可证、README、配置和构建流程 | 插件作者 |
| 插件 GitHub Release | 按 target 命名的 DLL、SO、dylib 和构建证明 | 插件作者 |

主仓库不接收第三方动态库、压缩包、配置、数据库或用户数据。商城流水线也不会 checkout、构建或执行投稿插件，只读取目录 TOML 和 GitHub 公开元数据。

## 动态插件和静态插件

| 类型 | 商城能力 | 用户如何安装 |
|---|---|---|
| 动态插件 | 展示、在线安装、更新、回滚、卸载 | 管理面板下载与宿主 target 完全匹配的动态库并热加载 |
| 静态插件 | 展示源码、版本和驱动兼容范围 | 用户把插件加入 QimenBot 源码后重新构建 `qimenbotd` |

静态插件不能提供 `dynamic_api` 或 `assets`。两种插件都必须按版本声明 `drivers`，让用户知道它支持 OneBot 11 普通消息还是官方 QQ Bot 消息，以及具体支持哪些场景。

## 一次完整投稿

```text
准备公开仓库和许可证
  -> 确定稳定 plugin_id
  -> 编写并测试插件
  -> 声明版本级驱动兼容
  -> GitHub Actions 构建各 target
  -> 按固定名称发布 Release
  -> 计算字节数和 SHA256
  -> 在 QimenBot marketplace/ 新增 TOML
  -> 本地校验和 GitHub 元数据校验
  -> 提交商城 PR
```

Release 必须先发布，商城 PR 后提交。审核需要核对真实资产名称、大小、SHA256 和可选 Artifact Attestation，不能先放空值等待合并后补齐。

## 目录结构

插件 ID 为 `group-tools`、版本为 `1.0.0` 时：

```text
marketplace/plugins/group-tools/
├── plugin.toml
└── versions/
    └── 1.0.0.toml
```

`plugin.toml` 保存不会随版本变化的身份信息。`versions/1.0.0.toml` 保存这个版本的 QimenBot 范围、动态 ABI、驱动兼容、Release tag、目标平台和资产校验值。

同一个插件可以持续新增 `1.1.0.toml`、`1.2.0.toml`。已经合并的历史版本不可原地修改，修复时必须发布新的 SemVer。

## 阅读顺序

1. [开源仓库规范](/marketplace/repository-rules)：先确认项目具备可审查、可独立构建的公开来源。
2. [驱动兼容声明](/marketplace/driver-compatibility)：准确区分 OneBot 11 与官方 QQ Bot 的消息入口。
3. [构建产物命名](/marketplace/artifact-naming)：确定每个 target 的唯一 Release 文件名。
4. [发布流水线](/marketplace/release-workflow)：构建六个平台资产并生成 SHA256 与证明。
5. [PR 与版本规则](/marketplace/pr-rules)：创建目录文件、运行校验并提交审核。

仓库内的原始模板和规则文件位于 [`marketplace/`](https://github.com/lvyunqi/QimenBot/tree/main/marketplace)。线上文档用于阅读，投稿时以目标分支中的 Schema、模板和校验程序为准。

## 先记住四条

- `plugin_id` 是永久身份，发布后不要更换或转给另一个项目。
- 驱动能力按版本填写，只写实际测试过的场景。
- 二进制必须匹配宿主 OS、CPU 和 C 运行时，musl 包不能加载动态插件。
- 同一版本的 Release 资产不可替换，任何修复都发布新版本。

商城收录不等于安全认证。动态插件与 QimenBot 在同一进程中运行，可以访问宿主账号有权访问的文件和网络；公开源码、SHA256 和构建证明分别解决可审查、完整性和来源问题，不能提供运行时沙箱。
