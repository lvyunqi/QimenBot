#!/usr/bin/env bash
# ── QimenBot Skills Installer ──
# 将 skills/ 目录下的 AI Skill 文件安装到各工具期望的路径。
# 安装后的文件已在 .gitignore 中排除，不会污染项目根目录。

set -e
cd "$(dirname "$0")/.."

echo "Installing QimenBot AI Skills..."

# Claude Code
mkdir -p .claude/commands
cp skills/claude-code.md .claude/commands/plugin.md
echo "  ✓ Claude Code  → .claude/commands/plugin.md"

# Cursor
mkdir -p .cursor/rules
cp skills/cursor.mdc .cursor/rules/plugin-dev.mdc
echo "  ✓ Cursor        → .cursor/rules/plugin-dev.mdc"

# GitHub Copilot
mkdir -p .github/instructions
cp skills/copilot.md .github/copilot-instructions.md
cp skills/copilot-plugin.md .github/instructions/plugin-dev.instructions.md
echo "  ✓ Copilot       → .github/copilot-instructions.md"

# Gemini CLI
cp skills/gemini.md GEMINI.md
echo "  ✓ Gemini CLI    → GEMINI.md"

# Kiro
mkdir -p .kiro/steering
cp skills/kiro.md .kiro/steering/plugin-dev.md
echo "  ✓ Kiro          → .kiro/steering/plugin-dev.md"

# Windsurf
mkdir -p .windsurf/rules
cp skills/windsurf.md .windsurf/rules/plugin-dev.md
echo "  ✓ Windsurf      → .windsurf/rules/plugin-dev.md"

# AGENTS.md (Qodo / 通用标准)
cp skills/agents.md AGENTS.md
echo "  ✓ AGENTS.md     → AGENTS.md"

echo ""
echo "Done! All skills installed."
