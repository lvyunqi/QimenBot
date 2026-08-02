import { lazy, Suspense } from "react"
import { Activity, Bot, CircleAlert, Plus, RefreshCw, Send } from "lucide-react"

import type { AdminSnapshot } from "@/lib/api"
import { formatNumber, formatUptime, relativeTime } from "@/lib/format"
import { botStatusLabel, botStatusVariant, logTone } from "@/lib/status"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { RuntimeHealth } from "@/components/dashboard/runtime-health"

const ThroughputChart = lazy(() =>
  import("@/components/dashboard/throughput-chart").then((module) => ({ default: module.ThroughputChart })),
)

interface OverviewPageProps {
  snapshot?: AdminSnapshot
  loading: boolean
  error?: string | null
  onRefresh: () => void
  onNavigate: (page: string) => void
  onBotAction: (id: string, action: "start" | "stop" | "reconnect") => void
}

export function OverviewPage({ snapshot, loading, error, onRefresh, onNavigate, onBotAction }: OverviewPageProps) {
  const metrics = [
    {
      label: "在线机器人",
      value: snapshot ? snapshot.metrics.online_bots + " / " + snapshot.metrics.configured_bots : "-",
      detail: snapshot ? formatUptime(snapshot.server.uptime_secs) : "等待宿主",
      icon: Bot,
      tone: "brand",
    },
    {
      label: "累计事件",
      value: snapshot ? formatNumber(snapshot.metrics.events_total) : "-",
      detail: "Runtime 接收",
      icon: Activity,
      tone: "success",
    },
    {
      label: "累计回复",
      value: snapshot ? formatNumber(snapshot.metrics.replies_total) : "-",
      detail: "命令与系统响应",
      icon: Send,
      tone: "success",
    },
    {
      label: "告警",
      value: snapshot ? String(snapshot.metrics.warning_count) : "-",
      detail: snapshot?.server.restart_required ? "配置待重启" : "连接与插件",
      icon: CircleAlert,
      tone: snapshot?.metrics.warning_count ? "warning" : "brand",
    },
  ] as const

  return (
    <main className="page-shell overview-page">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>运行总览</h1>
            <span className="environment-tag">{snapshot?.server.environment ?? "LIVE"}</span>
            {snapshot?.server.restart_required && <Badge variant="warning">重启后应用全部配置</Badge>}
          </div>
          <p>{snapshot ? "版本 " + snapshot.server.version + " · " + snapshot.server.config_revision : "连接本机管理 API"}</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={onRefresh} disabled={loading}>
            <RefreshCw className={loading ? "animate-spin-slow" : ""} />
            <span className="hidden sm:inline">刷新状态</span>
          </Button>
          <Button size="sm" onClick={() => onNavigate("bots")}>
            <Plus />
            添加机器人
          </Button>
        </div>
      </div>

      {error && <div className="notice-banner is-danger">{error}</div>}

      <section className="metric-strip" aria-label="运行指标">
        {metrics.map((metric, index) => {
          const Icon = metric.icon
          return (
            <div
              className={"metric-cell enter-item" + (index === 0 ? " is-anchor" : "")}
              style={{ animationDelay: index * 45 + "ms" }}
              key={metric.label}
            >
              <div className={"metric-icon is-" + metric.tone}>
                <Icon strokeWidth={1.9} />
              </div>
              <div className="min-w-0">
                <p className="metric-label">{metric.label}</p>
                <div className="mt-1 flex flex-wrap items-baseline gap-x-2 gap-y-0.5">
                  <strong className="metric-value">{metric.value}</strong>
                  <span className={"metric-detail is-" + metric.tone}>{metric.detail}</span>
                </div>
              </div>
            </div>
          )
        })}
      </section>

      <div className="dashboard-grid dashboard-grid-top">
        <Suspense fallback={<ChartFallback />}>
          <ThroughputChart data={snapshot?.metrics.throughput ?? []} />
        </Suspense>
        <RecentLogs logs={snapshot?.recent_logs ?? []} onOpenLogs={() => onNavigate("logs")} />
      </div>

      <div className="dashboard-grid dashboard-grid-bottom">
        <BotSummary bots={snapshot?.bots ?? []} onOpenBots={() => onNavigate("bots")} onBotAction={onBotAction} />
        <RuntimeHealth snapshot={snapshot} />
      </div>
    </main>
  )
}

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

function RecentLogs({ logs, onOpenLogs }: { logs: AdminSnapshot["recent_logs"]; onOpenLogs: () => void }) {
  return (
    <section className="panel enter-panel event-panel" style={{ animationDelay: "80ms" }}>
      <div className="panel-header">
        <div>
          <div className="flex items-center gap-2">
            <Activity className="size-4 text-success" />
            <h2 className="panel-title">实时事件</h2>
            <span className="live-indicator">流式</span>
          </div>
          <p className="panel-subtitle">日志缓冲区最新记录</p>
        </div>
        <Button variant="ghost" size="sm" onClick={onOpenLogs}>打开日志</Button>
      </div>
      <div className="event-list" aria-live="polite">
        {logs.length === 0 && <EmptyRow message="暂时没有日志进入缓冲区" />}
        {logs.map((event, index) => (
          <div className="event-row" key={event.id} style={{ animationDelay: index === 0 ? "0ms" : undefined }}>
            <span className={"event-tone is-" + logTone(event)} />
            <span className="event-time">{new Date(event.timestamp).toLocaleTimeString("zh-CN", { hour12: false })}</span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-xs font-semibold text-foreground">{event.message}</div>
              <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground">{event.target}</div>
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}

function BotSummary({
  bots,
  onOpenBots,
  onBotAction,
}: {
  bots: AdminSnapshot["bots"]
  onOpenBots: () => void
  onBotAction: (id: string, action: "start" | "stop" | "reconnect") => void
}) {
  return (
    <section className="panel enter-panel bot-table-panel" style={{ animationDelay: "120ms" }}>
      <div className="panel-header">
        <div>
          <div className="flex items-center gap-2">
            <Bot className="size-4 text-primary" />
            <h2 className="panel-title">机器人实例</h2>
          </div>
          <p className="panel-subtitle">协议连接和事件处理状态</p>
        </div>
        <Button variant="outline" size="sm" onClick={onOpenBots}>查看全部</Button>
      </div>
      <div className="bot-table" role="table" aria-label="机器人实例">
        <div className="bot-table-head" role="row">
          <span>实例</span>
          <span>协议</span>
          <span>连接</span>
          <span>活动</span>
          <span aria-hidden="true" />
        </div>
        {bots.map((bot, index) => (
          <div className="bot-table-row" role="row" key={bot.id} style={{ animationDelay: 160 + index * 45 + "ms" }}>
            <div className="bot-instance-cell" role="cell">
              <div className="bot-mark">
                <Bot />
                <span className={"bot-status-dot is-" + bot.state} />
              </div>
              <div className="min-w-0">
                <div className="truncate font-mono text-xs font-bold text-foreground" title={bot.id}>{bot.id}</div>
                <div className="bot-instance-meta">{bot.transport}</div>
              </div>
            </div>
            <div role="cell" className="bot-protocol-cell">
              <Badge variant={bot.protocol === "qq-official" ? "default" : "neutral"}>{bot.protocol}</Badge>
            </div>
            <div role="cell" className="bot-connection-cell">
              <Badge variant={botStatusVariant(bot.state)}>{botStatusLabel(bot.state)}</Badge>
              <span>重连 {bot.reconnect_count}</span>
            </div>
            <div role="cell" className="bot-activity-cell">
              <strong>{formatNumber(bot.events_received)} 条</strong>
              <span>{relativeTime(bot.last_event_epoch_ms)}</span>
            </div>
            <div role="cell" className="bot-row-action">
              <Button variant="ghost" size="icon-sm" aria-label={"重连 " + bot.id} onClick={() => onBotAction(bot.id, "reconnect")} disabled={!bot.desired_enabled}>
                <RefreshCw />
              </Button>
            </div>
            <div role="cell" className="bot-runtime-route">
              <span>已启用模块</span>
              <div>
                {bot.enabled_modules.slice(0, 3).map((module) => <code key={module}>{module}</code>)}
                {bot.enabled_modules.length === 0 && <small>无</small>}
                {bot.enabled_modules.length > 3 && <small>+{bot.enabled_modules.length - 3}</small>}
              </div>
              <strong>{bot.intents.length} Intents</strong>
            </div>
          </div>
        ))}
        {bots.length === 0 && <EmptyRow message="还没有配置机器人实例" />}
      </div>
    </section>
  )
}

function EmptyRow({ message }: { message: string }) {
  return <div className="px-4 py-10 text-center text-xs font-semibold text-muted-foreground">{message}</div>
}
