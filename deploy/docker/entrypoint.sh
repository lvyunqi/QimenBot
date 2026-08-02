#!/bin/sh
set -eu

mkdir -p /data/config/plugins /data/plugins/bin /data/logs
if [ ! -f /data/config/base.toml ]; then
  cp /opt/qimenbot/defaults/base.toml.example /data/config/base.toml
fi
if [ ! -f /data/config/plugin-state.toml ]; then
  cp /opt/qimenbot/defaults/plugin-state.toml /data/config/plugin-state.toml
fi

# 挂载目录可能由宿主机以 root 创建，启动前只修正 QimenBot 自己的持久化目录。
chown -R qimenbot:qimenbot /data/config /data/plugins /data/logs
exec gosu qimenbot:qimenbot "$@"
