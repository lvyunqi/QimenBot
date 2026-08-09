# 运行、配置与排错

## 仓库与文档地图

| 内容 | 本地路径 | 在线地址 |
|---|---|---|
| 主仓库 | - | <https://github.com/lvyunqi/QimenBot> |
| 用户文档 | `docs/` | <https://lvyunqi.github.io/QimenBot/> |
| 静态 API | `crates/qimen-plugin-api/` | <https://github.com/lvyunqi/QimenBot/tree/main/crates/qimen-plugin-api> |
| 静态宏 | `crates/qimen-plugin-derive/` | <https://github.com/lvyunqi/QimenBot/tree/main/crates/qimen-plugin-derive> |
| 动态 Host API | `crates/abi-stable-host-api/` | <https://crates.io/crates/abi-stable-host-api> |
| 动态宏 | `crates/qimen-dynamic-plugin-derive/` | <https://crates.io/crates/qimen-dynamic-plugin-derive> |
| 消息模型 | `crates/qimen-message/` | <https://github.com/lvyunqi/QimenBot/tree/main/crates/qimen-message> |
| 静态示例 | `plugins/qimen-plugin-example/` | <https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-plugin-example> |
| 动态示例 | `plugins/qimen-dynamic-plugin-example/` | <https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example> |
| 独立模板 | `templates/dynamic-plugin/` | <https://github.com/lvyunqi/QimenBot/tree/main/templates/dynamic-plugin> |
| 官方 QQ Markdown | - | <https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html> |
| 发行包 | - | <https://github.com/lvyunqi/QimenBot/releases> |

仓库内源码可能领先 crates.io。API 0.6 外部动态插件应同时使用已发布的 `abi-stable-host-api 0.1.13` 与 `qimen-dynamic-plugin-derive 0.1.13`，不依赖浮动分支或作者电脑上的本地 path。

## 宿主配置

```toml
[official_host]
builtin_modules = ["command", "admin"]
plugin_modules = ["my-static-plugin"]
plugin_state_path = "config/plugin-state.toml"
plugin_bin_dir = "plugins/bin"
plugin_config_dir = "config/plugins"
dynamic_plugin_timeout_secs = 30

[official_host.commands]
help_enabled = true
help_page_size = 6
plugins_enabled = true
registry_enabled = true
dynamic_errors_enabled = true
prefixes = ["/"]
private_bare_enabled = true
group_bare_enabled = true
mention_enabled = true
reply_enabled = true

[official_host.webhook]
enabled = false
bind = "127.0.0.1:8088"
base_path = "/webhooks"
max_body_bytes = 1048576
request_timeout_ms = 5000
max_in_flight = 64
access_token = ""

[[bots]]
id = "qq-main"
# OneBot 通常填写 self_id；其他协议填写长期稳定的外部账号标识。
account_id = "2733944636"
enabled = true
```

关键区别：

- `plugin_modules`：允许加载的静态 `#[module(id = "...")]`。
- `plugin_bin_dir`：动态库扫描目录。
- `plugin_state_path`：静态和动态插件共同使用的启停状态。
- `[[bots]].id`：部署实例别名，可能随部署改变。
- `[[bots]].account_id`：主动发送的稳定账号选择器。
- `official_host.webhook`：API 0.5/0.6 动态 Webhook 的统一网关，不是单个插件配置。
- `official_host.commands`：宿主统一命令入口、分页 help 兜底和三个管理员管理命令开关；不定义插件业务命令。

动态插件自身配置位于 `official_host.plugin_config_dir/<plugin_id>.toml`，默认就是
`config/plugins/<plugin_id>.toml`。配置文件名必须与动态描述符 ID 完全一致。宿主会
在 init、reload 和 API 0.6 的即时应用前把 TOML 转为 JSON；目录不存在时会按需创建。
不要把这个目录和 `plugin_state_path` 混用：前者保存插件业务配置，后者只保存启停状态。

## 原始消息日志

开发插件或排查平台差异时，可以只打开原始收发消息日志，不必把所有模块都切到 debug：

```toml
[observability]
level = "info,qimen_raw_message=debug"
```

target 为 `qimen_raw_message` 的日志正文是协议侧完整 JSON。`direction=inbound` 表示收到的消息；`direction=outbound` 表示回复或主动发送，并附带 Bot、协议、传输、动作和来源等字段。它不会记录 QQ Gateway 鉴权帧和心跳，但仍可能包含用户消息、OpenID、群号和媒体地址，只应临时开启。

## 加载流程

静态插件：

```text
Cargo workspace 发现 crate
  -> qimenbotd 依赖并保留 inventory 符号
  -> official_host 扫描 ModuleEntry
  -> plugin_modules + plugin-state 过滤
  -> 注册命令、系统事件和拦截器
```

动态插件：

```text
扫描 plugin_bin_dir
  -> dlopen 动态库
  -> 读取插件描述符和 API 版本
  -> plugin-state 过滤
  -> API 0.4/0.5/0.6 绑定 Host API
  -> API 0.6 读取并校验独立 Schema 描述符
  -> 读取 official_host.plugin_config_dir/<id>.toml
  -> 调用 init
  -> 注册命令、事件、拦截器和 Webhook
```

Web 插件页的“重新扫描”或 `POST /api/v1/plugins/reload` 只重扫动态插件；修改静态插件必须重新编译并重启 `qimenbotd`。

## API 0.6 配置入口

只有导出独立配置描述符的 API 0.6 动态插件才会在管理面板显示配置入口。Schema
负责类型、必填项、范围和条件约束，UI Schema 只负责控件、顺序和中文提示；宿主仍会
在服务端使用 JSON Schema Draft 2020-12 再校验一次，不能只相信浏览器。

管理 API：

| 方法 | 路径 | 作用 |
|---|---|---|
| `GET` | `/api/v1/plugins/<id>/config` | 返回 Schema、UI Schema、当前值、密钥占位信息和 revision |
| `POST` | `/api/v1/plugins/<id>/config/validate` | 只校验草稿，不写文件、不触发生效 |
| `PUT` | `/api/v1/plugins/<id>/config` | 校验、备份、原子写入并按插件声明应用 |

保存时必须原样带回读取到的 `revision`。revision 不一致会返回 `409`，重新读取后
合并自己的修改再保存。`writeOnly`、`x-qimen-secret` 和 `format = "password"`
字段永远不会在 GET 中返回明文；保留旧密钥、替换密钥和清除密钥分别走专用字段，
不要试图把空字符串塞进普通 `values` 绕过保护。

生效模式的行为固定如下：

| 模式 | 保存后的动作 | 插件开发要求 |
|---|---|---|
| `live` | 调用 `#[config_change]`，失败则恢复文件和插件内存状态 | 回调必须可逆、快速、线程安全 |
| `reload` | 停止旧实例并重新执行 `shutdown`/`init` | `init` 必须支持重复调用，后台线程要先停止并 join |
| `restart` | 写入文件并标记宿主需要重启 | 不要在回调中假装已经生效 |

宿主最多保留每个插件 20 份备份，可从面板回滚。回滚同样经过 Schema、业务校验和
对应生效流程；失败时不应留下半写入文件。Schema 只能使用本地 `$ref`，根节点必须
明确声明 `type: "object"`，单个 Schema/UI Schema 最大 256 KiB。

## 管理入口与插件命令边界

插件状态、热重载、在线配置、健康信息和命令优先级可以使用 Web 管理面板或带管理 Token 的 API：

| 方法 | 路径 | 用途 |
|---|---|---|
| `GET` | `/api/v1/plugins` | 查看来源、启停、命令、配置能力和健康状态 |
| `PUT` | `/api/v1/plugins/<id>` | 启用或停用并持久化 |
| `POST` | `/api/v1/plugins/reload` | 重新扫描并热重载动态插件 |
| `PUT` | `/api/v1/plugins/<id>/priority` | 调整同名命令优先级 |

聊天内默认还提供 `/plugins`、`/registry` 和 `/dynamic-errors`，均要求管理员或所有者权限。三者固定以优先级 `10` 进入普通命令注册表，可以在 Web“配置 → 命令入口”逐项关闭；关闭后宿主不保留对应命令名和别名。静态插件默认优先级 `30`、动态插件默认 `20`，所以插件声明同名命令时通常会接管。`ping`、`echo`、`status` 始终完全由插件提供。宿主只在插件未接管时提供可关闭的分页 `help`；插件声明 `help` 或 `h` 后插件优先。

## 官方 QQ Bot 兼容规则

- sender、group、channel、guild、message ID 全程按字符串保存。
- 群聊、C2C、频道和 DMS 的被动回复由运行时根据来信场景路由。
- 群内命令是否需要 @ 由平台事件和命令分发共同决定，不在插件中硬编码 XML/标签删除。
- `ctx.onebot_actions()` 是 OneBot Action 客户端；官方平台专属 Markdown、Keyboard、Ark、Embed 和媒体应使用通用 `Message` 段，并按支持矩阵验证。
- 官方 QQ Markdown 不是浏览器 HTML 的完整替代品；按[官方 Markdown 文档](https://bot.q.qq.com/wiki/develop/api-v2/server-inter/message/type/markdown.html)使用标题、文字样式、链接、图片、列表、引用、分隔线和换行等平台扩展。原生 HTML 标签是否渲染取决于客户端和消息场景，`<br>`、`<font>` 等写法必须实测。
- 官方 QQ 本地媒体使用 `base64://` 通用段：群/C2C 由宿主分片预上传，频道/DMS 图片由宿主发送 multipart；插件不持有 QQ 凭据，也不自行调用预上传接口。
- Base64 错误按“解码、格式、大小、prepare、分片 PUT、part finish、merge、send”阶段定位；日志不得打印完整 Base64。
- 官方原始字段从规范化事件中的 `raw_json.qqbot_payload` 读取；缺失时必须降级。
- 回复配额、媒体上传和平台权限是宿主/开放平台约束，插件不能假设无限主动发送。

## 宿主可信账号上下文

动态命令、拦截器和事件路由的 `raw_event_json` 都包含宿主覆盖的保留对象：

```json
{
  "qimen_context": {
    "version": 1,
    "protocol": "qq-official",
    "bot_instance": "qq-main",
    "account_id": "102012345"
  }
}
```

- `protocol` 是当前适配器协议。
- `bot_instance` 对应可调整的 `[[bots]].id`，不要持久化为账号主键。
- `account_id` 只在管理员配置了非空值时出现；优先用于数据库分区、缓存键和 `BotApi::for_account`。
- `version` 未识别时只使用当前已知字段，不猜测新字段含义。
- 上游事件伪造的同名对象会被宿主替换；宿主不复制 Secret、Token 或其他凭据。

OneBot 插件兼容旧宿主时，可以把协议原生 `self_id` 作为稳定账号回退。官方 QQ 普通事件不保证包含 AppID；缺少 `account_id` 时，有状态插件应要求管理员补配置，不能把不同 Bot 的数据归入 `unknown`。

## 诊断顺序

### 插件完全没有出现

静态插件依次检查：

1. crate 名是否匹配 `plugins/qimen-plugin-*`。
2. `cargo check -p <crate>` 是否通过。
3. `qimenbotd` 是否添加 path 依赖。
4. `extern crate` 和 `black_box(__QIMEN_MODULE_ID)` 是否存在。
5. `plugin_modules` 是否包含模块 ID。
6. `plugin-state.toml` 是否将其禁用。

动态插件依次检查：

1. 文件是否真的位于当前 `plugin_bin_dir`，不是安装根目录；配置文件是否位于当前 `plugin_config_dir`。
2. 扩展名、CPU、操作系统和 GNU/musl 是否匹配。
3. Web 插件页健康状态和实时日志中的第一条加载错误。
4. 描述符 `id`、`api`、API 0.6 配置描述符和两个配套 crate 的真实发布版本。
5. `plugin-state.toml` 是否禁用。
6. Linux 执行 `file`、`ldd`、`readelf --version-info`。
7. API 0.6 插件再检查 Schema 是否是合法 JSON、根节点是否为 object，以及是否误用了远程 `$ref`。

### 插件出现但命令不触发

1. 在 Web 插件页确认命令名、别名来源和插件实际加载状态。
2. 检查同名命令优先级；数值相同时再核对插件声明优先级和插件 ID。
3. 检查 `role`、`scope`、Bot 启用模块和消息是否属于群/私聊。
4. 检查 `[official_host.commands]` 是否启用了当前前缀、私聊裸命令、@ 或回复入口。
5. 官方 QQ 群检查所订阅 Intent、全量消息权限和 @ 事件类型。
6. 确认参数语法：静态 aliases 是数组，动态 aliases 是逗号分隔字符串。
7. 检查前置拦截器是否返回阻断；静态和动态拦截器共用同一条链。

### 拦截器不执行或行为异常

1. 完整阅读 [消息拦截器开发](interceptors.md)，确认目标是 `Message`，不是 notice/request/meta、`MessageSent` 或 Webhook。
2. 检查消息是否在拦截器之前被去重、群过滤、空文本检查或 Bot 级限流丢弃。
3. 静态插件确认模块已启用，`interceptors = [...]` 中的类型是可直接构造的单元结构体；带字段类型需要手工 `Module::interceptors()`。
4. 动态插件确认描述符包含回调符号，重新扫描后日志出现 `registering dynamic interceptor`。
5. 官方 QQ 检查 Intent 和平台权限；宿主未收到的事件不会进入拦截器。
6. `pre_handle` 阻断后不会调用任何 `after_completion`；回复失败或流水线报错也不会调用后置钩子。
7. 动态前置回调失败、panic 或超时时会 fail-open，检查 `dynamic-errors`、插件健康状态和超时熔断记录。

### 能触发但不回复

1. 确认返回的不是 `Continue` / `Ignore`。
2. 检查发送失败日志中的协议、目标 ID、消息类型和官方错误码。
3. 官方 QQ Bot 使用字符串消息 ID；不要调用 `message_id().as_i64()` 一类旧假设。
4. 动态插件确认回调没有 panic、超时或被熔断。
5. 主动发送确认显式选择了有效 `bot_id` / `account_id`，并处理 `SendEnqueueStatus`。

### 热重载后崩溃或仍是旧代码

1. 确认部署的是刚构建的目标目录，不是另一个 `target/release`。
2. Windows 覆盖 DLL 前确认旧库已卸载；Linux 检查 inode 和时间戳。
3. shutdown 停止并 join 所有插件线程，不保留指向动态库代码或内存的回调。
4. 不让宿主持有插件分配的普通 Rust `String`、`Vec`、trait object。
5. 清理重复命名的旧 `.so/.dll/.dylib`，避免一次扫描加载两个版本。

## 平台检查

Linux：

```bash
uname -m
file ./runtime/qimenbotd ./plugins/bin/libqimen_dynamic_plugin_myplugin.so
ldd --version
ldd ./plugins/bin/libqimen_dynamic_plugin_myplugin.so
readelf --version-info ./plugins/bin/libqimen_dynamic_plugin_myplugin.so | grep GLIBC_
```

Windows PowerShell：

```powershell
Get-FileHash .\plugins\bin\qimen_dynamic_plugin_myplugin.dll -Algorithm SHA256
Get-Item .\plugins\bin\qimen_dynamic_plugin_myplugin.dll | Select-Object Name,Length,LastWriteTime
```

Docker：先用 `docker image inspect` 确认 `Architecture`，再按 `linux/amd64 -> x86_64-unknown-linux-gnu`、`linux/arm64 -> aarch64-unknown-linux-gnu` 构建插件。插件目录必须映射到容器内的 `/data/plugins/bin` 或当前配置指定位置。

若启用在线配置，还要把 `official_host.plugin_config_dir` 映射出来，例如 Docker 中
使用 `/data/config/plugins`，这样容器更新或重建不会丢失插件配置和备份。

## 安全与仓库清洁

- Webhook Token、QQ Bot Secret、数据库口令只通过环境变量或用户本地配置提供。
- Webhook 对外开放时必须有 TLS、Bearer token、第三方签名校验和重放保护。
- 动态回调设置超时，后台队列有界，失败重试必须退避。
- API 0.6 配置文件和备份目录只授予宿主运行用户读写权限；不要把包含密钥的 TOML
  映射到公共静态目录或提交到 Git。
- 不提交 `config/plugins/*.toml`、`plugins/bin/`、`*.db`、日志和运行时资源。
- 框架测试、注释和示例使用通用插件 ID 与命令，不能写入私有插件名称或业务资产。

## 验收报告应包含

- 插件类型、插件 ID、命令/事件/Webhook 清单。
- 所用 QimenBot Release、crate 版本、ABI API 和 target。
- 实际运行的格式化、检查、测试、构建和加载命令。
- 生成物准确路径、SHA256（对外发布时）及部署位置。
- 需要用户配置的字段、权限、Intent、Token 和重启/热重载方式。
- 未能在真实宿主验证的协议或平台风险。
