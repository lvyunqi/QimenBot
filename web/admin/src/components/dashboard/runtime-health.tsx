import { Cpu, Database, Gauge, HardDrive } from "lucide-react"

import type { AdminSnapshot } from "@/lib/api"

export function RuntimeHealth({ snapshot }: { snapshot?: AdminSnapshot }) {
  const health = [
    {
      label: "日志缓冲",
      value: snapshot ? snapshot.resources.log_entries + " / " + snapshot.resources.log_capacity : "-",
      progress: snapshot ? percent(snapshot.resources.log_entries, snapshot.resources.log_capacity) : 0,
      icon: Database,
      tone: "primary",
    },
    {
      label: "Bot 监督器",
      value: snapshot ? String(snapshot.resources.active_bot_supervisors) : "-",
      progress: snapshot ? percent(snapshot.resources.active_bot_supervisors, Math.max(snapshot.metrics.configured_bots, 1)) : 0,
      icon: Cpu,
      tone: "success",
    },
    {
      label: "动态插件",
      value: snapshot ? snapshot.metrics.loaded_dynamic_plugins + " loaded" : "-",
      progress: snapshot ? Math.min(100, snapshot.metrics.loaded_dynamic_plugins * 18) : 0,
      icon: HardDrive,
      tone: snapshot?.resources.dynamic_plugin_failures ? "warning" : "primary",
    },
    {
      label: "插件错误",
      value: snapshot ? String(snapshot.resources.dynamic_plugin_failures) : "-",
      progress: snapshot ? Math.min(100, snapshot.resources.dynamic_plugin_failures * 12) : 0,
      icon: Gauge,
      tone: snapshot?.resources.dynamic_plugin_failures ? "warning" : "success",
    },
  ] as const

  return (
    <section className="panel enter-panel health-panel" style={{ animationDelay: "160ms" }}>
      <div className="panel-header">
        <div>
          <h2 className="panel-title">运行资源</h2>
          <p className="panel-subtitle">宿主进程与队列</p>
        </div>
        <span className="font-mono text-[10px] text-muted-foreground">LIVE</span>
      </div>
      <div className="health-list">
        {health.map((item) => {
          const Icon = item.icon
          return (
            <div className="health-row" key={item.label}>
              <div className="flex items-center gap-2.5">
                <Icon className="size-4 text-muted-foreground" strokeWidth={1.8} />
                <span className="text-xs font-semibold text-foreground">{item.label}</span>
              </div>
              <span className="ml-auto font-mono text-[11px] font-semibold text-foreground">{item.value}</span>
              <div className="health-track">
                <span className={`health-fill is-${item.tone}`} style={{ width: `${item.progress}%` }} />
              </div>
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
