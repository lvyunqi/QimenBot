# GitHub Actions 发布流水线

QimenBot 仓库提供一份可直接改造的动态插件发布工作流：

- [查看 `marketplace/examples/release.yml`](https://github.com/lvyunqi/QimenBot/blob/main/marketplace/examples/release.yml)
- 保存到插件仓库的 `.github/workflows/release.yml`

它完成代码检查、六个平台构建、固定资产命名、SHA256、Artifact Attestation 和 GitHub Release。静态插件不需要这份工作流，因为商城不会下载静态插件二进制。

## 第一步：修改两个变量

工作流顶部：

```yaml
env:
  PLUGIN_ID: group-tools
  LIB_STEM: qimen_dynamic_plugin_group_tools
```

插件 ID 使用商城永久 ID。`LIB_STEM` 的公式是：

```text
qimen_dynamic_plugin_ + 把 plugin_id 中的 - 换成 _
```

例如：

| `PLUGIN_ID` | `LIB_STEM` |
|---|---|
| `group-tools` | `qimen_dynamic_plugin_group_tools` |
| `weather` | `qimen_dynamic_plugin_weather` |

`verify` job 会检查两者是否对应，防止发布后才发现资产名错误。

## 第二步：确认 Cargo 产物名

`Cargo.toml`：

```toml
[package]
name = "qimen-dynamic-plugin-group-tools"
version = "1.0.0"
edition = "2024"
rust-version = "1.89"

[lib]
crate-type = ["cdylib"]
```

如果设置了自定义 `[lib].name`，Cargo 输出可能不等于 `LIB_STEM`。最简单的做法是移除自定义名称，让包名自动转换成下划线库名；否则同步调整构建步骤的源文件路径，但 Release 目标文件仍必须遵守商城命名。

发布仓库应提交 `Cargo.lock`。工作流中的构建和测试使用 `--locked`，依赖锁不一致会直接失败。

## 第三步：理解四个 job

### `verify`

在 Rust 1.89 上运行：

```text
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

插件需要额外生成代码、安装系统库或运行集成测试时，在这个 job 中补充。不要删除基本检查后只保留 `cargo build`。

### `linux`

构建：

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

两个 job 都在 `rust:1.89-bullseye` 容器中运行，把 glibc 基线固定为 Debian 11 的 2.31。x64 和 ARM64 使用对应架构的 GitHub runner，避免只安装 Rust target 却缺少 linker、libc 和目标架构工具链。

如果插件依赖系统开发包，在 Linux job 的 checkout 后加入：

```yaml
- name: Install system dependencies
  run: |
    apt-get update
    apt-get install -y --no-install-recommends <需要的包>
    rm -rf /var/lib/apt/lists/*
```

安装新系统包可能提高或增加运行时依赖。发布前仍要用 `readelf` 与 `ldd` 检查最终 `.so`。

### `windows` 和 `macos`

使用对应 CPU 的官方 runner 构建：

| runner | target |
|---|---|
| `windows-2022` | `x86_64-pc-windows-msvc` |
| `windows-11-arm` | `aarch64-pc-windows-msvc` |
| `macos-15-intel` | `x86_64-apple-darwin` |
| `macos-15` | `aarch64-apple-darwin` |

每个 job 把 Cargo 原始产物复制成带完整 target 的商城资产名，并生成 `.sha256` 旁路文件。

### `release`

下载六个平台产物，调用 `actions/attest-build-provenance` 生成 GitHub Artifact Attestation，再由 `softprops/action-gh-release` 创建或更新当前 tag 的 Release。

插件仓库需要这些工作流权限：

```yaml
permissions:
  contents: write
  id-token: write
  attestations: write
```

公开仓库的 GitHub 托管 runner 和 Release 存储通常可以免费使用，但仍受 GitHub 当前产品额度、保留期和公平使用规则约束。以仓库 Actions 页面和 GitHub 官方计费说明为准。

## 第四步：打 tag

先让版本保持一致：

- `Cargo.toml` 的 `package.version`；
- `#[dynamic_plugin(..., version = "...")]`；
- README 的兼容说明；
- 准备发布的商城版本文件名。

提交并推送 tag：

```bash
git tag v1.0.0
git push origin v1.0.0
```

示例工作流监听 `v<major>.<minor>.<patch>` 形式，也会接收带后缀的预发布 tag。工作流开始后，到仓库的 Actions 页面逐个检查 job；不要在部分 target 失败时先提交商城 PR。

## 第五步：读取资产数据

Release 完成后，打开每个 `.sha256`，并读取动态库字节数。Linux / macOS：

```bash
sha256sum <asset>
wc -c < <asset>
```

Windows PowerShell：

```powershell
(Get-FileHash .\<asset>.dll -Algorithm SHA256).Hash.ToLower()
(Get-Item .\<asset>.dll).Length
```

把动态库本身的值填入对应 `[[assets]]`。不要填写 `.sha256` 文件的大小或哈希。

如果 Release 页可以验证构建证明，把 `github_attestation = true`。商城 PR 流水线会按二进制 SHA256 查询证明；查不到时校验失败。

## 第六步：提交商城版本

```toml
schema_version = 1
version = "1.0.0"
released_at = "2026-08-03T00:00:00Z"
release_tag = "v1.0.0"
channel = "stable"
qimenbot = ">=0.1.16, <0.2.0"
dynamic_api = "0.5"
yanked = false
data_schema_version = 1
rollback_safe = true

[[drivers]]
driver = "onebot11"
scenes = ["private", "group"]
events = ["message"]
outbound = ["reply"]

[[assets]]
target = "x86_64-unknown-linux-gnu"
asset_name = "libqimen_dynamic_plugin_group_tools-x86_64-unknown-linux-gnu.so"
sha256 = "<Release 动态库的 SHA256>"
size_bytes = 123456
min_glibc = "2.31"
github_attestation = true
```

为 Release 中每个准备支持的 target 增加一项 `[[assets]]`。不要求首次发布就覆盖六个平台，但不能为没有真实资产的平台填占位值。

## 常见失败

### `can't find crate for core/std`

只写了 `--target`，但 Rust target 或目标工具链没有安装。示例使用对应 CPU 的原生 runner；自行改成交叉编译时，还要提供 linker、libc 和系统库。

### `GLIBC_2.39 not found`

插件在过新的 Linux 环境构建。恢复 Debian 11 容器，重新发布新的 SemVer。不要替换原版本资产，也不要只修改 `min_glibc`。

### Release 中两个 CPU 文件重名

上传的是 Cargo 原始产物，没有按商城规则加入完整 target。检查准备资产步骤和 [`LIB_STEM` 命名](/marketplace/artifact-naming)。

### Attestation 校验失败

确认工作流拥有 `id-token: write` 与 `attestations: write`，证明的 subject 是最终上传的动态库，版本文件 SHA256 与该文件完全相同。

### ARM job 找不到 runner

示例使用 GitHub 当前公开 runner 标签。企业或受限组织可能禁用某些镜像；先在仓库 Actions 设置中确认允许的 runner，再选择自托管原生 runner。自托管构建必须公开说明环境和信任边界。
