import { BarChart3 } from "lucide-react"
import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts"

import type { ThroughputPoint } from "@/lib/api"

interface ThroughputChartProps {
  data: ThroughputPoint[]
}

export function ThroughputChart({ data }: ThroughputChartProps) {
  const chartData = data.map((point) => ({
    time: new Date(point.minute_epoch * 60_000).toLocaleTimeString("zh-CN", {
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }),
    events: point.events,
    replies: point.replies,
  }))

  return (
    <section className="panel enter-panel chart-panel">
      <div className="panel-header">
        <div>
          <div className="flex items-center gap-2">
            <BarChart3 className="size-4 text-primary" />
            <h2 className="panel-title">消息吞吐</h2>
          </div>
          <p className="panel-subtitle">最近 2 小时 · 每分钟滚动统计</p>
        </div>
        <div className="hidden items-center gap-3 text-[11px] font-semibold text-muted-foreground sm:flex">
          <span className="flex items-center gap-1.5"><i className="size-2 rounded-full bg-primary" />事件</span>
          <span className="flex items-center gap-1.5"><i className="size-2 rounded-full bg-success" />回复</span>
        </div>
      </div>

      <div className="chart-canvas" aria-label="消息吞吐趋势图">
        <ResponsiveContainer width="100%" height="100%">
          <AreaChart data={chartData} margin={{ top: 10, right: 8, left: 0, bottom: 0 }}>
            <CartesianGrid vertical={false} stroke="var(--chart-grid)" strokeDasharray="3 5" />
            <XAxis
              dataKey="time"
              axisLine={false}
              tickLine={false}
              tick={{ fill: "var(--muted-foreground)", fontSize: 10, fontFamily: "var(--font-mono)" }}
              interval={1}
              dy={8}
            />
            <YAxis
              axisLine={false}
              tickLine={false}
              tick={{ fill: "var(--muted-foreground)", fontSize: 10, fontFamily: "var(--font-mono)" }}
              width={34}
            />
            <Tooltip
              cursor={{ stroke: "var(--border-strong)", strokeDasharray: "3 3" }}
              contentStyle={{
                background: "var(--popover)",
                border: "1px solid var(--border)",
                borderRadius: 7,
                boxShadow: "0 12px 30px rgba(25,26,23,.12)",
                fontSize: 12,
              }}
              labelStyle={{ color: "var(--muted-foreground)", fontFamily: "var(--font-mono)", marginBottom: 6 }}
            />
            <Area
              type="monotone"
              dataKey="events"
              name="事件"
              stroke="var(--chart-primary)"
              strokeWidth={2.2}
              fill="var(--chart-primary)"
              fillOpacity={0.08}
              animationDuration={850}
              animationEasing="ease-out"
            />
            <Area
              type="monotone"
              dataKey="replies"
              name="回复"
              stroke="var(--chart-success)"
              strokeWidth={1.8}
              fill="var(--chart-success)"
              fillOpacity={0.035}
              animationDuration={1050}
              animationEasing="ease-out"
            />
          </AreaChart>
        </ResponsiveContainer>
        {chartData.length === 0 && (
          <div className="pointer-events-none absolute inset-0 grid place-items-center text-xs font-semibold text-muted-foreground">
            等待运行时事件
          </div>
        )}
      </div>
    </section>
  )
}
