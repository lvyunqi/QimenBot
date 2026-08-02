# 生产环境运维

本页接着[部署指南](/guide/deployment)，说明二进制服务化、HTTPS、日志和完整备份。刚开始试用时不需要一次做完；确认机器人正常收发消息后再配置。

## Linux systemd

systemd 只管理根目录的 `qimenbot`，它再管理 `runtime/qimenbotd`。以下示例使用 `/opt/qimenbot`。

### 准备目录

```bash
sudo useradd --system --home /opt/qimenbot --shell /usr/sbin/nologin qimenbot
sudo install -d -o qimenbot -g qimenbot /opt/qimenbot
sudo tar -xzf QimenBot-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz \
  -C /opt/qimenbot --strip-components=1
sudo -u qimenbot cp -n /opt/qimenbot/config/base.toml.example /opt/qimenbot/config/base.toml
sudo -u qimenbot cp -n /opt/qimenbot/config/qimenbot.toml.example /opt/qimenbot/config/qimenbot.toml
sudo chmod 0755 /opt/qimenbot/qimenbot /opt/qimenbot/runtime/qimenbotd
sudo chown -R qimenbot:qimenbot /opt/qimenbot
```

创建 `/opt/qimenbot/.env`：

```dotenv
QQBOT_APPID=你的AppID
QQBOT_SECRET=你的Secret
QIMEN_ADMIN_TOKEN=随机管理Token
RUST_LOG=info
```

`config/base.toml` 中的对应字段要写成 `${QQBOT_APPID}`、`${QQBOT_SECRET}` 和 `${QIMEN_ADMIN_TOKEN}`，否则只设置环境变量不会覆盖 TOML。

```bash
sudo chown root:qimenbot /opt/qimenbot/.env
sudo chmod 0640 /opt/qimenbot/.env
```

systemd 环境文件不写 `export`。值中包含空格时使用双引号。

### 创建服务

保存为 `/etc/systemd/system/qimenbot.service`：

```ini
[Unit]
Description=QimenBot
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=qimenbot
Group=qimenbot
WorkingDirectory=/opt/qimenbot
EnvironmentFile=-/opt/qimenbot/.env
ExecStart=/opt/qimenbot/qimenbot run
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

更新时需要替换 `runtime/qimenbotd`，因此 `ReadWritePaths` 要包含整个 `/opt/qimenbot`。

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now qimenbot
sudo systemctl status qimenbot --no-pager
sudo journalctl -u qimenbot -f
```

命令行检查更新时使用服务账号：

```bash
sudo -u qimenbot /opt/qimenbot/qimenbot check
```

修改 service 文件后执行 `systemctl daemon-reload`；修改 `.env` 后执行 `systemctl restart qimenbot`。

## Windows Service

Windows 可以用 NSSM 或 WinSW 包装 `qimenbot.exe`。不要再为 `runtime/qimenbotd.exe` 单独创建服务。

### NSSM

管理员 PowerShell 中执行：

```powershell
New-Item -ItemType Directory -Force C:\QimenBot\logs | Out-Null
nssm install QimenBot "C:\QimenBot\qimenbot.exe"
nssm set QimenBot AppParameters "run"
nssm set QimenBot AppDirectory "C:\QimenBot"
nssm set QimenBot AppStdout "C:\QimenBot\logs\qimenbot.log"
nssm set QimenBot AppStderr "C:\QimenBot\logs\qimenbot-error.log"
nssm set QimenBot AppRotateFiles 1
nssm set QimenBot AppRotateOnline 1
nssm set QimenBot AppRotateBytes 10485760
nssm set QimenBot AppExit Default Restart
nssm set QimenBot AppRestartDelay 5000
nssm set QimenBot Start SERVICE_AUTO_START
```

Windows 服务不会继承当前终端的临时 `$env:`。需要使用环境变量时写入 Machine 范围，然后启动服务：

```powershell
[Environment]::SetEnvironmentVariable("QQBOT_APPID", "你的AppID", "Machine")
[Environment]::SetEnvironmentVariable("QQBOT_SECRET", "你的Secret", "Machine")
[Environment]::SetEnvironmentVariable("QIMEN_ADMIN_TOKEN", "随机管理Token", "Machine")
nssm start QimenBot
Get-Service QimenBot
```

同样要在 `config/base.toml` 中使用 `${变量名}` 占位符。

系统环境变量对本机管理员可见，应限制管理员账号和 `C:\QimenBot` 的 ACL。

### WinSW

把 WinSW 重命名为 `QimenBotService.exe`，并在同目录创建 `QimenBotService.xml`：

```xml
<service>
  <id>QimenBot</id>
  <name>QimenBot</name>
  <description>QimenBot service</description>
  <executable>%BASE%\qimenbot.exe</executable>
  <arguments>run</arguments>
  <workingdirectory>%BASE%</workingdirectory>
  <startmode>Automatic</startmode>
  <onfailure action="restart" delay="10 sec" />
  <log mode="roll-by-size">
    <sizeThreshold>10240</sizeThreshold>
    <keepFiles>8</keepFiles>
  </log>
</service>
```

```powershell
Set-Location C:\QimenBot
.\QimenBotService.exe install
.\QimenBotService.exe start
.\QimenBotService.exe status
```

NSSM 和 WinSW 二选一。

## HTTPS 反向代理

远程管理至少要有管理 Token、HTTPS 和防火墙限制。二进制部署让 `admin_web.bind` 保持 `127.0.0.1:3210`；Docker 把端口改为只绑定宿主机回环地址：

```yaml
ports:
  - "127.0.0.1:3210:3210"
```

Nginx 示例：

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

    location = /api/v1/logs/stream {
        proxy_pass http://127.0.0.1:3210;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 3600s;
    }

    location = /healthz {
        return 404;
    }
}
```

实时日志使用 SSE，`/api/v1/logs/stream` 必须关闭代理缓冲。证书签发和续期按 Certbot 或服务器面板文档配置。

公网只开放 `80/443`。`3210` 不直接开放，`6701` 和 `8088` 只在确实使用时按来源 IP 放行。`/healthz` 无需 Token，应只给本机或监控网络访问。

## 日志和健康检查

| 部署方式 | 查看日志 |
| --- | --- |
| Docker Compose | `docker compose --env-file deploy/docker/.env logs -f qimenbot` |
| systemd | `journalctl -u qimenbot -f` |
| NSSM | `C:\QimenBot\logs\qimenbot*.log` |
| WinSW | WinSW 生成的滚动日志 |

管理面板的实时日志是内存缓冲区，重启后会清空。`admin-audit.jsonl` 只记录管理操作，不是完整运行日志。

```bash
curl --fail --silent http://127.0.0.1:3210/healthz
```

监控可每分钟检查一次，连续三次失败再告警，同时观察进程反复重启、Gateway 持续重连和磁盘占用。

## 完整备份与恢复

Docker 停止容器后备份 `.env` 和所有映射目录。二进制部署停止 `qimenbot` 后备份整个安装目录。

```bash
docker compose --env-file deploy/docker/.env stop qimenbot
tar -czf "qimenbot-data-$(date +%Y%m%d-%H%M%S).tar.gz" data deploy/docker/.env
docker compose --env-file deploy/docker/.env start qimenbot
```

```bash
sudo systemctl stop qimenbot
sudo tar -czf "/var/backups/qimenbot-$(date +%Y%m%d-%H%M%S).tar.gz" -C /opt qimenbot
sudo systemctl start qimenbot
```

SQLite 数据库不要在写入时直接复制。备份包含 Secret 和管理 Token，应加密并限制读取权限。恢复时先安装备份对应的程序或镜像版本，再恢复配置、插件和数据库，确认运行正常后再升级。

## 更新中断说明

单实例更新需要断开 QQ Gateway 或 OneBot WebSocket，再由新进程重新连接，通常会中断数秒。`qimenbot` 会优雅关闭并在新版本健康检查失败时自动回滚，但不保证严格零停机。

同一 QQ 官方 Bot 的并行 Gateway 会话受官方平台限制。在没有验证事件去重和会话接管前，不要同时启动两个实例模拟滚动更新。
