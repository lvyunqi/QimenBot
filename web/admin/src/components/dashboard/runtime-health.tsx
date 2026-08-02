import { Cpu, Database, Gauge, HardDrive } from "lucide-react"

import type { AdminSnapshot } from "@/lib/api"

export function RuntimeHealth({ snapshot }: { snapshot?: AdminSnapshot }) {
  const bufferUsage = snapshot ? percent(snapshot.resources.log_entries, snapshot.resources.log_capacity) : 0
  const configuredBots = snapshot?.metrics.configured_bots ?? 0
  const activeSupervisors = snapshot?.resources.active_bot_supervisors ?? 0
  const pluginFailures = snapshot?.resources.dynamic_plugin_failures ?? 0
  const health = [
    {
      label: "日志缓冲",
      value: snapshot ? snapshot.resources.log_entries + " / " + snapshot.resources.log_capacity : "-",
      detail: snapshot ? bufferUsage + "% 已用" : "等待运行时数据",
      icon: Database,
      tone: bufferUsage >= 80 ? "warning" : "primary",
    },
    {
      label: "Bot 监督器",
      value: snapshot ? activeSupervisors + " / " + configuredBots : "-",
      detail: !snapshot
        ? "等待运行时数据"
        : configuredBots === 0
          ? "尚未配置机器人"
          : activeSupervisors === configuredBots
            ? "监督任务完整"
            : "有实例未运行",
      icon: Cpu,
      tone: configuredBots > 0 && activeSupervisors === configuredBots ? "success" : "warning",
    },
    {
      label: "动态插件",
      value: snapshot ? String(snapshot.metrics.loaded_dynamic_plugins) : "-",
      detail: snapshot?.metrics.loaded_dynamic_plugins ? "已加载到运行时" : "当前未加载",
      icon: HardDrive,
      tone: pluginFailures ? "warning" : "primary",
    },
    {
      label: "插件错误",
      value: snapshot ? String(pluginFailures) : "-",
      detail: pluginFailures ? "需要检查加载日志" : "未发现加载错误",
      icon: Gauge,
      tone: pluginFailures ? "warning" : "success",
    },
  ] as const

  return (
    <section className="panel enter-panel health-panel" style={{ animationDelay: "160ms" }}>
      <div className="panel-header">
        <div>
          <h2 className="panel-title">运行时状态</h2>
          <p className="panel-subtitle">宿主、监督任务与插件加载</p>
        </div>
        <span className={"runtime-state" + (snapshot ? " is-online" : "")}>
          <i aria-hidden="true" />
          {snapshot ? "宿主运行中" : "等待状态"}
        </span>
      </div>
      <div className="health-list">
        {health.map((item) => {
          const Icon = item.icon
          return (
            <div className="health-row" key={item.label}>
              <div className={`health-icon is-${item.tone}`}>
                <Icon strokeWidth={1.8} />
              </div>
              <div className="health-copy">
                <span>{item.label}</span>
                <small>{item.detail}</small>
              </div>
              <strong>{item.value}</strong>
            </div>
          )
        })}
      </div>
    </section>
  )
}

function percent(value: number, total: number) {
  return Math.min(100, Math.round((value / Math.max(total, 1)) * 100))
}
