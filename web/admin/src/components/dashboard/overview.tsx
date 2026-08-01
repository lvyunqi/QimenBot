import { lazy, Suspense } from "react"
import { Plus, RefreshCw } from "lucide-react"

import { Button } from "@/components/ui/button"
import { BotTable } from "@/components/dashboard/bot-table"
import { EventFeed } from "@/components/dashboard/event-feed"
import { MetricStrip } from "@/components/dashboard/metric-strip"
import { RuntimeHealth } from "@/components/dashboard/runtime-health"

const ThroughputChart = lazy(() =>
  import("@/components/dashboard/throughput-chart").then((module) => ({ default: module.ThroughputChart })),
)

function ChartFallback() {
  return (
    <section className="panel chart-panel chart-fallback" aria-label="正在载入消息吞吐">
      <div className="panel-header">
        <div className="space-y-2">
          <div className="skeleton-line h-3 w-24" />
          <div className="skeleton-line h-2 w-36" />
        </div>
      </div>
      <div className="chart-canvas p-5">
        <div className="skeleton-line size-full" />
      </div>
    </section>
  )
}

export function Overview() {
  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>运行总览</h1>
            <span className="environment-tag">DEV</span>
          </div>
          <p>2026 年 8 月 1 日 · 所有实例</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm">
            <RefreshCw />
            <span className="hidden sm:inline">刷新状态</span>
          </Button>
          <Button size="sm">
            <Plus />
            添加机器人
          </Button>
        </div>
      </div>

      <MetricStrip />

      <div className="dashboard-grid dashboard-grid-top">
        <Suspense fallback={<ChartFallback />}>
          <ThroughputChart data={[]} />
        </Suspense>
        <EventFeed />
      </div>

      <div className="dashboard-grid dashboard-grid-bottom">
        <BotTable />
        <RuntimeHealth />
      </div>
    </main>
  )
}
