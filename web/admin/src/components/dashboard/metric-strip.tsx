import { Activity, Bot, CircleAlert, Send } from "lucide-react"

const metrics = [
  { label: "在线机器人", value: "2 / 3", detail: "1 个正在重连", icon: Bot, tone: "brand" },
  { label: "今日事件", value: "26,482", detail: "+12.8%", icon: Activity, tone: "success" },
  { label: "发送成功率", value: "99.94%", detail: "16,928 次动作", icon: Send, tone: "success" },
  { label: "待处理告警", value: "1", detail: "连接重试", icon: CircleAlert, tone: "warning" },
] as const

export function MetricStrip() {
  return (
    <section className="metric-strip" aria-label="运行指标">
      {metrics.map((metric, index) => {
        const Icon = metric.icon
        return (
          <div className="metric-cell enter-item" style={{ animationDelay: `${index * 45}ms` }} key={metric.label}>
            <div className={`metric-icon is-${metric.tone}`}>
              <Icon strokeWidth={1.9} />
            </div>
            <div className="min-w-0">
              <p className="metric-label">{metric.label}</p>
              <div className="mt-1 flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                <strong className="metric-value">{metric.value}</strong>
                <span className={`metric-detail is-${metric.tone}`}>{metric.detail}</span>
              </div>
            </div>
          </div>
        )
      })}
    </section>
  )
}
