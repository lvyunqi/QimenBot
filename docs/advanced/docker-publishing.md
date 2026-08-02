# 发布 Docker Hub 镜像

这一页只面向 QimenBot 仓库维护者。普通用户直接使用 `mryunqi/qimenbot`，不需要 Docker Hub Token。

仓库中的 `.github/workflows/docker-publish.yml` 会在推送 `v*` Tag 时构建并发布 `linux/amd64`、`linux/arm64` 镜像。

## 创建仓库和令牌

在 [Docker Hub](https://hub.docker.com/) 创建 Public 仓库，例如 `mryunqi/qimenbot`。Docker Personal 可以免费发布公共镜像，但拉取和构建仍受 Docker Hub 的公平使用限制。

进入 **Account settings > Personal access tokens** 创建令牌。令牌至少需要 Read、Write 权限。不要把账号密码或令牌写入仓库文件。

## 配置 GitHub Environment

工作流使用名为 `docker hub` 的 GitHub Environment。进入：

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

## Tag 规则

推送 `v0.1.15` 后会生成：

```text
mryunqi/qimenbot:0.1.15
mryunqi/qimenbot:0.1
mryunqi/qimenbot:latest
mryunqi/qimenbot:sha-<提交摘要>
```

每个版本 Tag 都是包含两个 CPU 架构的 manifest。`workflow_dispatch` 可用于测试工作流，但从普通分支手工运行时通常只生成 `sha-*` Tag，不能替代正式版本 Tag。

## 发布后检查

```bash
docker buildx imagetools inspect mryunqi/qimenbot:0.1.15
docker pull mryunqi/qimenbot:0.1.15
docker image inspect mryunqi/qimenbot:0.1.15 --format '{{.Architecture}} {{.Os}}'
```

登录阶段出现 `unauthorized` 时，确认 Secret 中存放的是 Docker Hub Token、用户名属于镜像 Namespace，且令牌状态为 Active。一个架构构建失败时，多架构 manifest 不会完整发布，应先查看失败平台的日志。
