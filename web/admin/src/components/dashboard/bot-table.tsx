import { ArrowRight, Bot, MoreHorizontal, RefreshCw } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { bots, type BotStatus } from "@/data/preview"

const statusMeta: Record<BotStatus, { label: string; variant: "success" | "warning" | "neutral" }> = {
  online: { label: "在线", variant: "success" },
  reconnecting: { label: "重连中", variant: "warning" },
  offline: { label: "离线", variant: "neutral" },
}

export function BotTable() {
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
        <Button variant="outline" size="sm">
          查看全部
          <ArrowRight />
        </Button>
      </div>

      <div className="bot-table" role="table" aria-label="机器人实例">
        <div className="bot-table-head" role="row">
          <span>实例</span>
          <span>协议</span>
          <span>连接</span>
          <span>延迟</span>
          <span>今日事件</span>
          <span>最后活动</span>
          <span aria-hidden="true" />
        </div>
        {bots.map((bot, index) => {
          const status = statusMeta[bot.status]
          return (
            <div className="bot-table-row" role="row" key={bot.id} style={{ animationDelay: `${160 + index * 45}ms` }}>
              <div className="flex min-w-0 items-center gap-3" role="cell">
                <div className="bot-mark">
                  <Bot />
                  <span className={`bot-status-dot is-${bot.status}`} />
                </div>
                <div className="min-w-0">
                  <div className="truncate font-mono text-xs font-bold text-foreground">{bot.id}</div>
                  <div className="mt-0.5 text-[10px] text-muted-foreground">{bot.transport}</div>
                </div>
              </div>
              <div role="cell"><Badge variant={bot.protocol === "QQ Official" ? "default" : "neutral"}>{bot.protocol}</Badge></div>
              <div role="cell"><Badge variant={status.variant}><span className={`size-1.5 rounded-full bg-current ${bot.status === "reconnecting" ? "animate-pulse" : ""}`} />{status.label}</Badge></div>
              <div role="cell" className="table-mono">{bot.latency}</div>
              <div role="cell" className="table-mono">{bot.events}</div>
              <div role="cell" className="text-xs text-muted-foreground">{bot.lastSeen}</div>
              <div role="cell" className="flex justify-end gap-1">
                {bot.status === "reconnecting" && (
                  <Button variant="ghost" size="icon-sm" aria-label={`重新连接 ${bot.id}`}>
                    <RefreshCw className="animate-spin-slow" />
                  </Button>
                )}
                <Button variant="ghost" size="icon-sm" aria-label={`${bot.id} 更多操作`}>
                  <MoreHorizontal />
                </Button>
              </div>
            </div>
          )
        })}
      </div>
    </section>
  )
}
