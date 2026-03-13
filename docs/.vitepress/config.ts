import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'QimenBot',
  description: '基于 Rust 的高性能多协议 Bot 框架',
  lang: 'zh-CN',
  base: '/QimenBot/',

  head: [
    ['link', { rel: 'icon', type: 'image/jpeg', href: '/QimenBot/logo.jpg' }],
    ['meta', { name: 'theme-color', content: '#7c3aed' }],
    ['meta', { name: 'og:type', content: 'website' }],
    ['meta', { name: 'og:title', content: 'QimenBot' }],
    ['meta', { name: 'og:description', content: '基于 Rust 的高性能多协议 Bot 框架' }],
  ],

  themeConfig: {
    logo: '/logo.jpg',
    siteTitle: 'QimenBot',

    nav: [
      { text: '指南', link: '/guide/introduction', activeMatch: '/guide/' },
      { text: '插件开发', link: '/plugin/overview', activeMatch: '/plugin/' },
      { text: 'API 参考', link: '/api/plugin-api', activeMatch: '/api/' },
      { text: '进阶', link: '/advanced/runtime', activeMatch: '/advanced/' },
      {
        text: '相关链接',
        items: [
          { text: 'GitHub', link: 'https://github.com/lvyunqi/QimenBot' },
          { text: 'OneBot 11 协议', link: 'https://github.com/botuniverse/onebot-11' },
          { text: '更新日志', link: '/changelog' },
        ]
      }
    ],

    sidebar: {
      '/guide/': [
        {
          text: '入门',
          items: [
            { text: '框架介绍', link: '/guide/introduction' },
            { text: '快速开始', link: '/guide/getting-started' },
            { text: '配置详解', link: '/guide/configuration' },
          ]
        },
        {
          text: '核心概念',
          items: [
            { text: '架构设计', link: '/guide/architecture' },
            { text: '事件处理流程', link: '/guide/event-flow' },
          ]
        }
      ],
      '/plugin/': [
        {
          text: '开始',
          items: [
            { text: '插件开发概览', link: '/plugin/overview' },
          ]
        },
        {
          text: '静态插件开发',
          items: [
            { text: '命令开发', link: '/plugin/commands' },
            { text: '消息构建', link: '/plugin/messages' },
            { text: '事件处理', link: '/plugin/events' },
            { text: '拦截器', link: '/plugin/interceptors' },
          ]
        },
        {
          text: '动态插件开发',
          collapsed: false,
          items: [
            { text: '动态插件教程', link: '/plugin/dynamic' },
            { text: '快速开始', link: '/plugin/dynamic#quickstart' },
            { text: '宏详解', link: '/plugin/dynamic#macro' },
            { text: 'CommandRequest', link: '/plugin/dynamic#command-request' },
            { text: 'CommandResponse', link: '/plugin/dynamic#command-response' },
            { text: '拦截器', link: '/plugin/dynamic#pre-handle' },
            { text: '插件配置', link: '/plugin/dynamic#config' },
            { text: '完整示例', link: '/plugin/dynamic#full-example' },
            { text: '手动 FFI 写法', link: '/plugin/dynamic#manual-ffi' },
            { text: '运行时管理', link: '/plugin/dynamic#runtime' },
          ]
        }
      ],
      '/api/': [
        {
          text: 'API 参考',
          items: [
            { text: '插件 API', link: '/api/plugin-api' },
            { text: '消息 API', link: '/api/message-api' },
            { text: 'OneBot API 客户端', link: '/api/onebot-client' },
            { text: 'FFI 接口', link: '/api/ffi-api' },
            { text: '类型参考', link: '/api/types' },
          ]
        }
      ],
      '/advanced/': [
        {
          text: '进阶',
          items: [
            { text: '运行时原理', link: '/advanced/runtime' },
            { text: '传输层', link: '/advanced/transport' },
            { text: '部署指南', link: '/advanced/deployment' },
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/lvyunqi/QimenBot' }
    ],

    footer: {
      message: '基于 MIT 许可证发布',
      copyright: 'Copyright © 2026-present QimenBot Contributors'
    },

    search: {
      provider: 'local',
      options: {
        translations: {
          button: { buttonText: '搜索文档', buttonAriaLabel: '搜索文档' },
          modal: {
            noResultsText: '无法找到相关结果',
            resetButtonTitle: '清除查询条件',
            footer: { selectText: '选择', navigateText: '切换', closeText: '关闭' }
          }
        }
      }
    },

    outline: {
      label: '页面导航',
      level: [2, 3]
    },

    lastUpdated: {
      text: '最后更新于'
    },

    editLink: {
      pattern: 'https://github.com/lvyunqi/QimenBot/edit/main/docs/:path',
      text: '在 GitHub 上编辑此页面'
    },

    docFooter: {
      prev: '上一页',
      next: '下一页'
    },

    returnToTopLabel: '回到顶部',
    sidebarMenuLabel: '菜单',
    darkModeSwitchLabel: '主题',
    lightModeSwitchTitle: '切换到浅色模式',
    darkModeSwitchTitle: '切换到深色模式',
  },

  lastUpdated: true,

  markdown: {
    lineNumbers: true,
    theme: {
      light: 'github-light',
      dark: 'one-dark-pro'
    }
  },

  sitemap: {
    hostname: 'https://lvyunqi.github.io/QimenBot/'
  }
})
