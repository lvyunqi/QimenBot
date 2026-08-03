# 部署 QimenBot

第一次部署优先使用 Docker。它不需要安装 Rust，配置和插件也会保存在宿主机。只有不能使用 Docker，或者需要修改源码时，再选择二进制或源码构建。

## 选择安装方式

| 方式 | 适合场景 | 更新方法 |
| --- | --- | --- |
| Docker 一键安装 | Linux 服务器，接入 QQ 官方机器人 | 重新拉取镜像 |
| Docker Compose | NAS、Portainer、OneBot、自定义数据盘 | 修改镜像 Tag 后重建容器 |
| Release 二进制 | Windows、macOS、没有 Docker 的 Linux | `qimenbot` 受控更新 |
| 源码构建 | 修改框架或静态插件 | 拉取代码后重新构建 |

Docker Hub 镜像是 [`mryunqi/qimenbot`](https://hub.docker.com/r/mryunqi/qimenbot)，支持 `linux/amd64` 和 `linux/arm64`。预编译二进制在 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases)。

## Docker 一键安装

这条命令适合 QQ 官方机器人。运行前准备好开放平台 AppID 和 Secret，并确认服务器已经安装 Docker Engine 与 Compose v2。

```bash
bash <(curl -fsSL https://raw.githubusercontent.com/lvyunqi/QimenBot/main/deploy/docker/install.sh)
```

脚本会询问 AppID 和 Secret，自动生成管理 Token，然后下载 Compose 文件、创建数据目录并启动容器。Secret 输入时不会显示在终端。

普通用户默认安装到 `~/qimenbot`，root 用户默认安装到 `/opt/qimenbot`。指定其他位置：

```bash
QIMENBOT_HOME=/srv/qimenbot bash <(curl -fsSL https://raw.githubusercontent.com/lvyunqi/QimenBot/main/deploy/docker/install.sh)
```

启动成功后打开 `http://服务器IP:3210/`，使用脚本显示的管理 Token 登录。Intents、所有者、管理员、模块和插件都在 Web 面板中继续配置。

常用命令：

```bash
cd ~/qimenbot
docker compose --env-file .env ps
docker compose --env-file .env logs -f qimenbot
docker compose --env-file .env restart qimenbot
```

再次运行一键命令会保留原来的 `.env` 和数据目录，只更新 Compose 文件并拉取 `.env` 中指定的镜像版本。

## 手动使用 Docker Compose

NAS、Portainer、OneBot 用户，或者需要把数据放到指定磁盘时，按下面的步骤部署。

### 1. 下载文件

```bash
git clone --depth 1 https://github.com/lvyunqi/QimenBot.git
cd QimenBot
cp deploy/docker/.env.example deploy/docker/.env
mkdir -p data/config data/plugins data/logs
```

Windows PowerShell：

```powershell
git clone --depth 1 https://github.com/lvyunqi/QimenBot.git
Set-Location QimenBot
Copy-Item deploy/docker/.env.example deploy/docker/.env
New-Item -ItemType Directory -Force data/config,data/plugins,data/logs
```

### 2. 填写基础环境变量

打开 `deploy/docker/.env`。部署时只需要先理解这些变量：

| 变量 | 是否必填 | 用途 |
| --- | --- | --- |
| `QIMEN_ADMIN_TOKEN` | 必填 | 登录管理面板和调用管理 API，建议使用 32 字节以上随机值 |
| `QQBOT_APPID` | QQ 官方 Bot 必填 | QQ 开放平台 AppID |
| `QQBOT_SECRET` | QQ 官方 Bot 必填 | QQ 开放平台 Secret，不要提交到 Git |
| `QIMEN_WEBHOOK_TOKEN` | 启用 Webhook 时填写 | Webhook Gateway 的 Bearer Token |
| `RUST_LOG` | 选填 | 日志过滤级别，日常使用 `info`，排障时临时改为 `debug` |

Compose 和宿主机相关变量：

| 变量 | 默认值 | 用途 |
| --- | --- | --- |
| `QIMENBOT_IMAGE` | `mryunqi/qimenbot` | Docker Hub 镜像名 |
| `QIMENBOT_TAG` | `latest` | 镜像版本；生产环境建议固定为 `0.1.16` 这类版本号 |
| `QIMEN_CONFIG_DIR` | `./data/config` | 宿主机配置目录 |
| `QIMEN_PLUGIN_DIR` | `./data/plugins` | 宿主机动态插件目录 |
| `QIMEN_LOG_DIR` | `./data/logs` | 宿主机日志或插件文件目录 |
| `QIMEN_MARKETPLACE_DIR` | `./data/marketplace` | 宿主机商城缓存和历史审核二进制目录 |

服务器使用独立数据盘时，可以这样填写：

```dotenv
QIMENBOT_IMAGE=mryunqi/qimenbot
QIMENBOT_TAG=0.1.16

QIMEN_CONFIG_DIR=/srv/qimenbot/config
QIMEN_PLUGIN_DIR=/srv/qimenbot/plugins
QIMEN_LOG_DIR=/srv/qimenbot/logs
QIMEN_MARKETPLACE_DIR=/srv/qimenbot/marketplace

QIMEN_ADMIN_TOKEN=替换为随机长字符串
QQBOT_APPID=替换为你的AppID
QQBOT_SECRET=替换为你的Secret
QIMEN_WEBHOOK_TOKEN=
RUST_LOG=info
```

生成管理 Token：

```bash
openssl rand -hex 32
```

环境变量只负责部署参数和敏感凭据。机器人协议、Intents、沙箱开关、所有者、管理员、内置模块、静态插件、Webhook 参数和日志格式都可以在 Web 面板中修改，不需要为每个配置项增加环境变量。

### 3. 确认目录映射

| 宿主机变量 | 容器目录 | 保存的内容 | 备份要求 |
| --- | --- | --- | --- |
| `QIMEN_CONFIG_DIR` | `/data/config` | `base.toml`、插件状态、插件配置、审计记录 | 必须备份 |
| `QIMEN_PLUGIN_DIR` | `/data/plugins` | `bin/` 下的动态插件 `.so` | 使用动态插件时备份 |
| `QIMEN_LOG_DIR` | `/data/logs` | 插件写入的文件日志 | 按需要备份 |
| `QIMEN_MARKETPLACE_DIR` | `/data/cache/marketplace` | 商城目录缓存、下载资产和可回滚版本 | 使用商城时备份 |

删除或重建容器不会删除宿主机目录。不要把它们放在 `/tmp`，也不要提交到 Git。Linux 容器只能加载针对当前 CPU 架构构建的 `.so`，不能加载 Windows `.dll`。

### 4. 启动

```bash
docker compose --env-file deploy/docker/.env config
docker compose --env-file deploy/docker/.env pull
docker compose --env-file deploy/docker/.env up -d
docker compose --env-file deploy/docker/.env ps
```

状态变成 `healthy` 后，打开 `http://服务器IP:3210/`。启动失败时查看日志：

```bash
docker compose --env-file deploy/docker/.env logs --tail 200 qimenbot
```

### 5. 在 Web 面板中完成配置

首次登录后按这个顺序操作：

1. 在“机器人”页面核对 QQ AppID、Intents、沙箱模式、所有者和管理员。
2. 在“配置”页面选择内置模块和静态插件，保存后按提示重启。
3. 把动态插件 `.so` 放入宿主机的 `QIMEN_PLUGIN_DIR/bin/`，再到“插件”页面重新加载。
4. 在“日志”页面确认 Gateway 已连接，然后回到 QQ 测试私聊、群消息和群内 @。

密码框留空表示保留现有 Secret。直接在面板填写新 Secret 会保存到 `base.toml`；希望凭据与配置文件分离时，继续使用 `.env` 中的 `${QQBOT_SECRET}` 占位方式。

### 6. OneBot 11 首次配置

Docker 默认模板是 QQ 官方 Bot。只使用 OneBot 时，在第一次启动前复制模板：

下面使用默认的 `./data/config`。如果已经修改 `QIMEN_CONFIG_DIR`，请在实际配置目录中创建 `base.toml`。

```bash
mkdir -p data/config
cp deploy/docker/base.toml.example data/config/base.toml
```

删除文件末尾原有的 `[[bots]]`，换成 OneBot 配置：

```toml
[[bots]]
id = "onebot-main"
protocol = "onebot11"
transport = "ws-forward"
endpoint = "ws://host.docker.internal:3001"
enabled = true
enabled_modules = ["command", "admin"]
owners = ["你的QQ号"]
admins = []
```

容器里的 `127.0.0.1` 不是宿主机。OneBot 在宿主机运行时使用 `host.docker.internal`；两个程序都在同一个 Compose 网络时使用 OneBot 服务名，例如 `ws://napcat:3001`。

### 7. 更新与回滚

先备份三个映射目录，再修改 `.env` 中的 `QIMENBOT_TAG`：

```bash
docker compose --env-file deploy/docker/.env pull qimenbot
docker compose --env-file deploy/docker/.env up -d qimenbot
docker compose --env-file deploy/docker/.env logs --tail 100 qimenbot
```

回滚时把 Tag 改回旧版本，执行相同命令。使用 `latest` 也要先执行 `pull`，否则可能继续使用本地旧镜像。

::: warning Docker 由宿主机更新
管理面板会把容器标记为“Docker 编排托管”。不要进入容器覆盖 `qimenbotd`，也不要把 Docker Socket 挂进容器。
:::

## Release 二进制

不能使用 Docker 时，下载 `QimenBot-*` 完整压缩包。根目录的 `qimenbot` 是唯一入口，负责启动、重启、更新和失败回滚；`runtime/qimenbotd` 是内部核心，不要单独运行。

### 1. 下载正确版本

| 系统 | Release 文件名中包含 | 动态插件 target |
| --- | --- | --- |
| Windows 64 位 | `x86_64-pc-windows-msvc` | `x86_64-pc-windows-msvc` |
| 常见 Linux 64 位，glibc 2.31+ | `x86_64-unknown-linux-gnu` | `x86_64-unknown-linux-gnu` |
| Linux ARM64，glibc 2.31+ | `aarch64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` |
| Alpine 或无合适 glibc 的 Linux 64 位 | `x86_64-unknown-linux-musl` | 不支持动态插件 |
| macOS Intel | `x86_64-apple-darwin` | `x86_64-apple-darwin` |
| macOS Apple 芯片 | `aarch64-apple-darwin` | `aarch64-apple-darwin` |

Linux 用 `uname -m` 查看 CPU 架构，用 `ldd --version` 判断 glibc。从 `v0.1.16` 起，GNU 包按 glibc 2.31 构建，Ubuntu 20.04/22.04、Debian 11/12 通常可以直接运行。旧的 `v0.1.15` GNU 包如果提示缺少 `GLIBC_2.39`，请升级到 `v0.1.16`；需要动态插件时不要改用 musl 包。

::: danger 使用动态插件不要下载 musl 包
musl 包是静态程序，不能通过 `dlopen` 加载 `.so`，日志会出现 `Dynamic loading not supported`。需要动态插件但服务器 glibc 低于 2.31 时，请使用 Docker，或升级系统后改用同 CPU 架构的 GNU 包。只有完全不使用动态插件时才选择 musl 包。
:::

Docker 镜像使用 GNU/glibc。`linux/amd64` 镜像对应 `x86_64-unknown-linux-gnu` 插件，`linux/arm64` 镜像对应 `aarch64-unknown-linux-gnu` 插件。完整构建、检查和错误对照见[动态插件开发](/plugin/dynamic#quickstart)。

### 2. 解压并创建配置

```bash
cp -n config/base.toml.example config/base.toml
cp -n config/qimenbot.toml.example config/qimenbot.toml
chmod +x qimenbot
```

Windows PowerShell：

```powershell
Copy-Item .\config\base.toml.example .\config\base.toml -ErrorAction SilentlyContinue
Copy-Item .\config\qimenbot.toml.example .\config\qimenbot.toml -ErrorAction SilentlyContinue
```

编辑 `config/base.toml`，或者先使用包内默认配置启动，再从本机管理面板修改。QQ 官方 Bot 的完整配置见[官方 QQ Bot 接入](/guide/qq-official-quickstart)。

希望把凭据放到环境变量时，先在 `base.toml` 的对应字段中使用占位符：

```toml
[admin_web]
enabled = true
bind = "127.0.0.1:3210"
access_token = "${QIMEN_ADMIN_TOKEN}"

[[bots]]
id = "qq-official"
protocol = "qq-official"
transport = "gateway"
appid = "${QQBOT_APPID}"
secret = "${QQBOT_SECRET}"
intents = ["GROUP_AND_C2C_EVENT"]
enabled = true
owners = []
admins = []
```

然后在启动 `qimenbot` 的同一个终端设置变量：

```bash
export QQBOT_APPID='你的 AppID'
export QQBOT_SECRET='你的 Secret'
export QIMEN_ADMIN_TOKEN='随机管理 Token'
```

```powershell
$env:QQBOT_APPID = "你的 AppID"
$env:QQBOT_SECRET = "你的 Secret"
$env:QIMEN_ADMIN_TOKEN = "随机管理 Token"
```

### 3. 启动 QimenBot

```bash
./qimenbot run
```

```powershell
.\qimenbot.exe run
```

打开 `http://127.0.0.1:3210/`。服务器远程访问不要直接暴露无加密面板，按[生产环境运维](/advanced/operations)配置 systemd、Windows Service 或 HTTPS 反向代理。

### 4. 在线更新

管理面板的“版本更新”页可以检查和安装 Release。命令行也可以向正在运行的 `qimenbot` 投递操作：

```bash
./qimenbot check
./qimenbot install
./qimenbot restart
```

更新只替换 `runtime/qimenbotd`，不会改动配置、插件和数据库。新版本未通过 `/healthz` 时会自动恢复旧核心。根目录的 `qimenbot` 需要升级时，停止服务后用新压缩包中的同名文件手工覆盖。

### 5. 校验下载文件（可选）

完整压缩包带有同名 `.sha256`。以下命令以 `v0.1.16` Linux x86_64 GNU 包为例：

```bash
sha256sum -c QimenBot-v0.1.16-x86_64-unknown-linux-gnu.tar.gz.sha256
```

## 从源码构建

源码构建需要 Rust 1.89+、Node.js 22、npm 和 Git。管理面板会嵌入 daemon，必须先构建前端：

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot
npm --prefix web/admin ci
npm --prefix web/admin run build
cargo build --release --locked --package qimenbotd --package qimenbot
```

本地开发可以在仓库根目录运行：

```bash
cargo run --package qimenbotd
```

跳过前端构建时，管理面板只会显示未构建提示，不应作为生产包。动态插件是独立 workspace，需要进入插件目录单独构建。

## 数据备份

Docker 至少备份 `.env` 和三个映射目录。为了保证插件数据库完整，先停止容器：

```bash
docker compose --env-file deploy/docker/.env stop qimenbot
tar -czf "qimenbot-$(date +%Y%m%d-%H%M%S).tar.gz" data deploy/docker/.env
docker compose --env-file deploy/docker/.env start qimenbot
```

使用绝对映射路径时，把命令里的 `data` 换成实际的配置、插件和日志目录。二进制部署直接在停止 `qimenbot` 后备份整个安装目录。备份中含有明文凭据，应加密并限制读取权限。

## 默认端口

| 端口 | 用途 | 公网建议 |
| --- | --- | --- |
| `3210` | 管理面板、API、`/healthz` | 不直接开放，使用 HTTPS 反向代理 |
| `6701` | OneBot 反向 WebSocket | 只有外部 OneBot 需要连接时开放 |
| `8088` | 动态插件 Webhook | 只有启用 Webhook 时开放 |

QQ 官方 Bot 通过出站 HTTPS 和 WSS 连接，一般不需要额外开放入站端口。systemd、Windows Service、Nginx、防火墙、日志留存和恢复流程见[生产环境运维](/advanced/operations)。

## 常见问题

| 现象 | 处理方法 |
| --- | --- |
| Compose 提示变量未设置 | 确认命令带有 `--env-file deploy/docker/.env`，并填写 `QIMEN_ADMIN_TOKEN` |
| 容器一直 `unhealthy` | 执行 `docker compose --env-file deploy/docker/.env logs --tail 200 qimenbot` |
| 面板打不开 | 本机先访问 `http://127.0.0.1:3210/healthz`，再检查端口映射和防火墙 |
| OneBot 在宿主机但容器连不上 | endpoint 改为 `host.docker.internal`，并让 OneBot 监听 Docker 网桥可访问的地址 |
| QQ 官方 Bot 鉴权失败 | 重新核对 AppID、Secret、沙箱环境，检查值前后是否有空格 |
| 私聊正常但群内 @ 不回复 | 按[官方 QQ Bot 接入](/guide/qq-official-quickstart)检查事件权限和 Intents |
| `Dynamic loading not supported` | 当前是静态 musl 包；换用同 CPU 架构的 GNU 包或 Docker |
| 动态插件加载失败 | 用 `file`、`ldd` 检查 `.so`，确认系统、CPU、GNU target、glibc 和插件 API 均与 QimenBot 一致 |
| 源码构建后面板显示未构建 | 先执行 `npm --prefix web/admin run build`，再重新构建 Rust |

排障时不要先删除配置目录或 `.qimen-update/`。先保存日志和配置副本，再做修改。

项目维护者发布镜像的步骤见[发布 Docker Hub 镜像](/advanced/docker-publishing)。
