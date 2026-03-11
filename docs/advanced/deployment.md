# 部署指南

本页介绍如何将 QimenBot 部署到生产环境。

## 编译发布版本

```bash
# Release 模式编译（优化后体积更小、运行更快）
cargo build --release

# 编译产物在 target/release/qimenbotd
```

::: tip 编译优化
Release 模式启用了所有优化（LTO、代码剥离等），二进制文件体积和运行性能都比 Debug 模式好很多。
:::

## 目录结构

推荐的部署目录结构：

```
/opt/qimenbot/
├── qimenbotd              # 可执行文件
├── config/
│   ├── base.toml          # 主配置
│   └── plugin-state.toml  # 插件状态（自动管理）
├── plugins/
│   └── bin/               # 动态插件目录
│       ├── libmy_plugin.so
│       └── ...
└── logs/                  # 日志目录（可选）
```

## 生产配置

### 配置模板

```toml
[runtime]
env = "prod"
shutdown_timeout_secs = 30     # 生产环境给更多关闭时间
task_grace_secs = 10

[observability]
level = "info"                 # 生产用 info 或 warn
json_logs = true               # JSON 日志便于采集

[official_host]
builtin_modules = ["command", "admin", "scheduler"]
plugin_modules  = ["your-plugin"]
plugin_state_path = "config/plugin-state.toml"
plugin_bin_dir = "plugins/bin"

[[bots]]
id        = "production-bot"
protocol  = "onebot11"
transport = "ws-forward"
endpoint  = "${ONEBOT_ENDPOINT}"    # 使用环境变量
access_token = "${ONEBOT_TOKEN}"    # 使用环境变量
enabled   = true
owners    = ["${BOT_OWNER}"]

[bots.limiter]
enable = true
rate = 5.0
capacity = 10
```

### 环境变量

通过环境变量管理敏感配置：

```bash
export ONEBOT_ENDPOINT="ws://127.0.0.1:3001"
export ONEBOT_TOKEN="your-secret-token"
export BOT_OWNER="123456"
```

## systemd 服务

### 创建服务文件

```ini
# /etc/systemd/system/qimenbot.service
[Unit]
Description=QimenBot - Rust Bot Framework
After=network.target

[Service]
Type=simple
User=qimenbot
Group=qimenbot
WorkingDirectory=/opt/qimenbot
ExecStart=/opt/qimenbot/qimenbotd
Restart=on-failure
RestartSec=5
Environment=ONEBOT_ENDPOINT=ws://127.0.0.1:3001
Environment=ONEBOT_TOKEN=your-token
Environment=BOT_OWNER=123456
Environment=RUST_LOG=info

# 安全加固
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/opt/qimenbot/config /opt/qimenbot/logs
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

### 管理服务

```bash
# 启用开机自启
sudo systemctl enable qimenbot

# 启动
sudo systemctl start qimenbot

# 查看状态
sudo systemctl status qimenbot

# 查看日志
sudo journalctl -u qimenbot -f

# 重启
sudo systemctl restart qimenbot

# 停止
sudo systemctl stop qimenbot
```

## Docker 部署

### Dockerfile

```dockerfile
# 构建阶段
FROM rust:1.89 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

# 运行阶段
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/qimenbotd /usr/local/bin/
COPY config/ /app/config/

WORKDIR /app
EXPOSE 6701

CMD ["qimenbotd"]
```

### docker-compose.yml

```yaml
version: '3.8'
services:
  qimenbot:
    build: .
    restart: unless-stopped
    environment:
      - ONEBOT_ENDPOINT=ws://onebot:3001
      - ONEBOT_TOKEN=your-token
      - BOT_OWNER=123456
    volumes:
      - ./config:/app/config
      - ./plugins/bin:/app/plugins/bin
    depends_on:
      - onebot

  onebot:
    image: your-onebot-implementation
    # ... OneBot 实现的配置
```

## 性能调优

### 限流器配置

根据实际负载调整限流器：

```toml
[bots.limiter]
enable = true
rate = 10.0       # 每秒 10 条（高流量群）
capacity = 20     # 允许突发 20 条
timeout_secs = 3  # 等待 3 秒（而不是直接丢弃）
```

### 日志级别

生产环境建议 `info` 或 `warn`。`debug` 和 `trace` 会产生大量日志影响性能：

```toml
[observability]
level = "warn"        # 只记录警告和错误
json_logs = true      # JSON 便于结构化查询
```

### 多 Bot 资源分配

每个 Bot 实例占用一个 Tokio 异步任务。如果运行多个 Bot，确保系统有足够的 CPU 和内存：

| Bot 数量 | 推荐配置 |
|---------|---------|
| 1-3 | 1 核 / 256MB |
| 3-10 | 2 核 / 512MB |
| 10+ | 4 核 / 1GB |

## 监控

### 健康检查

```bash
# 通过发送 /ping 命令验证 Bot 是否正常
# 或者检查进程是否运行
systemctl is-active qimenbot
```

### 日志监控

JSON 日志可以接入 ELK / Loki / Grafana 等监控系统：

```json
{"timestamp":"2024-01-01T00:00:00Z","level":"INFO","message":"Bot [qq-main] 已就绪"}
```

### 动态插件健康

通过 `/dynamic-errors` 命令查看动态插件的熔断器状态：

```
插件: my-plugin
  状态: 正常
  失败次数: 0
  最后错误: 无

插件: buggy-plugin
  状态: 隔离中 (剩余 45 秒)
  失败次数: 3
  最后错误: panic in callback
```

## 备份策略

需要备份的文件：

| 文件 | 说明 | 频率 |
|------|------|------|
| `config/base.toml` | 主配置 | 修改后备份 |
| `config/plugin-state.toml` | 插件状态 | 每日 |
| `plugins/bin/` | 动态插件 | 更新后备份 |

::: warning 不要备份的文件
- `target/` — 编译中间文件，可以重新编译生成
- `Cargo.lock` — 如果是部署二进制文件则不需要
:::
