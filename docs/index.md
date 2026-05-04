---
layout: home

hero:
  name: QimenBot
  text: 高性能多协议 Bot 框架
  tagline: 基于 Rust 构建，宏驱动开发，约 7 行代码即可编写一个插件
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
    title: 极致性能
    details: Rust 原生异步运行时，零成本抽象，轻松应对高并发消息处理场景
  - icon: 🔌
    title: 插件化架构
    details: '通过 #[module] + #[commands] 宏，只需几行代码就能开发功能完整的插件'
  - icon: 🌐
    title: 多协议支持
    details: 支持 OneBot 11 和官方 QQ Bot，WebSocket / HTTP / Gateway 多种传输方式可选
  - icon: 🛡️
    title: 安全防护
    details: 内置消息去重、速率限制、群事件过滤、插件 ACL 权限控制等保护机制
  - icon: 🎛️
    title: 拦截器链
    details: pre_handle / after_completion 双阶段拦截，灵活实现黑名单、冷却、日志等功能
  - icon: 📦
    title: 动态插件
    details: 支持通过 FFI 加载动态链接库插件，无需重新编译即可扩展功能
---

## 一个简单的例子

只需 **7 行代码**，就能创建一个完整的 Bot 插件：

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
前往 [快速开始](/guide/getting-started) 了解如何搭建你的第一个 Bot，或者直接查看 [插件开发指南](/plugin/overview) 开始编写插件。
:::
