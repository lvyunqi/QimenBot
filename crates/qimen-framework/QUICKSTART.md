# Qimen Framework 快速上手

## 目标

本快速上手文档说明第三方开发者应如何理解当前工作区。

目标心智模型：

- 依赖 framework 层，而不是 official host 可执行程序
- 实现一个 `Module`
- 注册 `CommandPlugin` 和 `SystemPlugin`
- 让 official host 或自定义 host 去加载这些插件

## 推荐 Crates

对于第三方开发，优先从以下位置导入：

- `crates/qimen-framework`
- `crates/qimen-plugin-api`

## 核心概念

### Framework 层

用于：

- runtime 抽象
- message 模型
- protocol 模型
- plugin traits

### Official Host 层

仅作为以下内容的参考：

- 默认配置加载
- 内置模块注册
- 默认进程启动路径

## 插件模型

当前稳定的扩展入口是 `Module`。

一个模块可以暴露：

- `command_plugins()`
- `system_plugins()`
- 通过插件注册协议提供的 `register_plugins()`

命令插件实现：

- `CommandPlugin`
- 用于声明命令定义的 `commands()`

系统事件插件实现：

- `SystemPlugin`
- 稳定的 route enums，例如 `SystemNoticeRoute`、`SystemRequestRoute` 和 `SystemMetaRoute`

## 示例流程

1. 创建一个类似 `plugins/qimen-plugin-example` 的 crate
2. 实现 `Module`
3. 返回 `CommandPlugin` 和/或 `SystemPlugin` 实例
4. 让 host 将该模块注册进 `ModuleRegistry`
5. host 收集插件 bundles 并将其传入 runtime

## 稳定插件注册协议

当前插件注册协议为：

1. 实现 `Module`
2. 通过 `command_plugins()` 和 `system_plugins()` 暴露插件，或重写 `register_plugins()`
3. host 调用 `ModuleRegistry::collect_plugins()`
4. runtime 接收 `PluginBundle`

这样可以让插件收集过程独立于 host 专属的 runtime 组装逻辑。

## 示例能力

### 命令插件

用于：

- 自定义命令回复
- 业务命令路由
- 可复用的 bot 命令包

### 系统插件

用于：

- notice handlers
- 请求批准策略
- 生命周期或心跳响应
- 戳一戳自动回复定制

## 当前示例 Crate

见：

- `plugins/qimen-plugin-example/src/lib.rs`

它演示了：

- 一个同时导出命令插件和系统插件的模块
- 一个自定义 `echo` 的命令插件
- 一个响应 poke notice 事件的系统插件

## 对第三方作者的下一步建议

如果你想构建可复用的插件包，先从复制以下目录结构开始：

- `plugins/qimen-plugin-example/`

然后用你自己的 handler 替换示例命令/系统逻辑。
