# 驱动兼容声明

商城把驱动兼容性放在 `versions/<version>.toml` 中。原因很简单：插件 `1.0.0` 可能只支持 OneBot 11，`1.1.0` 才增加官方 QQ Bot，同一个插件的不同版本不能共用一份笼统说明。

这组数据用于商城列表、版本详情和搜索。它不会把插件隔离在某个协议中，也不会替代作者的实际测试。QimenBot 可以同时运行多个协议机器人，安装前应按当前使用的机器人驱动检查版本详情。

## 支持的驱动

| `driver` | 商城显示 | 适用范围 |
|---|---|---|
| `onebot11` | OneBot 11 | OneBot 11 正向/反向 WebSocket、HTTP 等接入产生的标准化消息 |
| `qq-official` | 官方 QQ Bot | QQ 开放平台 Gateway 事件和 OpenAPI 回复、主动发送 |

不要用 `onebot11` 代指所有 QQ 消息。官方 QQ Bot 的用户、群和频道标识是字符串，事件类型、可回复窗口和发送限制也与 OneBot 11 不同。

## 消息场景

`scenes` 至少填写一项，只写已经通过真实机器人或可重复夹具验证的场景。

| 值 | 含义 | 常见驱动 |
|---|---|---|
| `private` | OneBot 私聊或官方 QQ 单聊/C2C | 两者 |
| `group` | 普通群消息 | OneBot 11 |
| `group-at` | 群内 @ 机器人消息 | 官方 QQ Bot |
| `channel` | 频道普通消息；OneBot 端通常属于实现扩展 | OneBot 11 |
| `channel-at` | 频道内 @ 机器人消息 | 官方 QQ Bot |
| `channel-private` | 频道私信 | 两者，取决于接入实现和权限 |

OneBot 11 不能声明 `group-at` 或 `channel-at`。这两个值专门表示官方 QQ 开放平台的 @ 消息入口。插件即使共用同一条命令处理函数，也应按事件的真实来源分别声明。

## 事件和发送能力

`events` 表示插件会处理的事件种类：

| 值 | 含义 |
|---|---|
| `message` | 消息或命令 |
| `notice` | 成员变化、撤回、互动等通知 |
| `request` | 好友、加群、邀请等请求 |
| `meta` | 心跳、生命周期等元事件 |

`outbound` 表示插件在该驱动下实际可用的发送方式：

| 值 | 含义 |
|---|---|
| `reply` | 对当前入站事件回复 |
| `proactive` | 脱离当前事件主动发送 |
| `rich-message` | 除纯文本外还支持图片、@、表情等消息段 |

每个驱动至少声明一种事件或发送能力。`rich-message` 不能单独出现，必须同时声明 `reply` 或 `proactive`，否则用户无法判断富媒体通过哪条发送路径生效。

## OneBot 11 示例

```toml
[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message", "notice", "request"]
outbound = ["reply", "proactive", "rich-message"]
```

如果插件只在普通群聊测试过，不要顺手写入 `private`、`channel` 或 `channel-private`。

## 官方 QQ Bot 示例

```toml
[[drivers]]
driver = "qq-official"
scenes = ["private", "group-at", "channel-at", "channel-private"]
events = ["message", "notice"]
outbound = ["reply", "proactive", "rich-message"]
```

官方 QQ Bot 的 `private` 表示开放平台单聊/C2C，不是传统 QQ 数字账号私聊。插件必须把 `openid`、群 ID、频道 ID 等标识按字符串保存和传递。

## 同时支持两个驱动

分别写两个 `[[drivers]]`，不能把场景混进同一项：

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

一个版本最多各登记一次 `onebot11` 和 `qq-official`。增加新驱动、补充场景或修正能力声明时，发布新的 SemVer 版本并新增版本文件，不要修改已经合并的历史版本。

## 投稿前核对

- OneBot 11 的普通群消息和官方 QQ 的群内 @ 已分别测试。
- 官方 QQ Bot 的 openid、群 ID、频道 ID 没有强转为数字。
- `events` 与代码中的命令、路由、通知和请求处理一致。
- `proactive` 已通过后台任务或 Webhook 等真实入口验证。
- `rich-message` 在每个声明的驱动上都验证过所用消息段。
- README 已说明官方平台权限、Intents 和 OneBot 实现扩展等前置条件。
