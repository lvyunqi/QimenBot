# 部署与更新

QimenBot 提供两种生产部署方式：

- **Docker**：适合已经使用 Docker Compose、Portainer 或 Kubernetes 的环境。更新由容器编排层完成。
- **预编译二进制**：适合 Windows 桌面、Windows Service 和普通 Linux 主机。由 `qimen-launcher` 监督主程序并执行受控更新。

两种方式不要混用。容器内不会替换自身二进制，二进制 launcher 也不会调用 Docker 命令。

## 二进制部署

### 选择安装包

在 [GitHub Releases](https://github.com/lvyunqi/QimenBot/releases) 下载与系统对应的压缩包。

| 系统 | Release 标识 |
| --- | --- |
| Windows 64 位 | `x86_64-pc-windows-msvc` |
| Linux 64 位，glibc | `x86_64-unknown-linux-gnu` |
| Linux ARM64，glibc | `aarch64-unknown-linux-gnu` |
| Linux 64 位，musl | `x86_64-unknown-linux-musl` |
| macOS Intel | `x86_64-apple-darwin` |
| macOS Apple 芯片 | `aarch64-apple-darwin` |

不确定 Linux 使用哪一种时，先执行 `ldd --version`。常见的 Ubuntu、Debian、Rocky Linux 使用 `gnu`；Alpine Linux 使用 `musl`。

### 安装目录

解压后保留以下结构，不要只拿走 `qimenbotd`：

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
└── .qimen-update/             # 首次启动 launcher 后自动创建
```

首次启动时，程序会从 `.example` 文件创建实际配置。编辑 `config/base.toml`，填入机器人连接信息，再启动 launcher：

```powershell
# Windows PowerShell
.\qimen-launcher.exe run
```

```bash
# Linux / macOS
chmod +x qimen-launcher qimenbotd
./qimen-launcher run
```

以后都应启动 `qimen-launcher`，不要再把 `qimenbotd` 注册成系统服务。launcher 会启动主程序、记录更新状态，并在异常退出时按配置重新拉起。

### launcher 配置

实际配置文件是 `config/launcher.toml`。主要选项如下：

```toml
[process]
working_dir = "."
restart_policy = "on-failure"
restart_delay_secs = 3
max_crash_restarts = 5
graceful_shutdown_secs = 30
health_url = "http://127.0.0.1:3210/healthz"
health_timeout_secs = 45

[update]
enabled = true
repository = "lvyunqi/QimenBot"
channel = "stable"
auto_install = false
check_interval_secs = 21600
```

`auto_install = false` 表示发现新版本后只在管理面板提示，由管理员确认安装。建议先保持默认值，确认更新和回滚在自己的环境中工作正常后，再考虑改为 `true`。

管理面板的“版本更新”页面可以执行检查、安装和优雅重启。也可以在命令行投递相同操作：

```bash
qimen-launcher check
qimen-launcher install
qimen-launcher restart
```

### 更新过程

launcher 按以下顺序安装新版本：

1. 从配置指定的 GitHub 仓库读取 Release。
2. 根据当前系统和 CPU 选择名称中带 target triple 的资产。
3. 下载 `qimenbotd` 和同名 `.sha256` 文件。
4. 校验 SHA256，校验失败时不停止当前机器人。
5. 通知运行时停止接收新任务并优雅退出。
6. 备份旧的 `qimenbotd`，替换为新版本。
7. 启动新版本并请求 `/healthz`，确认返回的版本号正确。
8. 健康检查失败时恢复备份并重新启动旧版本。

`config/`、`plugins/`、数据库和日志不在替换范围内。launcher 自身作为稳定引导层不会在线替换；需要升级 launcher 时，先停止服务，再用新 Release 压缩包中的 launcher 覆盖旧文件。

## Linux systemd

下面的服务由 systemd 管理 launcher，launcher 再管理 `qimenbotd`：

```ini
# /etc/systemd/system/qimenbot.service
[Unit]
Description=QimenBot
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=qimenbot
Group=qimenbot
WorkingDirectory=/opt/qimenbot
ExecStart=/opt/qimenbot/qimen-launcher run --config /opt/qimenbot/config/launcher.toml
Restart=on-failure
RestartSec=5
TimeoutStopSec=40
EnvironmentFile=-/opt/qimenbot/.env
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=/opt/qimenbot/config /opt/qimenbot/plugins /opt/qimenbot/.qimen-update /opt/qimenbot/logs

[Install]
WantedBy=multi-user.target
```

创建专用用户并启动：

```bash
sudo useradd --system --home /opt/qimenbot --shell /usr/sbin/nologin qimenbot
sudo chown -R qimenbot:qimenbot /opt/qimenbot
sudo systemctl daemon-reload
sudo systemctl enable --now qimenbot
sudo journalctl -u qimenbot -f
```

修改 service 文件后要再次执行 `systemctl daemon-reload`。只修改 `base.toml` 时不需要 reload，通过面板或 `qimen-launcher restart` 重启运行时即可。

## Windows Service

Windows 本身不能直接把普通控制台程序注册为服务，可以使用 NSSM 或 WinSW。无论使用哪一种，服务程序都应指向 `qimen-launcher.exe`：

```text
Application:      C:\QimenBot\qimen-launcher.exe
Arguments:        run --config C:\QimenBot\config\launcher.toml
Startup directory: C:\QimenBot
```

不要把 `qimenbotd.exe` 和 launcher 分别注册成两个服务，否则更新时会出现两个监督者同时拉起进程。

## 上传到 Docker Hub

这一节面向项目维护者。普通使用者可以直接跳到“使用 Docker Compose”。

### 1. 创建公共仓库

登录 [Docker Hub](https://hub.docker.com/)，依次进入 **My Hub > Repositories > Create repository**：

- Namespace：选择自己的 Docker Hub 用户名或组织。
- Repository name：建议填写 `qimenbot`。
- Visibility：选择 **Public**。

公共仓库可由 Docker Personal 免费发布，但仍受 Docker Hub 的公平使用和拉取频率限制。

### 2. 创建访问令牌

进入 **Account settings > Personal access tokens**，创建具备读写权限的令牌。令牌只会完整显示一次，不要把它写进仓库文件。

### 3. 配置 GitHub Actions

进入 GitHub 仓库的 **Settings > Secrets and variables > Actions**，添加两个 Repository secret：

| 名称 | 内容 |
| --- | --- |
| `DOCKERHUB_USERNAME` | Docker Hub 用户名 |
| `DOCKERHUB_TOKEN` | 上一步创建的访问令牌 |

如果镜像不是默认的 `<用户名>/qimenbot`，再到 **Variables** 添加：

```text
DOCKERHUB_IMAGE=组织名/镜像名
```

仓库中的 `.github/workflows/docker-publish.yml` 会在推送 `v*` Tag 时构建并上传 `linux/amd64`、`linux/arm64` 两种架构。假设 Tag 为 `v0.2.0`，将产生：

```text
用户名/qimenbot:0.2.0
用户名/qimenbot:0.2
用户名/qimenbot:latest
用户名/qimenbot:sha-提交摘要
```

发布使用的是 Docker Hub Token，不是账号密码。GitHub Actions 日志不会显示 secret 原文。

## 使用 Docker Compose

### 1. 准备环境变量

仓库已经提供 `compose.yaml` 和环境变量模板：

```powershell
# Windows PowerShell
Copy-Item deploy/docker/.env.example deploy/docker/.env
```

```bash
# Linux / macOS
cp deploy/docker/.env.example deploy/docker/.env
```

编辑 `deploy/docker/.env`，至少修改：

```dotenv
QIMEN_ADMIN_TOKEN=一段足够长的随机字符串
QQBOT_APPID=QQ开放平台AppID
QQBOT_SECRET=QQ开放平台Secret
```

真实 `.env` 已被 Git 忽略。不要把 QQ Secret 写进 `compose.yaml` 或提交到仓库。

### 2. 启动

```bash
docker compose --env-file deploy/docker/.env up -d
docker compose --env-file deploy/docker/.env ps
docker compose --env-file deploy/docker/.env logs -f qimenbot
```

面板地址是 `http://服务器地址:3210/`。首次进入时输入 `QIMEN_ADMIN_TOKEN`。

持久化数据位于根目录 `data/`：

```text
data/
├── config/       # 主配置、插件状态、审计记录
├── plugins/bin/  # 动态插件
└── logs/
```

删除或重建容器不会删除这些目录。升级镜像时不要删除 `data/`。

### 3. 更新镜像

使用固定版本最容易回滚。先在 `.env` 中修改 `QIMENBOT_TAG`，再执行：

```bash
docker compose --env-file deploy/docker/.env pull qimenbot
docker compose --env-file deploy/docker/.env up -d qimenbot
docker compose --env-file deploy/docker/.env ps
```

使用 `latest` 时也要先执行 `pull`，仅执行 `up -d` 不保证重新拉取镜像。

需要回滚时，把 `QIMENBOT_TAG` 改回上一个版本并重复上述命令。配置格式发生变化前应先备份 `data/config/`。

::: warning 不要在容器里点击安装
管理面板会识别 Docker 环境，并把版本更新标记为“Docker 编排托管”。容器中没有 Docker Socket，也不会自行覆盖 `/usr/local/bin/qimenbotd`。这是为了避免容器状态与镜像不一致。
:::

## 更新期间是否会断线

单实例更新时，QQ Gateway 或 OneBot WebSocket 需要断开并重新连接。launcher 会先优雅关闭并尽快重启，通常只有几秒不可用，但不能承诺严格零中断。

同一个 QQ 官方 Bot 是否允许两个 Gateway 会话并行，受官方平台限制。没有完成事件去重、会话接管和并发连接验证前，不建议用双实例滚动更新来模拟零停机。

## 健康检查与备份

健康端点不需要管理 Token，只返回运行状态和版本：

```bash
curl http://127.0.0.1:3210/healthz
```

需要备份的内容：

| 路径 | 内容 | 建议时机 |
| --- | --- | --- |
| `config/base.toml` 或 `data/config/base.toml` | 机器人与宿主配置 | 每次修改后 |
| `config/plugin-state.toml` | 插件启停状态 | 每日 |
| `config/plugins/` | 动态插件配置 | 每次修改后 |
| `plugins/bin/` | 动态插件二进制 | 每次更新后 |
| `*.db` | 插件数据库 | 按数据重要程度安排 |

`target/`、`.qimen-update/staging/` 和 Docker 镜像层都可以重新生成，不应作为业务数据备份。
