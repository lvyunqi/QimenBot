# Skill 安装与 Vibe Coding

QimenBot 插件可以用 AI 辅助开发。你可以先安装 QimenBot 插件开发 Skill，再用自然语言描述目标，让 AI 帮你选择插件类型、搭建目录、编写代码、补测试和排查编译日志。这种方式通常被称为 **Vibe Coding**。

这里的 Skill 是给 AI 编程助手看的开发手册，不是 QimenBot 的运行时依赖，也不会被打包进插件。它把当前仓库的静态插件、动态插件、crates.io 依赖、官方 QQ Bot、OneBot 11、在线配置、Webhook 和插件商城规则放在一起，减少“凭记忆猜 API”的情况。

::: tip 适合哪些人？
即使没有 Rust 或 QimenBot 主框架源码，也可以从动态插件开始。Skill 会说明需要哪些依赖、哪些 API 已发布、动态库应该放到哪里，以及如何根据日志继续修改。拥有主仓库源码、需要完整异步能力时，再选择静态插件。
:::

## 安装 Skill

### Codex（推荐）

安装脚本从 GitHub 仓库读取 `skills/qimenbot-plugin-development`，并放到 Codex 的本地 Skill 目录。先确认电脑已安装 Python 和 Git，然后在终端执行。

Windows PowerShell：

```powershell
python "$env:USERPROFILE\.codex\skills\.system\skill-installer\scripts\install-skill-from-github.py" `
  --repo lvyunqi/QimenBot `
  --path skills/qimenbot-plugin-development
```

macOS 或 Linux：

```bash
python ~/.codex/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo lvyunqi/QimenBot \
  --path skills/qimenbot-plugin-development
```

安装完成后，关闭并重新打开 Codex 会话，让 Skill 重新索引。不同版本的 Codex 可能把目录放在 `~/.codex/skills` 或由 `CODEX_HOME` 指定的位置；以安装脚本输出的目标路径为准。

### 检查是否安装成功

Windows 可以执行：

```powershell
Get-ChildItem "$env:USERPROFILE\.codex\skills\qimenbot-plugin-development"
```

macOS 或 Linux 可以执行：

```bash
ls ~/.codex/skills/qimenbot-plugin-development
```

目录中至少应有 `SKILL.md` 和 `references/`。新开一个对话后，可以直接问 AI：“请读取 QimenBot 插件开发 Skill，并根据我的目标判断使用静态还是动态插件。”如果它能引用当前 API、示例目录和构建约束，说明 Skill 已被识别。

### 其他 AI 编程工具

不同工具对 Skill 的目录命名和自动加载方式并不相同，不能把 Codex 的命令直接套用到所有工具。通用做法是：

1. 从 [QimenBot 仓库](https://github.com/lvyunqi/QimenBot) 获取 `skills/qimenbot-plugin-development` 整个目录。
2. 按所用工具的官方说明，将目录放入它的 skills、rules 或 instructions 目录。
3. 确认工具至少读取了 `SKILL.md`；如果它支持引用文件，再一并加载 `references/` 下与任务相关的文档。

如果工具不支持 Skill，可以把 `SKILL.md` 和对应参考文档作为项目上下文提供给 AI，效果仍然比只贴一段需求更稳定。安装 Skill 不会修改 QimenBot 配置，也不会替你上传代码。

## Vibe Coding 怎么用

Vibe Coding 的重点是“先描述目标，再让 AI 快速迭代”，而不是把生成结果直接当成可发布软件。推荐把一次需求拆成小步，每一步都让 AI 给出变更、运行检查并说明没有验证的部分。

```text
明确插件目标
  -> 选择静态或动态插件
  -> 让 AI 读取 Skill 和对应示例
  -> 生成最小项目
  -> 编译、测试并检查产物
  -> 部署或重新扫描插件
  -> 在 OneBot 11 或官方 QQ Bot 中实测
  -> 根据日志继续迭代
```

### 给 AI 的第一条提示词

可以直接复制下面的模板，再填入最后几行：

```text
你是 QimenBot 插件开发助手。
请先读取 QimenBot 插件开发 Skill，并根据我的目标判断使用静态插件还是动态插件。
不要假设不存在的宏、字段或协议能力；先查当前示例和已发布的 crates.io 版本。
先给出目录、依赖、命令/事件/配置设计，再生成代码。
完成后运行 cargo fmt、cargo check、cargo test，并说明没有实际验证的部分。

插件目标：
运行协议（OneBot 11 / 官方 QQ Bot / 两者）：
是否拥有 QimenBot 主框架源码（是 / 否）：
目标系统和架构（例如 x86_64 Linux GNU、aarch64 Linux GNU、Windows MSVC）：
是否需要主动推送、Webhook 或在线配置：
```

之后可以继续用日志驱动修改，例如：“这是 `cargo check` 的完整错误，请只修复导致错误的代码，并保留现有插件 ID。”每次只处理一类问题，比较容易定位回归。

## 先选插件类型

| 需求 | 静态插件 | 动态插件 |
| --- | --- | --- |
| 是否需要 QimenBot 主框架源码 | 需要 | 不需要，可独立仓库开发 |
| 运行形式 | 编译进宿主二进制 | `.so` / `.dll` / `.dylib`，运行时扫描 |
| 编程模型 | 完整 Rust、`async`、`OneBotActionClient` | 稳定 FFI、同步回调、`BotApi` / `SendBuilder` |
| 迭代方式 | 改代码后重新构建宿主 | Web 插件页重新扫描，可热重载 |
| 适合场景 | 核心能力、复杂异步流程、直接调用 OneBot Action | 第三方发布、快速试错、没有主框架源码 |

静态插件的完整示例在 [`plugins/qimen-plugin-example`](https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-plugin-example)，动态插件的完整示例在 [`plugins/qimen-dynamic-plugin-example`](https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example)。动态插件也可以从[独立模板](https://github.com/lvyunqi/QimenBot/tree/main/templates/dynamic-plugin)开始。

## 构建和验收

### 静态插件

静态插件属于主仓库 workspace，在仓库根目录执行：

```bash
cargo fmt --check
cargo check
cargo test
cargo build --release
```

它需要在宿主构建时链接，并在 `apps/qimenbotd/src/main.rs` 中保留插件注册引用。修改后要确认 `config/base.toml` 中的模块 ID 已启用。

### 动态插件

动态插件是独立 workspace，先进入插件自己的目录再执行：

```bash
cargo fmt --check
cargo check
cargo test
cargo build --release
```

将生成的动态库复制到宿主配置的 `plugin_bin_dir`（默认是 `plugins/bin/`），然后在 Web 插件页重新扫描。文件名、CPU 架构、操作系统和 C 运行时必须与宿主匹配；`musl` 构建包不能加载依赖 glibc 的动态插件。发布前请确认产物名称包含完整 target，并记录 SHA256。

当前动态插件依赖版本为：

```toml
abi-stable-host-api = "0.1.13"
qimen-dynamic-plugin-derive = "0.1.13"
```

API 0.6 插件应显式声明：

```rust
#[dynamic_plugin(id = "my-plugin", version = "0.1.0", api = "0.6")]
```

API 0.6 包含在线配置能力；宿主也必须是支持该 API 的版本。不要把 Git 依赖、未发布的本地 path 依赖或不同版本的 ABI crate 混入发布包。

## 让 AI 写代码前后的检查

AI 能显著加快样板代码和重复劳动，但下面几项必须由开发者确认：

- 不要把 `QQBOT_SECRET`、Token、数据库文件或生产配置粘贴到提示词、Issue 和源码中。
- 检查命令前缀、免前缀、是否必须 `@`、群聊/私聊作用域；这些由宿主配置和协议适配器共同决定。
- 官方 QQ Bot 的 `openid` 等 ID 按字符串处理，不要强转成 QQ 数字 ID。
- 图片、视频、Markdown 等富媒体先确认目标协议的发送能力，不要让插件绕过宿主自行读取 Bot 凭证或实现上传流程。
- 不要盲目接受 AI 编造的宏、字段或版本号；以当前示例、API 文档和 crates.io 发布版本为准。
- 动态库要核对 target、GNU/musl、glibc、CPU 架构和宿主版本，避免“本机能编译、服务器无法加载”。
- 实际执行构建、宿主重载和协议测试；只通过 `cargo check` 不代表消息链路已经可用。

## 发布到插件商城

Vibe Coding 生成的插件同样要遵守商城规则：代码必须开源，仓库需要有许可证、README、支持的驱动器声明和可复现的 GitHub Actions 构建；Release 资产名称、target、版本和 SHA256 必须一致。提交前请阅读[商城规范](/marketplace/)、[PR 与版本规则](/marketplace/pr-rules)和[构建产物命名](/marketplace/artifact-naming)。

建议先让 AI 生成一个最小可用版本并完成真实测试，再逐步增加功能。插件 ID 一旦发布不要随意更换，配置文件和升级索引都会依赖它。

## 参考入口

- [插件开发概览](/plugin/overview)
- [动态插件教程](/plugin/dynamic)
- [在线配置 API 0.6](/advanced/dynamic-config-v06)
- [官方 QQ Bot Markdown 文档](https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html)
- [QimenBot GitHub 仓库](https://github.com/lvyunqi/QimenBot)
- [QimenBot 文档站](https://lvyunqi.github.io/QimenBot/)
