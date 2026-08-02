# QimenBot 插件开发 Skill

本目录只维护一份工具无关的插件开发 Skill，不再按不同 AI 工具复制多份内容。

## 目录结构

```text
skills/qimenbot-plugin-development/
├── SKILL.md
└── references/
    ├── static-plugins.md
    ├── dynamic-plugins.md
    └── runtime-and-troubleshooting.md
```

- `SKILL.md`：入口、插件类型选择、开发流程和共同约束。
- `static-plugins.md`：需要 QimenBot 主框架源码的静态插件开发。
- `dynamic-plugins.md`：可在主仓库外独立开发的动态插件、API 0.5、主动发送和 Webhook。
- `runtime-and-troubleshooting.md`：宿主配置、加载流程、官方 QQ Bot、部署和错误诊断。

## 使用方式

让编程工具先完整读取入口：

```text
请读取 skills/qimenbot-plugin-development/SKILL.md，
再根据任务类型完整读取其中指定的 references 文件后开始开发。
```

支持标准 Skill 目录的工具可以直接读取仓库内目录，也可以把整个目录复制到自己的 Skill 根目录：

```bash
cp -R skills/qimenbot-plugin-development "$YOUR_SKILLS_DIR/"
```

```powershell
Copy-Item -Recurse skills/qimenbot-plugin-development $env:YOUR_SKILLS_DIR
```

必须复制整个目录，不能只复制 `SKILL.md`，否则静态、动态和排错引用会丢失。对于不自动发现 Skill 的工具，直接把入口路径写进任务即可，无需维护专用格式副本。

## 维护规则

更新插件能力时只修改这一份 Skill：

1. 先核对当前源码、静态与动态示例、线上文档和 crates.io 实际发布版本。
2. 选型与工作流放在 `SKILL.md`，并保持简短。
3. 静态和动态插件知识不得混写；独立动态插件不能依赖本地主框架 path。
4. 详细内容放在同级 `references/`，引用保持一层深度。
5. 新增动态 API 时同步检查 `templates/dynamic-plugin/`，避免模板落后于 Skill。
6. 验证所有相对链接、代码块、版本说明和示例构建。

权威仓库：<https://github.com/lvyunqi/QimenBot>

在线文档：<https://lvyunqi.github.io/QimenBot/>
