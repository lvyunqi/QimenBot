# QimenBot Plugin Development Skills

本目录提供适配各种 AI 编程工具的插件开发 Skill，帮助 AI 快速理解 QimenBot 并辅助插件开发。

## 文件说明

| 文件 | 适配工具 | 安装方式 |
|------|---------|---------|
| `plugin-dev.md` | 通用参考 / 任何 AI | 直接阅读或喂给 AI |
| `claude-code.md` | Claude Code | 复制到 `.claude/commands/plugin.md` |
| `cursor.mdc` | Cursor | 复制到 `.cursor/rules/plugin-dev.mdc` |
| `copilot.md` | GitHub Copilot | 复制到 `.github/copilot-instructions.md` |
| `gemini.md` | Gemini CLI | 复制到项目根目录 `GEMINI.md` |
| `kiro.md` | Kiro | 复制到 `.kiro/steering/plugin-dev.md` |
| `windsurf.md` | Windsurf | 复制到 `.windsurf/rules/plugin-dev.md` |
| `agents.md` | Qodo / 通用 AGENTS.md 标准 | 复制到项目根目录 `AGENTS.md` |

## 快速安装

### 一键安装所有（Unix/macOS/Git Bash）

```bash
# 在项目根目录执行
bash skills/install.sh
```

### 手动安装单个工具

```bash
# Claude Code
mkdir -p .claude/commands && cp skills/claude-code.md .claude/commands/plugin.md

# Cursor
mkdir -p .cursor/rules && cp skills/cursor.mdc .cursor/rules/plugin-dev.mdc

# GitHub Copilot
cp skills/copilot.md .github/copilot-instructions.md

# Gemini CLI
cp skills/gemini.md GEMINI.md

# Kiro
mkdir -p .kiro/steering && cp skills/kiro.md .kiro/steering/plugin-dev.md

# Windsurf
mkdir -p .windsurf/rules && cp skills/windsurf.md .windsurf/rules/plugin-dev.md

# Qodo / AGENTS.md
cp skills/agents.md AGENTS.md
```

## 更新 Skill

编辑 `skills/` 目录下的源文件，然后重新执行安装脚本即可同步到各工具。
