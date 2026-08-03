# 构建产物命名

商城按插件 ID 和 Rust target 计算唯一资产名。文件名必须完全一致，包括前缀、下划线、连字符、target 和扩展名。

## 命名公式

先把插件 ID 中的 `-` 换成 `_`。例如：

```text
plugin_id: group-tools
库名主体: qimen_dynamic_plugin_group_tools
```

Windows：

```text
qimen_dynamic_plugin_<id_with_underscores>-<target>.dll
```

Linux 和 macOS：

```text
libqimen_dynamic_plugin_<id_with_underscores>-<target>.so
libqimen_dynamic_plugin_<id_with_underscores>-<target>.dylib
```

## 完整 target 表

以 `group-tools` 为例：

| 系统 | `target` | Release 资产名 |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-aarch64-pc-windows-msvc.dll` |
| Linux x64 GNU | `x86_64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so` |
| Linux ARM64 GNU | `aarch64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-aarch64-unknown-linux-gnu.so` |
| macOS Intel | `x86_64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-x86_64-apple-darwin.dylib` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-aarch64-apple-darwin.dylib` |

同一 Release 中不同 CPU 的系统动态库原本可能同名，所以商城强制把完整 target 放进资产名。仅写 `linux-amd64`、`arm64` 或 `windows.dll` 都不会通过校验。

## 从 Cargo 输出重命名

Cargo 的默认产物不带 target 后缀：

```text
target/x86_64-unknown-linux-gnu/release/libqimen_dynamic_plugin_group_tools.so
```

上传 Release 前复制或重命名为商城资产名：

```bash
cp \
  target/x86_64-unknown-linux-gnu/release/libqimen_dynamic_plugin_group_tools.so \
  libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
```

Windows PowerShell：

```powershell
Copy-Item `
  .\target\x86_64-pc-windows-msvc\release\qimen_dynamic_plugin_group_tools.dll `
  .\qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll
```

`Cargo.toml` 的包名可以带连字符，Rust 库文件会自动使用下划线。若显式设置了不同的 `[lib].name`，仍要在 Release 前改成商城规定的资产名。

## 版本文件

```toml
[[assets]]
target = "x86_64-unknown-linux-gnu"
asset_name = "libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so"
sha256 = "<64 位十六进制 SHA256>"
size_bytes = 123456
min_glibc = "2.31"
github_attestation = true
```

- `asset_name` 只填写文件名，不能带目录、URL 或通配符。
- `size_bytes` 是动态库本身的准确字节数，不是压缩包大小。
- `sha256` 对动态库本身计算，使用 64 位十六进制字符串。
- GNU/Linux 必须填写 `min_glibc`，其他系统不能填写。
- 每个版本、每个 target 只能登记一个资产。

## SHA256 和字节数

Linux / macOS：

```bash
sha256sum libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
wc -c < libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
```

Windows PowerShell：

```powershell
(Get-FileHash .\qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll -Algorithm SHA256).Hash.ToLower()
(Get-Item .\qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll).Length
```

可以额外上传 `.sha256` 文件方便人工下载核对，但商城只信任主仓库版本文件中经过审核的 SHA256。

## Linux 兼容性

商城不接受 `*-unknown-linux-musl` 动态资产。QimenBot 的 musl 安装包不支持运行时动态加载，复制 `.so` 也无法绕过这个限制。

GNU/Linux 动态库应在与目标服务器相同或更旧的 glibc 环境构建。推荐流水线使用 Debian 11，它的 glibc 是 2.31。不要在 glibc 2.39 的新发行版上构建后，把 `min_glibc` 手工写成 2.31；字段表示二进制的真实最低要求，不是期望值。

可用下面的命令检查依赖和符号版本：

```bash
file libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
ldd libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
readelf --version-info libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so | grep GLIBC_
```

## 不接受的资产

- `.zip`、`.tar.gz` 等压缩包；
- 没有完整 target 的动态库；
- musl `.so`；
- 一个文件同时冒充多个 target；
- 与插件 ID 不一致的库名；
- 同一版本重新上传后 SHA256 发生变化的文件；
- 静态插件的编译资产。静态插件只登记源码和兼容范围。
