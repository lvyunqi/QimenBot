---
layout: home

hero:
  name: QimenBot
  text: Rust 多协议 Bot 框架
  tagline: 支持静态与动态插件、OneBot 11 和官方 QQ Bot
  image:
    src: /logo.jpg
    alt: QimenBot Logo
  actions:
    - theme: brand
      text: 快速开始 →
      link: /guide/getting-started
    - theme: alt
      text: 在 GitHub 上查看
      link: https://github.com/lvyunqi/QimenBot
    - theme: alt
      text: 加入 QQ 群 835684778
      link: https://qun.qq.com/universal-share/share?ac=1&authKey=0sFE1a6DbXGo70vp3VpylxRQ8AmXY%2BgpIAbrB4Cgf9qjT634oSVcrHDWptDNP3%2Fq&busi_data=eyJncm91cENvZGUiOiI4MzU2ODQ3NzgiLCJ0b2tlbiI6IitmMTBOWS96UXQ2Tk9nakgrOWZFMElPL0VXcFJnNmp0c0NSS0tpK25wY24xNEpGV2MvdjY1c2VBL3ArM09TQngiLCJ1aW4iOiI0MzQ2NTgxOTgifQ%3D%3D&data=EJZhsrc7rxEPVPxGeDybFi7TfocR3lNIFijyePfdpsQTTzNNnqoiMvuahA0t8HoN8DVZR9aKBCKcTxDKmOb8IQ&svctype=4&tempid=h5_group_info

features:
  - icon: 🚀
    title: 并发运行时
    details: Rust 原生异步运行时，用于并发处理消息和后台任务
  - icon: 🔌
    title: 插件化架构
    details: '通过 #[module] + #[commands] 宏生成插件注册和命令路由代码'
  - icon: 🌐
    title: 多协议支持
    details: 支持 OneBot 11 和官方 QQ Bot，WebSocket / HTTP / Gateway 多种传输方式可选
  - icon: 🛡️
    title: 安全防护
    details: 内置消息去重、速率限制、群事件过滤、插件 ACL 权限控制等保护机制
  - icon: 🎛️
    title: 拦截器链
    details: pre_handle / after_completion 双阶段拦截，用于黑名单、冷却和日志等功能
  - icon: 📦
    title: 动态插件
    details: 支持通过 FFI 加载动态链接库插件，无需重新编译即可扩展功能
---

## 一个简单的例子

以下代码是一个最小 Bot 插件示例：

```rust
use qimen_plugin_api::prelude::*;

#[module(id = "hello", version = "0.1.0")]
#[commands]
impl HelloPlugin {
    #[command("向你打招呼")]
    async fn hello(&self) -> &str {
        "Hello from QimenBot! 🎉"
    }
}
```

发送 `/hello` 或 `hello`，Bot 就会回复 `Hello from QimenBot! 🎉`

::: tip 想了解更多？
通过 [快速开始](/guide/getting-started) 配置 Bot 实例，或查看 [插件开发指南](/plugin/overview) 编写插件。
:::
