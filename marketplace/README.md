# QimenBot 插件商城目录

这个目录是 QimenBot 插件商城的唯一登记源。商城不托管第三方插件二进制，也不 checkout、构建或运行投稿仓库中的代码。

每个插件使用一个永久不变的 `plugin_id`，每个发布版本使用一份不可修改的版本文件：

```text
marketplace/
├── plugins/
│   └── <plugin-id>/
│       ├── plugin.toml
│       └── versions/
│           ├── 1.0.0.toml
│           └── 1.1.0.toml
├── examples/
│   └── release.yml
├── schemas/
├── templates/
├── DRIVER_COMPATIBILITY.md
├── ARTIFACT_NAMING.md
├── REPOSITORY_RULES.md
└── PR_RULES.md
```

插件源码和 Release 资产放在插件作者自己的公开 GitHub 仓库中。主仓库只保存以下内容：

- 插件身份、仓库数字 ID、开源许可证和介绍；
- 各版本兼容的 OneBot 11 / 官方 QQ Bot 场景、事件和发送能力；
- QimenBot 范围、动态 ABI、目标平台和最低 glibc；
- 经过审核的 Release 资产名称、大小和 SHA256；
- 数据版本以及能否安全回滚。

`qimen-marketplace-index` 会验证这些文件并生成 `docs/public/marketplace/index.json`。商城 PR 合并到本仓库的 `main` 后，GitHub Pages 流水线会重新生成并发布索引，Web 管理面板随后自动读取新内容。发布通常需要一到几分钟，不是严格实时，也不需要另建文档仓库或发布新的 QimenBot 版本。

## 从哪里开始

第一次投稿按下面的顺序阅读：

1. [投稿教程](CONTRIBUTING.md)：从模板建立目录并运行本地检查。
2. [开源仓库规范](REPOSITORY_RULES.md)：确认仓库、许可证、README、依赖和权限说明符合要求。
3. [驱动兼容声明](DRIVER_COMPATIBILITY.md)：区分 OneBot 11 普通消息和官方 QQ Bot 消息场景。
4. [构建产物命名](ARTIFACT_NAMING.md)：按插件 ID 和完整 Rust target 命名 Release 动态库。
5. [商城 PR 规则](PR_RULES.md)：准备首次收录、新版本或撤回版本的 PR。
6. [GitHub Actions 示例](examples/release.yml)：构建六个支持的 target、生成 SHA256 和 Artifact Attestation。

静态插件也可以登记源码和兼容范围，但不能提供动态库资产，管理面板不会为它执行在线安装。
