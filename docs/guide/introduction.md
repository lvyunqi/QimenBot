# 框架介绍

## 什么是 QimenBot？

QimenBot 是一个用 **Rust** 编写的模块化、可扩展的聊天机器人框架。它基于 [OneBot](https://github.com/botuniverse/onebot-11) 协议，可以对接 QQ、微信等多种聊天平台。

与传统的 Bot 框架不同，QimenBot 将**可复用的框架层**与**参考 Host 实现**分离——你既可以直接部署官方 Host，也可以基于框架层构建自己的 Bot 平台。

## 为什么选择 QimenBot？

### 性能卓越

QimenBot 基于 Rust + [Tokio](https://tokio.rs/) 异步运行时构建：

- **零成本抽象** — Rust 的所有权系统在编译期消除了运行时开销
- **无 GC 停顿** — 不像 Java/Go，没有垃圾回收导致的延迟抖动
- **异步 I/O** — 基于 Tokio 的全异步架构，高并发场景下性能出色
- **编译期优化** — 泛型单态化、内联优化，运行时效率极高

### 开发简单

通过过程宏（proc macro），你只需要写几行代码就能完成一个功能完整的插件：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "my-plugin")]
#[commands]
impl MyPlugin {
    #[command("回复 pong")]
    async fn ping(&self) -> &str {
        "pong!"
    }
}
```

就这么简单——`#[module]` 声明插件，`#[command]` 定义命令，框架自动处理注册、路由、权限等一切细节。

### 架构灵活

- **模块化设计** — 每个功能都是独立模块，按需加载
- **多 Bot 实例** — 一个进程可以管理多个 Bot，各自独立配置
- **热重载** — 动态插件修改后无需重启，发送 `/plugins reload` 即可生效
- **拦截器链** — 在事件到达插件之前进行预处理（如权限校验、频率限制）

## 核心概念速览

| 概念 | 说明 |
|------|------|
| **Module（模块）** | 插件的最小单元，包含命令、事件处理器和拦截器 |
| **Command（命令）** | 用户通过 `/命令名` 触发的交互操作 |
| **Notice / Request / Meta** | OneBot 协议中的系统事件（通知、请求、元事件） |
| **Interceptor（拦截器）** | 在事件处理前后执行的钩子函数 |
| **Message（消息）** | 支持文本、图片、@、表情等多种段类型的富媒体消息 |
| **Transport（传输层）** | WebSocket / HTTP 等底层通信方式 |
| **Adapter（适配器）** | 将协议特定格式转换为框架统一格式 |

## 两种插件模式

QimenBot 支持两种插件开发方式：

### 静态插件

与框架一起编译，可以使用框架的全部 API（包括异步操作）：

- 完整的 `async/await` 支持
- 直接调用 `OneBotActionClient` 进行 API 操作
- 访问完整的 `Message` 类型系统
- 适合核心功能和需要复杂逻辑的场景

### 动态插件

编译为独立的动态库（`.so` / `.dll` / `.dylib`），运行时加载：

- 独立编译，不依赖主工作空间
- 支持热重载，修改后无需重启进程
- 使用 C ABI 的 FFI 接口
- 适合第三方扩展和快速迭代的场景

详细对比请参阅 [动态插件开发](/plugin/dynamic)。

## 协议支持

| 协议 | 状态 | 传输模式 |
|------|------|----------|
| OneBot 11 | ✅ 生产就绪 | WS 正向、WS 反向、HTTP API、HTTP POST |
| OneBot 12 | 🔲 计划中 | — |
| Satori | 🔲 计划中 | — |

## 致谢

QimenBot 的设计参考了以下优秀项目：

- [Shiro](https://github.com/MisakaTAT/Shiro) — 基于 Java 的 OneBot 框架，拦截器与插件模型的灵感来源
- [Kovi](https://github.com/ThriceCola/Kovi) — Rust OneBot 框架，简洁 API 设计的参考
