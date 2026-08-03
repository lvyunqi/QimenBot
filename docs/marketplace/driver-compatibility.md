# 驱动兼容声明

商城版本文件中的 `drivers` 用来回答两个问题：这个版本能处理哪一种机器人接入，以及在该接入下哪些消息场景和发送能力已经验证。

兼容信息放在 `versions/<version>.toml`，不放在 `plugin.toml`。例如 `1.0.0` 只支持 OneBot 11，`1.1.0` 增加官方 QQ Bot 时，两份历史记录可以同时保持准确。

## 两种驱动

| 值 | 页面显示 | 说明 |
|---|---|---|
| `onebot11` | OneBot 11 | OneBot 11 标准或实现扩展经过 QimenBot 适配后的事件和发送 |
| `qq-official` | 官方 QQ Bot | QQ 开放平台 Gateway 事件与 OpenAPI 发送 |

当前 Schema 只接受这两个已经投入使用的驱动。以后框架增加新协议时，会先更新 Schema、运行时和商城页面，再开放新的枚举值。

官方 QQ Bot 不是 OneBot 11 的另一种连接地址。它使用字符串 openid、官方 Intents、不同的消息事件和发送限制。插件共用处理函数不代表可以省略驱动测试。

## `scenes` 消息场景

每个驱动至少填写一个场景：

| 值 | 用户看到的含义 | 使用说明 |
|---|---|---|
| `private` | 私聊 | OneBot 私聊，或官方 QQ 单聊/C2C |
| `group` | 群聊 | OneBot 普通群消息 |
| `group-at` | 群内 @ | 官方 QQ 群内 @ 机器人事件 |
| `channel` | 频道消息 | OneBot 实现提供的频道普通消息扩展 |
| `channel-at` | 频道 @ | 官方 QQ 频道内 @ 机器人事件 |
| `channel-private` | 频道私信 | OneBot 频道私信扩展或官方 QQ 频道私信 |

OneBot 11 项不能填写 `group-at`、`channel-at`。这两个名称特意保留给官方 QQ Bot，避免商城只显示“支持群聊”，却不说明机器人只能在被 @ 时收到消息。

OneBot 的 `channel`、`channel-private` 往往属于实现扩展，不是每个 OneBot 服务端都具备。README 应写明测试过的服务端和必要配置。

## `events` 事件种类

| 值 | 包含内容 |
|---|---|
| `message` | 消息和命令 |
| `notice` | 成员变化、撤回、互动等通知 |
| `request` | 好友、加群、邀请等请求 |
| `meta` | 心跳、生命周期等元事件 |

有 `#[command]` 或消息路由时通常需要 `message`。只处理通知的插件可以写 `notice`，不要为了让列表看起来完整而把四项全部填上。

## `outbound` 发送能力

| 值 | 含义 |
|---|---|
| `reply` | 对当前事件进行回复 |
| `proactive` | 后台任务、Webhook 等入口主动发送 |
| `rich-message` | 发送图片、@、表情或其他非纯文本消息段 |

`rich-message` 必须和 `reply` 或 `proactive` 一起出现。富媒体能力也要逐驱动测试，例如 OneBot 可以发送某个消息段，不代表官方 QQ OpenAPI 接受相同参数。

每个驱动必须至少有一项 `events` 或 `outbound`。省略其中一个数组时按空数组处理。

## 只支持 OneBot 11

```toml
[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message", "notice", "request"]
outbound = ["reply", "proactive", "rich-message"]
```

## 只支持官方 QQ Bot

```toml
[[drivers]]
driver = "qq-official"
scenes = ["private", "group-at", "channel-at", "channel-private"]
events = ["message", "notice"]
outbound = ["reply", "proactive", "rich-message"]
```

`private` 在这里表示官方开放平台的单聊/C2C。代码应把 openid 和目标 ID 作为字符串保存，不要强转为传统 QQ 数字。

## 同时支持两种驱动

```toml
[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message"]
outbound = ["reply"]

[[drivers]]
driver = "qq-official"
scenes = ["private", "group-at"]
events = ["message"]
outbound = ["reply"]
```

一个版本最多各写一次 `onebot11` 和 `qq-official`。相同驱动拆成多个 `[[drivers]]` 会被拒绝。

## 页面如何使用这组数据

管理面板会在插件列表显示驱动徽标，在版本详情按“场景、事件、发送”展示能力，并允许搜索“官方 QQ Bot”“群内 @”“主动发送”等中文词。

驱动声明目前用于展示和审核，不是运行时权限开关。一个宿主可能同时配置 OneBot 11 与官方 QQ Bot，商城不会仅凭当前配置隐藏其他驱动的插件。安装前由管理员检查目标机器人是否位于版本的兼容矩阵中。

## 常见错误

### 把 OneBot 群聊写成官方 QQ 群聊

OneBot 普通群消息使用 `driver = "onebot11"` 和 `scene = "group"`。官方 QQ 群机器人通常接收 `GROUP_AT_MESSAGE_CREATE`，应使用 `driver = "qq-official"` 和 `scene = "group-at"`。

### 没测试就声明全部能力

商城数据是可审计承诺，不是功能愿望单。没有真实账号权限时可以只声明已经通过夹具和可复现测试的部分，并在 README 写明未验证项。

### 新版本增加驱动却修改旧文件

历史版本不可修改。发布新的 SemVer，在新文件中增加驱动；旧版本继续显示当时的真实范围。

### 只写驱动，不写权限前提

官方 QQ 的频道事件、群事件和主动发送受应用权限、Intents、消息窗口及平台规则影响。`drivers` 只描述插件代码能力，README 仍要列出平台侧开通条件。
