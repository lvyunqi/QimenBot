---
layout: home

hero:
  name: QimenBot
  text: 基于 Rust 的高性能多协议 Bot 框架
  tagline: 模块化、可扩展、声明式插件开发，让 Bot 开发变得简单
  image:
    src: /logo.jpg
    alt: QimenBot
  actions:
    - theme: brand
      text: 快速开始
      link: /guide/getting-started
    - theme: alt
      text: 插件开发
      link: /plugin/overview
    - theme: alt
      text: GitHub
      link: https://github.com/lvyunqi/QimenBot

features:
  - icon: 🚀
    title: 高性能
    details: 基于 Rust + Tokio 异步运行时构建，零成本抽象，高吞吐低延迟。编译即优化，无 GC 停顿。
  - icon: 🔌
    title: 声明式插件
    details: 通过 #[module] / #[command] / #[notice] 等宏，最少 7 行代码即可完成一个功能完整的插件。
  - icon: 🔄
    title: 动态插件 & 热重载
    details: 支持将插件编译为 .so/.dll/.dylib 动态库，运行时通过 /plugins reload 热重载，无需重启。
  - icon: 🌐
    title: 多协议支持
    details: OneBot 11 生产就绪，OneBot 12 / Satori 预留扩展点。支持正向/反向 WebSocket 和 HTTP。
  - icon: 🛡️
    title: 运行时保护
    details: 内置令牌桶限流、消息去重、群事件过滤、插件 ACL、熔断器保护，保障 Bot 稳定运行。
  - icon: 📦
    title: 模块化架构
    details: 框架层与 Host 层分离，可独立复用。内置命令、管理、调度、桥接四大核心模块。
---
