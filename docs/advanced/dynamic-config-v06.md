# 动态插件 API 0.6 在线配置

API 0.6 允许动态插件把配置结构交给 QimenBot。插件声明 JSON Schema 后，Web 管理面板会生成表单；保存时由宿主统一完成校验、密钥合并、备份、写入和生效。

配置文件仍是普通 TOML，默认位于 `config/plugins/<插件ID>.toml`。不用管理面板的部署可以继续手工编辑，插件也不需要依赖浏览器才能运行。

::: warning 发布状态
`abi-stable-host-api 0.1.12` 和 `qimen-dynamic-plugin-derive 0.1.12` 只支持到动态 API 0.5，不能把依赖保持在 `0.1.12` 却声明 `api = "0.6"`。

在配套 crate 发布前，仓库内示例使用本地 path 依赖；仓库外插件可以使用官方模板固定的公开 Git revision。发布后先运行 `cargo search abi-stable-host-api --limit 1` 和 `cargo search qimen-dynamic-plugin-derive --limit 1`，确认两个包的同一版本明确支持 API 0.6，再改用 crates.io。
:::

## 宿主实际做了什么

一次保存依次经过下面几步：

1. 管理 API 根据插件 ID 找到当前动态库和配置文件。
2. 检查浏览器提交的 revision，拒绝覆盖其他窗口刚保存的内容。
3. 合并需要保留的密钥；密钥原文不会返回浏览器。
4. 使用 JSON Schema Draft 2020-12 校验完整配置。
5. 插件导出 `#[validate_config]` 时，再执行插件自己的业务校验。
6. 将 JSON 转为 TOML，备份旧文件并替换当前文件。
7. 按 `live`、`reload` 或 `restart` 应用配置；失败时恢复旧文件和旧运行状态。

宿主最多为每个插件保留 20 份配置备份。在线表单和管理 API 不允许远程 `$ref`，也不允许插件注入 HTML 或 JavaScript。

## 最小目录

一个支持在线配置的插件至少包含下面三个文件：

```text
qimen-dynamic-plugin-myplugin/
├── Cargo.toml
├── config.schema.json
└── src/
    └── lib.rs
```

`config.ui.json` 是可选文件。JSON Schema 已经足够生成可用表单；只有需要指定徽标、滑杆、单位、字段顺序等展示细节时才增加 UI Schema。

独立插件使用官方模板固定的公开提交，不需要下载 QimenBot 主框架源码：

```toml
[dependencies]
abi-stable-host-api = { git = "https://github.com/lvyunqi/QimenBot.git", rev = "5a69e242df31813ddafa327ccbc005bb48c8c3d3" }
qimen-dynamic-plugin-derive = { git = "https://github.com/lvyunqi/QimenBot.git", rev = "5a69e242df31813ddafa327ccbc005bb48c8c3d3" }
abi_stable = "0.11"
serde_json = "1"
```

固定 revision 是 API 0.6 正式发布前的过渡方案。不要改成浮动的 `main` 或功能分支，也不要保留指向作者电脑目录的 path。两个配套 crate 发布后，应同时切换到明确支持 API 0.6 的同一 crates.io 版本。

## 声明配置

`#[dynamic_plugin]` 新增四个配置参数：

```rust
use abi_stable_host_api::{
    CommandRequest, CommandResponse, PluginConfigRequest, PluginConfigResult,
    PluginInitConfig, PluginInitResult,
};
use qimen_dynamic_plugin_derive::dynamic_plugin;

#[dynamic_plugin(
    id = "my-plugin",
    version = "0.1.0",
    api = "0.6",
    config_schema = "../config.schema.json",
    config_ui = "../config.ui.json",
    config_version = 1,
    config_apply = "reload"
)]
mod plugin {
    use super::*;

    #[init]
    fn init(config: PluginInitConfig) -> PluginInitResult {
        let raw = config.config_json.as_str();
        let value = if raw.is_empty() {
            serde_json::json!({})
        } else {
            match serde_json::from_str::<serde_json::Value>(raw) {
                Ok(value) => value,
                Err(error) => return PluginInitResult::err(&error.to_string()),
            }
        };

        let enabled = value["enabled"].as_bool().unwrap_or(true);
        eprintln!("plugin enabled: {enabled}");
        PluginInitResult::ok()
    }

    #[validate_config]
    fn validate(request: &PluginConfigRequest) -> PluginConfigResult {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(request.config_json.as_str()) else {
            return PluginConfigResult::err("配置不是有效 JSON");
        };
        if value["workers"].as_u64().unwrap_or(1) > 32 {
            return PluginConfigResult::err("workers 不能超过当前服务允许的 32");
        }
        PluginConfigResult::ok()
    }

    #[command(name = "status", description = "查看插件状态")]
    fn status(_request: &CommandRequest) -> CommandResponse {
        CommandResponse::text("running")
    }
}
```

| 参数 | 是否必填 | 含义 |
| --- | --- | --- |
| `config_schema` | 是 | 相对 `src/lib.rs` 的 JSON Schema 文件；声明后插件才会显示配置按钮 |
| `config_ui` | 否 | 相对 `src/lib.rs` 的 QimenBot UI Schema 文件 |
| `config_version` | 否 | 插件自己的配置结构版本，必须大于 0，默认 1 |
| `config_apply` | 否 | `live`、`reload` 或 `restart`，默认 `reload` |

`include_str!` 会把两个 JSON 文件编进动态库，部署时不需要把 Schema 单独复制到服务器。

## 编写 JSON Schema

根节点必须明确写成 `type: "object"`。Schema 最大 256 KiB，只能使用 `#/$defs/...` 一类本地引用。

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "状态通知插件",
  "type": "object",
  "additionalProperties": false,
  "required": ["enabled", "level", "workers"],
  "properties": {
    "enabled": {
      "title": "启用通知",
      "type": "boolean",
      "default": true
    },
    "level": {
      "title": "通知级别",
      "type": "string",
      "enum": ["info", "warning", "critical"],
      "x-enumNames": ["普通", "重要", "紧急"],
      "default": "info"
    },
    "workers": {
      "title": "工作线程",
      "type": "integer",
      "minimum": 1,
      "maximum": 32,
      "default": 4
    }
  }
}
```

`title` 是表单上的中文名称，配置键仍显示为等宽小字。`description` 用于解释业务含义。没有 `title` 时，面板只能根据字段名生成显示名称，因此对外发布的插件应为每个字段写清楚 `title`。

### 表单控件映射

| Schema | 默认控件 | 补充支持 |
| --- | --- | --- |
| `type: boolean` | Switch；可选布尔值使用“未设置 / 开启 / 关闭”分段选项 | `default` |
| `type: string` | 单行输入框 | `minLength`、`maxLength`、`pattern`、`email`、`uri`、`date`、`color` |
| `enum` 或全部由 `const` 组成的 `oneOf` | 少量选项显示为徽标，多量选项使用下拉框 | `x-enumNames`、分支 `title` |
| `integer` / `number` | 数字输入和步进按钮 | `minimum`、`maximum`、`multipleOf`、滑杆 |
| `type: object` | 分组表单 | `properties`、`required`、`additionalProperties`、`patternProperties` |
| `type: array` | 可添加、删除、复制、上下移动的列表 | `items`、`prefixItems`、`minItems`、`maxItems`、`uniqueItems` |
| 数组 `items.enum` + `uniqueItems: true` | 多选徽标 | `minItems`、`maxItems` |
| `oneOf` / `anyOf` | 配置形式选择器 | 分支 `title` |
| `allOf`、`if/then/else`、`dependentSchemas` | 合并当前条件对应的字段和必填状态 | 最终结果仍由后端完整校验 |
| 本地 `$ref` | 解析 `#` 和 `#/$defs/...` | 远程和相对 URL 引用会被拒绝 |
| `readOnly: true` | 只读值 | 适合显示由插件固定的标识或结构版本 |
| `type: ["string", "null"]` | 可清空为 `null` | 其他包含 `null` 的联合类型同理 |

JSON Schema 的 `default` 是注解，不会由标准校验器自动写入文件。管理面板首次打开时会把默认值填进草稿，用户保存后才会落盘；插件仍应在 `init` 中为手工配置和旧配置提供默认值。

## UI Schema

UI Schema 使用 JSON Pointer 指向字段。根节点用空字符串，数组下标可以用 `*` 通配。

```json
{
  "": {
    "ui:order": ["enabled", "level", "message", "interval_secs", "connections"]
  },
  "/level": {
    "ui:widget": "badges"
  },
  "/message": {
    "ui:widget": "textarea",
    "rows": 5,
    "placeholder": "输入通知正文"
  },
  "/interval_secs": {
    "ui:widget": "slider",
    "unit": "秒",
    "step": 5
  },
  "/connections": {
    "itemTitle": "name",
    "itemLabel": "连接",
    "addLabel": "添加连接",
    "collapsible": true
  },
  "/connections/*/timeout_secs": {
    "unit": "秒"
  }
}
```

也可以把字段映射放进顶层 `fields` 对象，或者使用按属性名嵌套的写法。相同字段同时出现时，优先级依次是 Schema 内的 `x-qimen-ui`、精确 Pointer、`fields` 精确 Pointer、通配 Pointer和嵌套配置。

| UI 选项 | 可用值 | 作用 |
| --- | --- | --- |
| `ui:widget` / `widget` | `badges`、`radio`、`select`、`textarea`、`code`、`password`、`color`、`slider`、`range`、`json` | 指定控件 |
| `ui:order` / `order` | 字段名数组，`*` 表示其余字段 | 调整对象字段顺序 |
| `title` / `ui:title` | 字符串 | 覆盖 Schema `title` |
| `description` | 字符串 | 覆盖 Schema `description` |
| `placeholder` | 字符串 | 输入框占位内容 |
| `help` | 字符串 | 字段下方的补充说明 |
| `unit` | 字符串 | 数字或文本输入右侧单位 |
| `step` | 数字 | 数字步进和滑杆步长 |
| `rows` | 正整数 | 多行输入高度 |
| `width` | `full` 或 `half` | 表单列宽 |
| `columns` | 正整数 | 对象内字段列数；手机端始终单列 |
| `labels` | 对象 | 枚举原值到中文名称的映射 |
| `hidden` | 布尔值 | 不显示字段但保留已有值；不能代替密钥标记 |
| `readonly` / `disabled` | 布尔值 | 只读展示或禁用控件 |
| `itemTitle` | 对象数组中的字段名 | 用该字段值作为数组项标题 |
| `itemLabel` / `addLabel` / `emptyLabel` | 字符串 | 数组标题、添加按钮和空状态文字 |
| `collapsible` / `collapsed` | 布尔值 | 对象数组是否可折叠及默认状态 |

UI Schema 只影响显示，不改变后端验证，也不能放宽 JSON Schema。宿主只读取已知选项，其他字段会被忽略。

## 密钥字段

下面三种写法都会被视为密钥：

```json
{
  "type": "string",
  "writeOnly": true,
  "x-qimen-secret": true,
  "minLength": 16
}
```

- `writeOnly: true`
- `x-qimen-secret: true`
- `format: "password"`

建议同时使用 `writeOnly` 和 `x-qimen-secret`，意图最清楚。不要在 Schema 的 `default`、`examples` 或 UI Schema 中放真实凭据。

GET 配置时，宿主只返回“已配置”状态，值会被替换为 `null`。保存时有三种明确操作：

- 不修改：保留磁盘中的原值。
- 输入新值：通过独立的 `secret_updates` 通道替换。
- 点击清除：删除 TOML 中的字段，再执行必填校验。

对象数组删除或排序时，面板用不含原文的引用让密钥跟随原数组项移动，避免把第一项的 Token 错配给第二项。引用只能在同一个 Schema 密钥字段模板内一对一移动。

插件的 `#[init]`、`#[validate_config]` 和 `#[config_change]` 收到的是宿主合并后的完整 JSON，包含实际密钥。不要打印整个 `config_json`，也不要把它写进错误信息。

## 三种生效方式

### `reload`：默认选择

```rust
config_apply = "reload"
```

保存时宿主暂停动态加载，调用旧实例 `shutdown`，写入配置，重新扫描并调用 `init`。新配置导致 `init` 失败时，宿主恢复旧文件并重新加载旧配置。

大多数插件应先使用 `reload`。业务状态集中在 `init` / `shutdown`，实现简单，回滚路径也容易验证。

### `live`：不中断插件路由

```rust
config_apply = "live"
```

`live` 必须同时提供一个 `#[config_change]`：

```rust
#[config_change]
fn apply(request: &PluginConfigRequest) -> PluginConfigResult {
    let next = match parse_config(request.config_json.as_str()) {
        Ok(next) => next,
        Err(error) => return PluginConfigResult::err(&error),
    };

    // 先完整构造 next_state，成功后再一次替换共享状态。
    replace_state(next);
    PluginConfigResult::ok()
}
```

宿主会独占插件生命周期锁，配置应用期间不会同时执行命令、事件或 Webhook 回调。返回失败时，宿主先恢复旧文件，再用旧配置调用一次 `#[config_change]` 恢复插件内存状态。

回调必须满足两个条件：

1. 先解析和准备新资源，全部成功后再替换当前状态，不能改到一半才返回错误。
2. 能接受 `previous_config_json` 为空的首次配置，也能再次应用旧配置完成回滚。

后台线程需要停止并 `join` 后再启动新线程。不要让新旧线程同时使用同一数据库、socket 或文件句柄。

### `restart`：等待宿主重启

```rust
config_apply = "restart"
```

宿主只保存文件并在面板标记“需要重启”。配置改变宿主级资源、插件无法安全重载，或需要与其他进程一起切换时使用这个模式。

插件未加载或已停用时，三种模式都会先保存文件，并在插件下次加载时通过 `init` 生效。

## 插件业务校验

JSON Schema 适合验证类型、长度、范围和条件必填。需要访问多个字段的业务规则放进 `#[validate_config]`：

```rust
#[validate_config]
fn validate(request: &PluginConfigRequest) -> PluginConfigResult {
    let value: serde_json::Value = match serde_json::from_str(request.config_json.as_str()) {
        Ok(value) => value,
        Err(error) => return PluginConfigResult::err(&error.to_string()),
    };

    let primary = value["primary_endpoint"].as_str();
    let fallback = value["fallback_endpoint"].as_str();
    if primary == fallback {
        return PluginConfigResult::err("主地址和备用地址不能相同");
    }
    PluginConfigResult::ok()
}
```

`PluginConfigRequest` 字段：

| 字段 | 含义 |
| --- | --- |
| `plugin_id` | 当前插件 ID |
| `config_json` | 已合并保留密钥的新配置 |
| `previous_config_json` | 保存前配置；首次创建文件时为空字符串 |

校验回调只检查，不修改插件状态，不发消息，不启动线程。回调是同步 FFI，会受宿主 `dynamic_plugin_timeout_secs` 限制。

插件没有加载时无法调用业务校验，只执行 JSON Schema 校验。插件下次加载仍必须在 `init` 中拒绝无效配置。

## 配置文件和目录

宿主配置可以修改插件配置目录：

```toml
[official_host]
plugin_config_dir = "config/plugins"
```

文件名固定为 `<plugin_id>.toml`。插件 ID 只能包含 ASCII 字母、数字、`-`、`_` 和 `.`，最长 128 字节，不能是 `.` 或 `..`。

Docker 部署应持久化整个 `/data/config`；二进制部署应备份 `config/plugins/`。`.qimen-revisions/` 是宿主自动维护的在线配置历史目录，不要让插件自己修改。

## 管理 API

面板使用下面三个接口，路径中的插件 ID 需要 URL 编码：

```text
GET  /api/v1/plugins/{id}/config
POST /api/v1/plugins/{id}/config/validate
PUT  /api/v1/plugins/{id}/config
```

保存请求的结构：

```json
{
  "revision": "当前 SHA-256 revision",
  "values": {
    "enabled": true
  },
  "secret_updates": {
    "/token": "new-secret",
    "/old_token": null
  },
  "secret_references": {
    "/connections/0/token": "/connections/1/token"
  }
}
```

普通客户端通常不需要自己构造 `secret_references`；这是管理面板在对象数组调整顺序时使用的内部通道。revision 不一致返回 HTTP 409，客户端应重新读取，不能静默重试覆盖。

## 从 API 0.5 升级

没有在线配置需求的 API 0.5 插件不必升级，旧动态库继续加载。需要升级时按下面顺序处理：

1. 等待支持 API 0.6 的两个配套 crate 正式发布，并保持版本一致。
2. 把 `api = "0.5"` 改为 `api = "0.6"`。
3. 增加根节点为 object 的 `config.schema.json`。
4. 选择 `reload`、`live` 或 `restart`；不确定时先用 `reload`。
5. 把现有 TOML 配置作为测试数据，确认 Schema 能接受旧配置。
6. 为配置解析、业务校验、线程关闭和失败回滚增加测试。
7. 在插件商城版本文件中声明 `dynamic_api = "0.6"`，并把 QimenBot 版本范围设为包含 API 0.6 的宿主版本。

API 0.6 没有修改旧 `PluginDescriptor` 布局。配置描述符和回调使用新的独立导出符号，因此宿主仍能加载 API 0.1 至 0.5 插件。

## 排错

### 插件卡片没有“配置”按钮

- 动态库必须声明 `api = "0.6"` 和 `config_schema`。
- 检查启动日志或 `/dynamic-errors` 中的第一条描述符错误。
- Schema 根节点必须明确是 `type: "object"`，JSON 文件不能超过 256 KiB。
- `config_schema` 路径相对 `src/lib.rs`，不是相对 QimenBot 启动目录。

### 表单能打开但不能保存

- 先处理字段下方和底部显示的 Schema 错误。
- HTTP 409 表示其他窗口或手工编辑已经改变文件，点击“重新读取”。
- 插件业务校验错误来自 `#[validate_config]`，应返回能直接定位字段的短消息。
- 手工 TOML 中的 `null` 无法保存；数组也不能包含 `null` 项。

### `live` 保存后自动恢复

`#[config_change]` 返回了错误、panic 或超时。查看日志中的“文件恢复”和“运行状态恢复”结果。先把插件改为 `reload` 验证 Schema 和配置解析，再单独排查即时状态替换。

### `reload` 后插件没有回来

新配置通过 Schema，但插件 `init` 返回失败。宿主会恢复旧文件并尝试重新加载旧配置；如果旧配置也无法加载，检查插件线程是否在 `shutdown` 中真正停止并 `join`。

## 发布前检查

- Schema 根节点明确写了 `type: "object"` 和 `$schema`。
- 所有面向用户的字段都有中文 `title`，复杂字段有准确 `description`。
- 必填、默认值、范围、枚举、条件字段和旧配置都经过测试。
- 密钥使用 `writeOnly` / `x-qimen-secret`，日志和错误不包含 `config_json`。
- `config_apply` 与实际生命周期一致；`live` 回调能恢复旧配置。
- 对象数组增删、排序后，密钥仍属于原来的业务项。
- 动态库在目标操作系统、CPU 和 GNU libc 环境实际加载通过。
- 商城版本声明 `dynamic_api = "0.6"`，没有把 API 0.5 宿主标成兼容。

完整实现可参考 [`plugins/qimen-dynamic-plugin-example`](https://github.com/lvyunqi/QimenBot/tree/main/plugins/qimen-dynamic-plugin-example)。
