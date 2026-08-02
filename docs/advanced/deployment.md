# 部署、更新与回滚

QimenBot 可以用 Docker Compose、Release 二进制或源码运行。三种方式使用同一套机器人配置，但进程管理和更新方式不同。生产环境只选一种，不要同时让 Docker、systemd 和 launcher 管理同一个实例。

## 选择部署方式

| 方式 | 适合场景 | 更新方式 | 需要编译环境 |
| --- | --- | --- | --- |
| Docker Compose | Linux 服务器、NAS、Portainer、已有容器环境 | 拉取新镜像并重建容器 | 否 |
| Release 二进制 + launcher | Windows、macOS、普通 Linux 主机 | 管理面板或 launcher 安装 Release | 否 |
| 源码构建 | 修改框架、开发静态插件、定制发行包 | 拉取代码后重新构建 | Rust 1.89+、Node.js 22 |

官方 Docker Hub 镜像为 [`mryunqi/qimenbot`](https://hub.docker.com/r/mryunqi/qimenbot)，预编译安装包位于 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases)。

部署前先记住四件事：

1. Docker 部署由 Compose、Portainer 或 Kubernetes 更新，容器内不运行 `qimen-launcher`。
2. 二进制部署只启动 `qimen-launcher`，不要再单独注册或守护 `qimenbotd`。
3. `config/`、`plugins/`、插件数据库和环境变量文件属于业务数据，升级前需要备份。
4. 同一份配置不要同时启动两个实例。QQ 官方 Bot 的 Gateway 会话和 OneBot 端口都可能发生冲突。

## 部署前准备

### 端口

| 默认端口 | 用途 | 公网建议 |
| --- | --- | --- |
| `3210` | Web 管理面板、API、`/healthz` | 不直接开放；使用防火墙或 HTTPS 反向代理 |
| `6701` | OneBot 反向 WebSocket 等自定义传输 | 只有外部 OneBot 实现需要连接时才开放 |
| `8088` | 动态插件 Webhook Gateway | 只有启用 Webhook 时才开放 |

QQ 官方 Bot 通过出站 HTTPS 和 WSS 连接腾讯服务器，通常不需要开放额外入站端口。OneBot 正向 WebSocket 同样由 QimenBot 主动连接 OneBot 实现。

### 凭据

- QQ 官方 Bot 使用 `QQBOT_APPID` 和 `QQBOT_SECRET`。
- 管理面板监听非回环地址时必须设置 `QIMEN_ADMIN_TOKEN`。
- Webhook 启用后应使用独立的 `QIMEN_WEBHOOK_TOKEN`。
- OneBot 的 Access Token 应按对应实现的要求写入 Bot 配置。

不要把真实凭据写进 `compose.yaml`、示例配置、截图或 Git 提交。QQ 官方 Bot 的 AppID、Secret、Intents 和沙箱设置见[官方 QQ Bot 接入](/guide/qq-official-quickstart)。

## Docker Compose

Docker 镜像同时提供 `linux/amd64` 和 `linux/arm64`。Docker 会根据宿主机架构自动选择，不需要手工区分 x86_64 和 ARM64。

### 1. 安装 Docker

先安装 Docker Engine 20.10+ 和 Compose v2，然后确认命令可用：

```bash
docker version
docker compose version
```

下面的命令都在 QimenBot 仓库根目录执行：

```bash
git clone --depth 1 https://github.com/lvyunqi/QimenBot.git
cd QimenBot
```

使用 Portainer 时，可以直接把仓库中的 `compose.yaml` 作为 Stack 文件；环境变量按下一节填写，三个 `./data/...` 挂载路径改为 Portainer 主机上的绝对路径更容易管理。

### 2. 创建环境变量文件

```bash
# Linux / macOS
cp deploy/docker/.env.example deploy/docker/.env
```

```powershell
# Windows PowerShell
Copy-Item deploy/docker/.env.example deploy/docker/.env
```

先生成管理 Token：

```bash
openssl rand -hex 32
```

```powershell
$bytes = New-Object byte[] 32
[Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
[Convert]::ToBase64String($bytes)
```

打开 `deploy/docker/.env`，至少修改管理 Token。使用 QQ 官方 Bot 时还要填写 AppID 和 Secret：

```dotenv
QIMENBOT_IMAGE=mryunqi/qimenbot
QIMENBOT_TAG=0.1.15
QIMEN_ADMIN_TOKEN=替换为刚才生成的随机字符串
QQBOT_APPID=替换为QQ开放平台AppID
QQBOT_SECRET=替换为QQ开放平台Secret
QIMEN_WEBHOOK_TOKEN=
```

生产环境建议固定 `QIMENBOT_TAG`，例如 `0.1.15`。`latest` 方便体验，但镜像内容会随新版本变化，回滚时不容易确认原版本。

### 3. 选择机器人协议

容器首次启动时会创建 `data/config/base.toml`。默认模板已经配置为 QQ 官方 Bot，只要环境变量正确即可启动。

使用 OneBot 11 时，应在第一次启动前准备配置：

```bash
mkdir -p data/config
cp deploy/docker/base.toml.example data/config/base.toml
```

删除文件末尾原有的 `[[bots]]` 区块，换成自己的 OneBot 配置。例如 OneBot 正向 WebSocket：

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

`127.0.0.1` 在容器内指向容器自身，不是宿主机。仓库的 Compose 配置已经把 `host.docker.internal` 映射到宿主机；如果 OneBot 也运行在同一个 Compose 网络中，优先把 `endpoint` 写成 OneBot 服务名，例如 `ws://napcat:3001`。

OneBot 反向 WebSocket 需要让 OneBot 实现连接 QimenBot 的监听地址。只有 OneBot 位于另一台主机时，才需要在防火墙中放行 `6701`。

### 4. 启动并验证

```bash
docker compose --env-file deploy/docker/.env config
docker compose --env-file deploy/docker/.env pull
docker compose --env-file deploy/docker/.env up -d
docker compose --env-file deploy/docker/.env ps
```

`config` 命令只检查最终 Compose 配置，不会启动容器。`ps` 中的状态变成 `healthy` 后，再打开 `http://服务器地址:3210/`，输入 `QIMEN_ADMIN_TOKEN`。

也可以直接检查健康端点：

```bash
curl --fail http://127.0.0.1:3210/healthz
```

查看启动日志：

```bash
docker compose --env-file deploy/docker/.env logs --tail 200 qimenbot
docker compose --env-file deploy/docker/.env logs -f qimenbot
```

按 `Ctrl+C` 只会退出日志跟随，不会停止容器。

### 5. 持久化目录

Compose 使用宿主机目录挂载，数据位于仓库根目录的 `data/`：

```text
data/
├── config/
│   ├── base.toml             # 主配置
│   ├── plugin-state.toml     # 插件启停状态
│   ├── admin-audit.jsonl     # 管理操作审计记录
│   └── plugins/              # 插件配置
├── plugins/
│   └── bin/                  # Linux 动态插件
└── logs/                     # 供插件或外部采集使用
```

重建容器不会删除这些目录。不要执行会主动删除业务目录的脚本，也不要把 `data/` 加入 Git。动态插件必须针对容器的 Linux 系统和 CPU 架构构建；Windows 的 `.dll` 不能放进 Linux 容器使用。

### 6. 更新和回滚

先备份 `data/`，再把 `.env` 中的版本改为目标版本。假设下一版是 `0.1.16`：

```dotenv
QIMENBOT_TAG=0.1.16
```

拉取并重建：

```bash
docker compose --env-file deploy/docker/.env pull qimenbot
docker compose --env-file deploy/docker/.env up -d qimenbot
docker compose --env-file deploy/docker/.env ps
docker compose --env-file deploy/docker/.env logs --tail 100 qimenbot
```

使用 `latest` 时也必须先执行 `pull`。只运行 `up -d` 可能继续使用本地旧镜像。

回滚时把 `QIMENBOT_TAG` 改回备份对应的旧版本，重复上述命令。如果新版本已经改写配置或插件数据库，应先停止容器，再恢复同一时间点的 `data/` 备份。

::: warning 容器内不执行二进制更新
管理面板会把容器识别为“Docker 编排托管”。请在宿主机更新镜像，不要进入容器覆盖 `/usr/local/bin/qimenbotd`，也不要向容器挂载 Docker Socket。
:::

## Release 二进制

### 1. 选择安装包

在 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases) 下载与系统对应的压缩包：

| 系统 | Release 标识 |
| --- | --- |
| Windows 64 位 | `x86_64-pc-windows-msvc` |
| Linux 64 位，glibc | `x86_64-unknown-linux-gnu` |
| Linux ARM64，glibc | `aarch64-unknown-linux-gnu` |
| Linux 64 位，musl | `x86_64-unknown-linux-musl` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple 芯片 | `aarch64-apple-darwin` |

Linux 可用下面两条命令判断：

```bash
uname -m
ldd --version
```

`x86_64` 选择 x86_64 包，`aarch64` 或 `arm64` 选择 ARM64 包。Ubuntu、Debian、Rocky Linux 通常使用 `gnu`，Alpine Linux 使用 `musl`。Windows PowerShell 可执行以下命令确认架构：

```powershell
[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
```

当前 Windows Release 提供 X64 版本。

### 2. 校验下载文件

Release 同时发布原始 `qimenbotd`、`qimen-launcher` 和同名 `.sha256`。以 Linux x86_64 GNU 为例：

```bash
sha256sum -c qimenbotd-v0.1.15-x86_64-unknown-linux-gnu.sha256
sha256sum -c qimen-launcher-v0.1.15-x86_64-unknown-linux-gnu.sha256
```

Windows PowerShell：

```powershell
$file = ".\qimenbotd-v0.1.15-x86_64-pc-windows-msvc.exe"
$expected = (Get-Content "$file.sha256").Split()[0]
$actual = (Get-FileHash $file -Algorithm SHA256).Hash
if ($actual.ToLower() -ne $expected.ToLower()) { throw "SHA256 校验失败" }
```

`.sha256` 文件中的文件名必须与下载的原始二进制位于同一目录。Release 压缩包用于完整安装，原始二进制主要供校验和 launcher 更新使用。

### 3. 准备安装目录

解压后保留完整目录，不要只拿走 `qimenbotd`：

```text
qimenbot/
├── qimen-launcher             # Windows 文件名带 .exe
├── qimenbotd                  # Windows 文件名带 .exe
├── config/
│   ├── base.toml.example
│   ├── launcher.toml.example
│   ├── plugin-state.toml
│   └── plugins/
├── plugins/
│   └── bin/
└── .qimen-update/             # launcher 首次启动后创建
```

复制实际配置：

```bash
# Linux / macOS
cp -n config/base.toml.example config/base.toml
cp -n config/launcher.toml.example config/launcher.toml
chmod +x qimen-launcher qimenbotd
```

```powershell
# Windows PowerShell
Copy-Item .\config\base.toml.example .\config\base.toml -ErrorAction SilentlyContinue
Copy-Item .\config\launcher.toml.example .\config\launcher.toml -ErrorAction SilentlyContinue
```

编辑 `config/base.toml`，配置 QQ 官方 Bot 或 OneBot。配置中使用 `${QQBOT_APPID}` 这类占位符时，变量必须存在于启动 launcher 的终端或系统服务环境中：

```bash
export QQBOT_APPID='你的 AppID'
export QQBOT_SECRET='你的 Secret'
export QIMEN_ADMIN_TOKEN='至少 32 字节的随机字符串'
```

```powershell
$env:QQBOT_APPID = "你的 AppID"
$env:QQBOT_SECRET = "你的 Secret"
$env:QIMEN_ADMIN_TOKEN = "至少 32 字节的随机字符串"
```

### 4. 启动

```bash
# Linux / macOS
./qimen-launcher run --config ./config/launcher.toml
```

```powershell
# Windows PowerShell
.\qimen-launcher.exe run --config .\config\launcher.toml
```

另开一个终端访问 `http://127.0.0.1:3210/healthz`。健康检查通过后再测试机器人消息；如果面板需要从其他电脑访问，先按后文配置管理 Token、HTTPS 反向代理和防火墙。

以后始终启动 launcher。它负责启动 `qimenbotd`、转发日志、控制崩溃重启并执行受控更新。直接运行 `qimenbotd` 可以用于临时排障，但会绕过这些能力。

### 5. launcher 配置

`config/launcher.toml` 的主要选项如下：

```toml
[process]
working_dir = "."
restart_policy = "on-failure"  # never / on-failure / always
restart_delay_secs = 3
max_crash_restarts = 5          # 0 表示不限制
graceful_shutdown_secs = 30
startup_grace_secs = 3
health_url = "http://127.0.0.1:3210/healthz"
health_timeout_secs = 45

[update]
enabled = true
repository = "lvyunqi/QimenBot"
channel = "stable"             # stable / beta
auto_install = false
check_interval_secs = 21600
request_timeout_secs = 30
update_dir = ".qimen-update"
```

`auto_install = false` 表示只提示新版本，由管理员在面板确认。`stable` 忽略 GitHub 预发布版本；需要测试预发布版时再改为 `beta`。

命令行也可以投递更新和重启操作。launcher 必须已经在运行，并且命令使用同一份配置：

```bash
./qimen-launcher check --config ./config/launcher.toml
./qimen-launcher install --config ./config/launcher.toml
./qimen-launcher restart --config ./config/launcher.toml
```

这些命令显示 `queued` 表示操作已写入队列，实际进度在管理面板或 launcher 日志中查看。

### 6. 二进制在线更新

launcher 会执行以下流程：

1. 从配置指定的 GitHub 仓库读取 Release。
2. 根据操作系统和 CPU 架构选择 `qimenbotd` 资产。
3. 下载二进制和 `.sha256`，校验通过后才准备安装。
4. 通知当前运行时停止接收新任务并优雅退出。
5. 备份旧 daemon，原位替换 `qimenbotd`。
6. 启动新版本并检查 `/healthz` 中的版本号。
7. 启动失败、健康检查超时或版本不符时，自动恢复旧 daemon。

在线更新不修改 `config/`、`plugins/` 和插件数据库，也不会替换 launcher 本身。升级 launcher 时应下载新的完整压缩包，停止系统服务后手工覆盖 `qimen-launcher`。

自动备份只用于本次更新失败后的即时恢复；新版本通过健康检查后，临时备份会被清理。需要主动降级时，先停止服务，备份业务数据，用旧 Release 压缩包中的 `qimenbotd` 和 `qimen-launcher` 一起覆盖，并把旧的 `.qimen-update` 移走后再启动。

## Linux systemd

systemd 只管理 launcher，launcher 再管理 daemon。以下示例假设安装目录为 `/opt/qimenbot`。

### 1. 创建用户和目录

```bash
sudo useradd --system --home /opt/qimenbot --shell /usr/sbin/nologin qimenbot
sudo install -d -o qimenbot -g qimenbot /opt/qimenbot
sudo tar -xzf qimenbot-v0.1.15-x86_64-unknown-linux-gnu.tar.gz \
  -C /opt/qimenbot --strip-components=1
sudo -u qimenbot cp -n /opt/qimenbot/config/base.toml.example /opt/qimenbot/config/base.toml
sudo -u qimenbot cp -n /opt/qimenbot/config/launcher.toml.example /opt/qimenbot/config/launcher.toml
sudo chmod 0755 /opt/qimenbot/qimenbotd /opt/qimenbot/qimen-launcher
sudo chown -R qimenbot:qimenbot /opt/qimenbot
```

### 2. 保存服务环境变量

创建 `/opt/qimenbot/.env`：

```dotenv
QQBOT_APPID=你的AppID
QQBOT_SECRET=你的Secret
QIMEN_ADMIN_TOKEN=随机管理Token
RUST_LOG=info
```

没有使用的变量可以删除。限制文件权限：

```bash
sudo chown root:qimenbot /opt/qimenbot/.env
sudo chmod 0640 /opt/qimenbot/.env
```

systemd 的环境文件不是 Shell 脚本，不要写 `export`。值中包含空格时使用双引号。

### 3. 创建服务

保存为 `/etc/systemd/system/qimenbot.service`：

```ini
[Unit]
Description=QimenBot launcher
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=qimenbot
Group=qimenbot
WorkingDirectory=/opt/qimenbot
EnvironmentFile=-/opt/qimenbot/.env
ExecStart=/opt/qimenbot/qimen-launcher run --config /opt/qimenbot/config/launcher.toml
Restart=on-failure
RestartSec=5
TimeoutStopSec=45
UMask=0077
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/qimenbot

[Install]
WantedBy=multi-user.target
```

`ReadWritePaths=/opt/qimenbot` 不能缩小到只有 `config/`。launcher 在线更新时需要替换安装目录中的 `qimenbotd`。

加载并启动：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now qimenbot
sudo systemctl status qimenbot --no-pager
sudo journalctl -u qimenbot -n 200 --no-pager
```

持续查看日志：

```bash
sudo journalctl -u qimenbot -f
```

从命令行让常驻 launcher 检查更新时，使用服务账号写入命令队列：

```bash
sudo -u qimenbot /opt/qimenbot/qimen-launcher check \
  --config /opt/qimenbot/config/launcher.toml
```

修改 service 文件后执行 `systemctl daemon-reload`。修改 `.env` 后执行 `systemctl restart qimenbot`。只修改 `base.toml` 时，可以在管理面板重启运行时，也可以重启 systemd 服务。

## Windows Service

Windows 不能直接把普通控制台程序注册成服务，可以使用 NSSM 或 WinSW。两种方案都只包装 `qimen-launcher.exe`。

### 方案一：NSSM

以管理员身份打开 PowerShell。假设 NSSM 已加入 `PATH`，QimenBot 位于 `C:\QimenBot`：

```powershell
New-Item -ItemType Directory -Force C:\QimenBot\logs | Out-Null
nssm install QimenBot "C:\QimenBot\qimen-launcher.exe"
nssm set QimenBot AppParameters "run --config C:\QimenBot\config\launcher.toml"
nssm set QimenBot AppDirectory "C:\QimenBot"
nssm set QimenBot AppStdout "C:\QimenBot\logs\launcher.log"
nssm set QimenBot AppStderr "C:\QimenBot\logs\launcher-error.log"
nssm set QimenBot AppRotateFiles 1
nssm set QimenBot AppRotateOnline 1
nssm set QimenBot AppRotateBytes 10485760
nssm set QimenBot AppExit Default Restart
nssm set QimenBot AppRestartDelay 5000
nssm set QimenBot Start SERVICE_AUTO_START
```

Windows 服务不会继承当前 PowerShell 的 `$env:` 临时变量。可以把变量写入系统环境后再启动服务：

```powershell
[Environment]::SetEnvironmentVariable("QQBOT_APPID", "你的AppID", "Machine")
[Environment]::SetEnvironmentVariable("QQBOT_SECRET", "你的Secret", "Machine")
[Environment]::SetEnvironmentVariable("QIMEN_ADMIN_TOKEN", "随机管理Token", "Machine")
nssm start QimenBot
Get-Service QimenBot
```

系统环境变量对本机管理员可见。生产主机应限制管理员账号和 `C:\QimenBot` 的 ACL，不要把凭据写进批处理文件。

常用命令：

```powershell
nssm restart QimenBot
nssm stop QimenBot
nssm edit QimenBot
```

### 方案二：WinSW

把 WinSW 可执行文件重命名为 `QimenBotService.exe`，与下面的 `QimenBotService.xml` 一起放入 `C:\QimenBot`：

```xml
<service>
  <id>QimenBot</id>
  <name>QimenBot</name>
  <description>QimenBot launcher service</description>
  <executable>%BASE%\qimen-launcher.exe</executable>
  <arguments>run --config "%BASE%\config\launcher.toml"</arguments>
  <workingdirectory>%BASE%</workingdirectory>
  <startmode>Automatic</startmode>
  <onfailure action="restart" delay="10 sec" />
  <log mode="roll-by-size">
    <sizeThreshold>10240</sizeThreshold>
    <keepFiles>8</keepFiles>
  </log>
</service>
```

管理员 PowerShell 中执行：

```powershell
cd C:\QimenBot
.\QimenBotService.exe install
.\QimenBotService.exe start
.\QimenBotService.exe status
```

环境变量可以使用前面的系统环境变量，也可以按 WinSW 文档添加 `<env>` 节点。不要同时安装 NSSM 和 WinSW 服务。

## 从源码构建

源码构建需要 Rust 1.89+、Node.js 22、npm 和 Git。管理面板会嵌入 `qimenbotd`，因此必须先构建前端。

```bash
git clone https://github.com/lvyunqi/QimenBot.git
cd QimenBot

npm --prefix web/admin ci
npm --prefix web/admin run build

cargo build --release --locked --package qimenbotd --package qimen-launcher
```

构建结果：

```text
target/release/qimenbotd
target/release/qimen-launcher
```

Windows 文件名带 `.exe`。如果跳过前端构建，Rust 构建仍可能成功，但管理面板只会显示“Admin UI has not been built”，不适合作为生产包。

本地开发可在仓库根目录直接启动 daemon：

```bash
cargo run --package qimenbotd
```

它默认读取 `config/base.toml`。需要指定其他配置时设置 `QIMEN_CONFIG_PATH`：

```bash
QIMEN_CONFIG_PATH=/绝对路径/base.toml cargo run --package qimenbotd
```

```powershell
$env:QIMEN_CONFIG_PATH = "C:\QimenBot\config\base.toml"
cargo run --package qimenbotd
```

源码构建用于生产时，不要直接把整个 `target/` 复制到服务器。按 Release 的目录结构准备一个干净安装目录，复制两个 release 二进制、`config/base.toml`、`config/launcher.toml.example`、`config/plugin-state.toml` 和插件目录，然后按二进制部署章节启动 launcher。

动态插件是独立 workspace，需要进入各自目录构建。插件的操作系统、CPU 架构和 C ABI 必须与部署主机一致。

## HTTPS 反向代理与防火墙

管理面板包含配置修改、插件管理和更新操作。远程访问时至少应具备三层保护：管理 Token、HTTPS、来源地址限制。

### Nginx 示例

二进制部署时让 `admin_web.bind` 保持 `127.0.0.1:3210`。Docker 部署时把 Compose 端口改为只绑定宿主机回环地址：

```yaml
ports:
  - "127.0.0.1:3210:3210"
```

然后由 Nginx 对外提供 HTTPS：

```nginx
server {
    listen 80;
    server_name bot.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name bot.example.com;

    ssl_certificate     /etc/letsencrypt/live/bot.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/bot.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:3210;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_read_timeout 3600s;
    }

    # 实时日志使用 SSE，关闭代理缓冲才能及时显示。
    location = /api/v1/logs/stream {
        proxy_pass http://127.0.0.1:3210;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
    }

    # launcher 和本机监控直接访问后端，不把健康信息暴露到公网。
    location = /healthz {
        return 404;
    }
}
```

证书签发和自动续期按 Certbot 或所在面板的文档配置。代理完成后只对公网开放 `80/443`，不要再开放 `3210`。

### 防火墙原则

- SSH、RDP 等管理端口只允许可信来源。
- 管理面板经反向代理访问时，公网只开放 `80/443`。
- `6701` 和 `8088` 默认关闭；确需开放时限制来源 IP。
- 保留出站 DNS、HTTPS 和 WSS，QQ 官方 Gateway 和 GitHub 更新需要访问外网。
- `/healthz` 不需要 Token，应只供本机、容器编排或监控网络访问。

Ubuntu 使用 UFW 时，可以先保留 SSH，再开放反向代理端口：

```bash
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw status verbose
```

确认 SSH 规则和云厂商安全组都正确后再启用 UFW。不要为管理面板额外添加 `3210/tcp` 公网规则。

## 日志、健康检查与监控

QimenBot 的框架日志默认写到标准输出。不同部署方式的查看方法如下：

| 部署方式 | 日志命令或位置 |
| --- | --- |
| Docker Compose | `docker compose --env-file deploy/docker/.env logs -f qimenbot` |
| systemd | `journalctl -u qimenbot -f` |
| NSSM | 示例中的 `C:\QimenBot\logs\launcher*.log` |
| WinSW | WinSW 生成的滚动日志文件 |
| 前台运行 | 当前终端 |

管理面板的“实时日志”是内存缓冲区，重启后会清空；`admin-audit.jsonl` 记录的是管理操作审计，不是完整运行日志。需要长期留存时，使用 journald、Docker 日志驱动、Loki 或其他日志采集器。

健康检查：

```bash
curl --fail --silent http://127.0.0.1:3210/healthz
```

建议每分钟检查一次，连续三次失败再告警，避免把短暂重启当成持续故障。监控端还应观察进程或容器是否反复重启、QQ Gateway 是否持续重连，以及磁盘是否被日志占满。

## 备份与恢复

### Docker

最稳妥的备份方式是短暂停止容器，保证插件数据库已经落盘：

```bash
docker compose --env-file deploy/docker/.env stop qimenbot
tar -czf "qimenbot-data-$(date +%Y%m%d-%H%M%S).tar.gz" data deploy/docker/.env
docker compose --env-file deploy/docker/.env start qimenbot
```

Windows PowerShell：

```powershell
docker compose --env-file deploy/docker/.env stop qimenbot
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
Compress-Archive -Path data,deploy/docker/.env -DestinationPath "qimenbot-data-$stamp.zip"
docker compose --env-file deploy/docker/.env start qimenbot
```

环境变量文件包含明文凭据，备份文件应加密并限制读取权限。恢复时使用与备份相同的镜像 Tag，解压 `data/` 和 `.env` 后再启动；确认运行正常后再升级。

### 二进制

停止 launcher 或系统服务后，备份以下内容：

| 路径 | 内容 |
| --- | --- |
| `config/base.toml` | Bot、模块和管理面板配置 |
| `config/launcher.toml` | 进程监督与更新配置 |
| `config/plugin-state.toml` | 插件启停状态 |
| `config/plugins/` | 动态插件配置 |
| `plugins/bin/` | 动态插件二进制 |
| `.env` 或系统服务环境 | AppID、Secret、管理 Token |
| 插件创建的 `*.db` 和数据目录 | 插件业务数据 |

最简单的整机备份是在停止服务后归档整个安装目录：

```bash
sudo systemctl stop qimenbot
sudo tar -czf "/var/backups/qimenbot-$(date +%Y%m%d-%H%M%S).tar.gz" -C /opt qimenbot
sudo systemctl start qimenbot
```

Windows 使用 NSSM 时，可以先执行 `nssm stop QimenBot`，再把 `C:\QimenBot` 整个目录压缩到其他磁盘，完成后执行 `nssm start QimenBot`。

还原时先安装备份对应版本的完整 Release，再覆盖上述业务数据。SQLite 数据库不要在进程写入时直接复制；无法停机时应使用 SQLite 在线备份能力或插件提供的导出命令。

`target/`、Docker 镜像层和 `.qimen-update/staging/` 都可以重新生成，不需要进入业务备份。

## 更新时会不会断线

单实例更新需要断开 QQ Gateway 或 OneBot WebSocket，并在新进程启动后重新连接。launcher 会先优雅关闭再启动，通常是数秒中断，但不保证严格零停机。

同一个 QQ 官方 Bot 是否允许并行 Gateway 会话由官方平台控制。在没有验证事件去重、会话接管和并发连接限制前，不要用两个实例同时登录来模拟滚动更新。

## 常见故障

| 现象 | 先检查什么 | 处理方法 |
| --- | --- | --- |
| Compose 提示变量未设置 | `deploy/docker/.env` 路径和名称 | 命令必须带 `--env-file deploy/docker/.env`，并填写 `QIMEN_ADMIN_TOKEN` |
| 容器一直 `unhealthy` | `docker compose ... logs --tail 200` | 检查配置语法、端口占用、QQ 凭据和 `/healthz` |
| 面板打不开 | 监听地址、端口映射、防火墙 | 本机先访问 `127.0.0.1:3210/healthz`，再逐层检查代理和防火墙 |
| 源码构建后面板显示未构建 | `web/admin/dist` 是否存在 | 先执行 `npm --prefix web/admin ci` 和 `npm --prefix web/admin run build`，再重新构建 Rust |
| OneBot 在宿主机但容器连不上 | endpoint 是否写了 `127.0.0.1` | 改用 `host.docker.internal`，并确认 OneBot 监听地址允许 Docker 网桥访问 |
| QQ 官方 Bot 鉴权失败 | AppID、Secret、沙箱环境 | 重新复制开放平台凭据，确认没有多余空格，并核对沙箱配置 |
| 私聊正常、群消息不进入 | QQ 开放平台事件权限和 Intents | 按[官方 QQ Bot 接入](/guide/qq-official-quickstart)检查群事件权限、`GROUP_AND_C2C_EVENT` 和群内 @ 测试方式 |
| launcher 命令显示 `queued` 但没有动作 | launcher 是否常驻、配置是否一致 | 给 `check/install/restart` 传入与运行服务相同的 `--config` 路径，检查 `.qimen-update` 权限 |
| 更新后自动回滚 | launcher 日志和 `/healthz` 版本 | 检查新进程启动错误、管理端口和健康检查超时，不要反复强制安装 |
| 动态插件加载失败 | 系统、架构、API 版本 | Linux 容器使用对应架构的 `.so`；Windows 使用 `.dll`；重新按目标环境构建 |
| `Address already in use` | 哪个进程占用端口 | Linux 用 `ss -ltnp`，Windows 用 `Get-NetTCPConnection -LocalPort 3210` |

排障时不要先删除 `data/`、`config/` 或 `.qimen-update/`。先保存日志和配置副本，再做可逆改动。

## 维护者：发布 Docker Hub 镜像

普通使用者不需要执行本节。仓库中的 `.github/workflows/docker-publish.yml` 会在推送 `v*` Tag 时构建并发布多架构镜像。

### 1. 创建 Docker Hub 仓库和令牌

在 [Docker Hub](https://hub.docker.com/) 创建 Public 仓库，例如 `mryunqi/qimenbot`。Docker Personal 可以免费发布公共镜像，但拉取和构建仍受 Docker Hub 的公平使用限制。

进入 **Account settings > Personal access tokens** 创建令牌。令牌至少需要 Read、Write 权限；不需要把账号密码交给 GitHub。令牌只完整显示一次，应放入密码管理器。

### 2. 配置 GitHub Environment

工作流使用名为 `docker hub` 的 GitHub Environment。进入仓库：

**Settings > Environments > docker hub > Environment secrets**

添加：

| Secret | 内容 |
| --- | --- |
| `DOCKERHUB_USERNAME` | Docker Hub 用户名，例如 `mryunqi` |
| `DOCKERHUB_TOKEN` | Docker Hub Personal Access Token |

在同一 Environment 的 Variables 中可选添加：

```text
DOCKERHUB_IMAGE=mryunqi/qimenbot
```

未设置 `DOCKERHUB_IMAGE` 时，工作流使用 `<DOCKERHUB_USERNAME>/qimenbot`。GitHub 仓库所有者与 Docker Hub 用户名可以不同。

### 3. 发布规则

推送 `v0.1.15` 后会生成：

```text
mryunqi/qimenbot:0.1.15
mryunqi/qimenbot:0.1
mryunqi/qimenbot:latest
mryunqi/qimenbot:sha-<提交摘要>
```

每个版本 Tag 都是同时包含 `linux/amd64` 和 `linux/arm64` 的 manifest。`workflow_dispatch` 可用于测试工作流，但从普通分支手工运行时通常只生成 `sha-*` Tag，不应替代正式版本 Tag。

发布后检查：

```bash
docker buildx imagetools inspect mryunqi/qimenbot:0.1.15
docker pull mryunqi/qimenbot:0.1.15
docker image inspect mryunqi/qimenbot:0.1.15 --format '{{.Architecture}} {{.Os}}'
```

若登录阶段出现 `unauthorized`，确认 Secret 中放的是 Docker Hub Token、用户名属于镜像 Namespace，且令牌仍为 Active。构建阶段失败则先看失败平台；一个架构失败时，多架构 manifest 不会发布完整。
