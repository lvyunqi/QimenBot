#!/usr/bin/env bash
set -Eeuo pipefail

RAW_BASE="https://raw.githubusercontent.com/lvyunqi/QimenBot/main"

if ! command -v docker >/dev/null 2>&1; then
  echo "未找到 Docker。请先安装 Docker Engine 和 Compose v2。" >&2
  exit 1
fi

if ! docker compose version >/dev/null 2>&1; then
  echo "未找到 docker compose。请安装 Compose v2。" >&2
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "未找到 curl。请先安装 curl。" >&2
  exit 1
fi

if ! docker info >/dev/null 2>&1; then
  echo "当前用户无法连接 Docker。请启动 Docker，并检查当前用户的 Docker 权限。" >&2
  exit 1
fi

if [[ "$(id -u)" -eq 0 ]]; then
  default_install_dir="/opt/qimenbot"
else
  default_install_dir="${HOME}/qimenbot"
fi

install_dir="${QIMENBOT_HOME:-$default_install_dir}"
env_file="${install_dir}/.env"
compose_file="${install_dir}/compose.yaml"

mkdir -p \
  "${install_dir}/data/config" \
  "${install_dir}/data/plugins" \
  "${install_dir}/data/logs"
chmod 700 "${install_dir}"

generate_token() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -hex 32
  else
    od -An -N32 -tx1 /dev/urandom | tr -d ' \n'
  fi
}

if [[ ! -f "${env_file}" ]]; then
  appid="${QQBOT_APPID:-}"
  secret="${QQBOT_SECRET:-}"

  if [[ -z "${appid}" ]]; then
    if [[ ! -t 0 ]]; then
      echo "非交互安装请通过 QQBOT_APPID 和 QQBOT_SECRET 提供凭据。" >&2
      exit 1
    fi
    read -r -p "QQ 官方机器人 AppID: " appid
  fi

  if [[ -z "${secret}" ]]; then
    if [[ ! -t 0 ]]; then
      echo "非交互安装请通过 QQBOT_APPID 和 QQBOT_SECRET 提供凭据。" >&2
      exit 1
    fi
    read -r -s -p "QQ 官方机器人 Secret: " secret
    echo
  fi

  if [[ -z "${appid}" || -z "${secret}" ]]; then
    echo "AppID 和 Secret 不能为空。" >&2
    exit 1
  fi

  admin_token="${QIMEN_ADMIN_TOKEN:-$(generate_token)}"
  image="${QIMENBOT_IMAGE:-mryunqi/qimenbot}"
  tag="${QIMENBOT_TAG:-latest}"

  umask 077
  {
    printf 'QIMENBOT_IMAGE=%s\n' "${image}"
    printf 'QIMENBOT_TAG=%s\n' "${tag}"
    printf 'QIMEN_CONFIG_DIR=%s\n' "${install_dir}/data/config"
    printf 'QIMEN_PLUGIN_DIR=%s\n' "${install_dir}/data/plugins"
    printf 'QIMEN_LOG_DIR=%s\n' "${install_dir}/data/logs"
    printf 'QIMEN_ADMIN_TOKEN=%s\n' "${admin_token}"
    printf 'QQBOT_APPID=%s\n' "${appid}"
    printf 'QQBOT_SECRET=%s\n' "${secret}"
    printf 'QIMEN_WEBHOOK_TOKEN=\n'
    printf 'RUST_LOG=info\n'
  } >"${env_file}"
  chmod 600 "${env_file}"

  echo "管理面板 Token: ${admin_token}"
  echo "Token 已保存到 ${env_file}，请妥善保管。"
else
  echo "检测到 ${env_file}，保留现有凭据和镜像版本。"
fi

tmp_compose="$(mktemp)"
trap 'rm -f "${tmp_compose}"' EXIT
curl --fail --silent --show-error --location \
  "${RAW_BASE}/compose.yaml" \
  --output "${tmp_compose}"
mv "${tmp_compose}" "${compose_file}"
trap - EXIT

docker compose --project-directory "${install_dir}" \
  --env-file "${env_file}" -f "${compose_file}" config --quiet
docker compose --project-directory "${install_dir}" \
  --env-file "${env_file}" -f "${compose_file}" pull
docker compose --project-directory "${install_dir}" \
  --env-file "${env_file}" -f "${compose_file}" up -d

healthy=false
for _ in {1..30}; do
  if curl --fail --silent "http://127.0.0.1:3210/healthz" >/dev/null 2>&1; then
    healthy=true
    break
  fi
  sleep 1
done

echo
echo "安装目录: ${install_dir}"
echo "配置目录: ${install_dir}/data/config"
echo "插件目录: ${install_dir}/data/plugins"
echo "日志目录: ${install_dir}/data/logs"

if [[ "${healthy}" == true ]]; then
  echo "QimenBot 已启动: http://127.0.0.1:3210/"
else
  echo "容器尚未通过健康检查，请执行以下命令查看日志：" >&2
  echo "cd '${install_dir}' && docker compose --env-file .env logs --tail 200 qimenbot" >&2
  exit 1
fi
