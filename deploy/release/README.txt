QimenBot 使用说明
=================

普通用户只需要运行根目录下的 qimenbot（Windows 为 qimenbot.exe）。

首次启动前：
1. 把 config/base.toml.example 复制为 config/base.toml。
2. 把 config/qimenbot.toml.example 复制为 config/qimenbot.toml。
3. 运行 qimenbot run。
4. 打开 http://127.0.0.1:3210/ 完成机器人、模块和插件配置。

runtime/qimenbotd 是内部核心程序，由 qimenbot 自动管理，请勿单独启动、移动或改名。

动态插件注意事项：
- 插件必须与 QimenBot 使用相同的操作系统、CPU 架构和构建 target。
- Docker amd64/arm64 分别使用 x86_64/aarch64 的 unknown-linux-gnu 插件。
- x86_64-unknown-linux-musl 包是静态程序，不支持加载 .so 动态插件；需要动态插件时请使用 GNU 包或 Docker。

完整教程：https://lvyunqi.github.io/QimenBot/guide/deployment.html
