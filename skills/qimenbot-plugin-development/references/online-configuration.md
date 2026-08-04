# 动态插件在线配置参考

本参考只服务于动态插件作者。它描述 API 0.6 的配置 Schema、UI Schema、密钥、回调和生效语义；不要把它当成商城后端的实现规范。

## 版本前提

动态 ABI API 和 Rust crate 版本是两套版本：

| 项目 | 当前仓库源码 | crates.io 已发布 |
|---|---|---|
| QimenBot 宿主 | API 0.6 | 以宿主 Release 为准 |
| `abi-stable-host-api` | API 0.6 类型已实现 | `0.1.12` 只到 API 0.5 |
| `qimen-dynamic-plugin-derive` | API 0.6 宏已实现 | `0.1.12` 只到 API 0.5 |

在配套 crate 发布前，仓库内示例使用 path 依赖验证；独立插件仓库不要把 `api = "0.6"` 与 crates.io `0.1.12` 混用。发布后运行：

```bash
cargo search abi-stable-host-api --limit 1
cargo search qimen-dynamic-plugin-derive --limit 1
```

两个包必须使用同一、明确支持 API 0.6 的版本。

## 声明入口

```rust
#[dynamic_plugin(
    id = "stable-plugin-id",
    version = "0.1.0",
    api = "0.6",
    config_schema = "../config.schema.json",
    config_ui = "../config.ui.json",
    config_version = 1,
    config_apply = "reload"
)]
mod plugin {}
```

- `config_schema` 必填，路径相对包含宏调用的 Rust 源文件；文件会被 `include_str!` 编进动态库。
- `config_ui` 可选，省略时面板完全按 JSON Schema 渲染。
- `config_version` 必须大于 0，默认 1；它是插件配置结构版本，不等于 QimenBot 版本或 crate 版本。
- `config_apply` 只能是 `live`、`reload`、`restart`，默认 `reload`。
- Schema 根节点必须明确 `type: "object"`，Schema 和 UI Schema 各不超过 256 KiB；远程、相对 URL `$ref` 会被宿主拒绝。

## Schema 设计

面板支持对象、嵌套对象、数组、对象数组、元组 `prefixItems`、枚举、多选、数值范围、`oneOf` / `anyOf` / `allOf`、`if/then/else`、`dependentSchemas`、本地 `$ref`、动态键、只读字段和可空联合类型。后端始终以 Draft 2020-12 校验结果为准，面板控件只是编辑器。

面向用户的属性至少写：

- `title`：中文字段名；
- `description`：实际业务含义和单位；
- `type`、`required`、范围、长度和枚举；
- `default`：适合新配置的安全默认值，不要放密钥。

`default` 不会改变手工 TOML 的语义。面板首次打开时把默认值放入浏览器草稿，用户保存后才写入；`init` 仍必须为无配置文件和旧配置提供默认值。

### UI Schema 约定

使用 JSON Pointer 指向字段：空字符串代表根，数组段可以写 `*`。支持精确 Pointer、顶层 `fields` 映射、按属性嵌套映射和 Schema 内 `x-qimen-ui`。优先级从高到低为：Schema `x-qimen-ui`、`fields` 精确 Pointer、顶层精确 Pointer、通配 Pointer、嵌套映射。

常用选项：

| 选项 | 示例 | 用途 |
|---|---|---|
| `ui:widget` | `badges` / `select` / `textarea` / `code` / `password` / `color` / `slider` / `json` | 控件偏好 |
| `ui:order` | `["enabled", "mode", "*"]` | 对象字段顺序 |
| `labels` | `{ "qq-official": "官方 QQ Bot" }` | 枚举显示名 |
| `unit` / `step` | `"秒"` / `5` | 单位和数值步长 |
| `rows` / `placeholder` | `4` / `"https://..."` | 文本域和输入提示 |
| `width` / `columns` | `full` / `2` | 字段宽度和对象列数 |
| `itemTitle` / `itemLabel` | `"name"` / `"连接"` | 对象数组标题 |
| `hidden` / `readonly` | `true` | 隐藏或只读展示 |

不要放 HTML、JavaScript、事件处理器或远程资源地址，宿主不会执行这些内容。

## 密钥

字段满足以下任意条件就会脱敏：`writeOnly: true`、`x-qimen-secret: true`、`format: "password"`。建议同时写 `writeOnly` 和 `x-qimen-secret`。

GET 接口永远不返回原文。保存请求把新值放在独立 `secret_updates` 通道；未提交更新时宿主保留磁盘原值，`null` 表示清除。插件回调会收到合并后的真实 JSON，所以不能记录完整 `config_json`。

对象数组排序和删除必须经过管理面板，不要在插件中假设密钥字段会以明文回传。面板提交不含原文的 `secret_references`，让同一 Schema 密钥模板内的值跟随业务项移动。

## 回调契约

```rust
use abi_stable_host_api::{PluginConfigRequest, PluginConfigResult};

#[validate_config]
fn validate(request: &PluginConfigRequest) -> PluginConfigResult {
    // 只解析和检查，不改共享状态、不启动线程。
    PluginConfigResult::ok()
}

#[config_change]
fn apply(request: &PluginConfigRequest) -> PluginConfigResult {
    // 先准备完整的新状态，成功后一次替换旧状态。
    PluginConfigResult::ok()
}
```

`PluginConfigRequest` 有 `plugin_id`、`config_json`、`previous_config_json`。首次创建文件时 `previous_config_json` 为空字符串。两个回调都是同步 FFI，受 `dynamic_plugin_timeout_secs` 限制；宏会把 panic 转为失败结果。

### 选择生效方式

| 模式 | 适合场景 | 作者要求 |
|---|---|---|
| `reload` | 大多数插件，配置影响初始化资源 | `shutdown` 停止并 join 所有线程；`init` 可重复执行 |
| `live` | 只替换内存状态，不想重建路由 | 必须有 `#[config_change]`；先准备后替换，失败可再次应用旧配置 |
| `restart` | 依赖宿主级资源或跨进程状态 | 文件保存后由宿主重启；`init` 仍要校验配置 |

`live` 回调执行期间持有动态库生命周期写锁，不会和插件命令、事件或 Webhook 并发。后台线程重配的安全顺序是：解析新配置 → 创建新资源 → 停止并 join 旧线程 → 安装新状态；任何一步失败都返回错误。

插件未加载时，宿主不能调用 `validate_config` / `config_change`，只做 Schema 校验；下次 `init` 仍是最后一道防线。

## 运行和排错

默认文件：`config/plugins/<plugin_id>.toml`；默认目录可通过 `[official_host].plugin_config_dir` 调整。动态库必须匹配宿主操作系统、CPU、C 运行时和 GNU/musl；musl 宿主不支持动态加载。

检查顺序：

1. `/plugins` 是否显示 API `0.6` 和配置能力；
2. `/dynamic-errors` 是否记录 Schema 根类型、引用或描述符错误；
3. `config/plugins/<id>.toml` 是否为有效 TOML；
4. `config_apply` 与回调导出是否匹配；
5. `shutdown` 是否真的停止并 join 后台线程；
6. Linux 用 `file`、`ldd`、`readelf --version-info` 检查 target 和 glibc。

开发期可临时打开 `qimen_raw_message=debug` 查看原始协议收发；不要把插件配置 JSON 作为调试日志内容。

## 发布前检查

- API、crate 版本和宿主 Release 已分开记录；
- Schema 根类型、字段标题、默认值和旧配置兼容性有测试；
- 密钥没有出现在默认值、README、日志和错误中；
- `live` 回调可逆，`reload` 线程可安全 join；
- 动态库按完整 target 构建，商城 `dynamic_api` 填 `0.6`，并声明真实驱动能力；
- 独立插件仓库不包含 QimenBot path 依赖、运行配置或动态库。
