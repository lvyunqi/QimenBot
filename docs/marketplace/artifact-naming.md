# 构建产物命名

同一 GitHub Release 往往同时包含 Windows x64、Windows ARM64、Linux x64、Linux ARM64 和两种 macOS 动态库。Cargo 原始产物在同一系统的不同 CPU 上会重名，因此商城要求把完整 Rust target 写入资产名。

## 固定公式

将插件 ID 中的连字符换成下划线：

```text
group-tools -> group_tools
```

Windows：

```text
qimen_dynamic_plugin_<id>-<target>.dll
```

Linux 和 macOS：

```text
libqimen_dynamic_plugin_<id>-<target>.so
libqimen_dynamic_plugin_<id>-<target>.dylib
```

这里的 `<id>` 已经使用下划线。例如 `group-tools` 的 Linux x64 文件是：

```text
libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
```

## 六个支持的 target

| 宿主 | `target` | `group-tools` 的资产名 |
|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-x86_64-pc-windows-msvc.dll` |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `qimen_dynamic_plugin_group_tools-aarch64-pc-windows-msvc.dll` |
| Linux x64 GNU | `x86_64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so` |
| Linux ARM64 GNU | `aarch64-unknown-linux-gnu` | `libqimen_dynamic_plugin_group_tools-aarch64-unknown-linux-gnu.so` |
| macOS Intel | `x86_64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-x86_64-apple-darwin.dylib` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `libqimen_dynamic_plugin_group_tools-aarch64-apple-darwin.dylib` |

商城不接受简称。`linux-amd64.so`、`windows-arm64.dll` 或没有 target 的 `libqimen_dynamic_plugin_group_tools.so` 都会被拒绝。

## Cargo 输出和 Release 资产不是同一个名字

指定 target 构建：

```bash
cargo build --locked --release --target x86_64-unknown-linux-gnu
```

Cargo 输出：

```text
target/x86_64-unknown-linux-gnu/release/libqimen_dynamic_plugin_group_tools.so
```

上传前重命名：

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

不要为了得到商城文件名去修改 Rust 的 `[lib].name`。target 后缀属于分发文件名，不属于 crate 内部库名。

## 填写 `[[assets]]`

```toml
[[assets]]
target = "x86_64-unknown-linux-gnu"
asset_name = "libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
size_bytes = 123456
min_glibc = "2.31"
github_attestation = true
```

字段要求：

- `target` 必须是表中的完整 Rust target。
- `asset_name` 只写文件名，不能写 URL、目录或通配符。
- `sha256` 对未压缩动态库计算。
- `size_bytes` 是同一动态库的准确字节数。
- GNU/Linux 必须写真实的 `min_glibc`；Windows 和 macOS 不能写。
- 每个版本、每个 target 只能有一个资产。

## 计算校验值

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

`.sha256` 旁路文件可以一同发布，方便手工下载用户核对。商城安装仍以 QimenBot 主仓库版本文件中的 SHA256 为准，因为同一 Release 中的二进制和旁路校验文件可能被一起替换。

## Linux 的 glibc 基线

动态库会继承构建环境使用的 glibc 符号版本。在 glibc 2.39 系统构建的 `.so`，不能把 `min_glibc` 手工写成 2.31 后期待旧服务器运行。

推荐在 Debian 11 / glibc 2.31 容器中构建：

```yaml
container: rust:1.89-bullseye
```

发布前检查：

```bash
file libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
ldd libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so
readelf --version-info libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so | grep GLIBC_
```

依赖 OpenSSL、数据库客户端等系统动态库时，还要检查目标服务器是否提供相同 SONAME。优先使用可合法静态链接或 vendored 的依赖，并在 README 说明例外。

## 为什么没有 musl

QimenBot 的 musl 安装包是静态分发方案，运行时动态加载被关闭。商城不接受 `x86_64-unknown-linux-musl` 或 `aarch64-unknown-linux-musl` 插件资产。需要动态插件时使用相同 CPU 的 GNU 完整包或官方 Docker 镜像。

## 不要上传压缩包代替动态库

商城安装器按审核后的动态库字节数和 SHA256 下载文件，不会解压 `.zip` 或 `.tar.gz`。Release 可以另外提供压缩包，但版本文件的 `asset_name` 必须指向未压缩的 DLL、SO 或 dylib。
