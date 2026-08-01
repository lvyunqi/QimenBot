import { useEffect, useRef, useState } from "react"
import { ArrowUpRight, Radio } from "lucide-react"

import { Button } from "@/components/ui/button"
import { eventRotation, eventSeed, type PreviewEvent } from "@/data/preview"

function currentTime() {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date())
}

export function EventFeed() {
  const [events, setEvents] = useState<PreviewEvent[]>(eventSeed)
  const nextId = useRef(20)
  const rotation = useRef(0)

  useEffect(() => {
    const timer = window.setInterval(() => {
      const next = eventRotation[rotation.current % eventRotation.length]
      rotation.current += 1
      nextId.current += 1
      setEvents((current) => [
        { ...next, id: nextId.current, time: currentTime() },
        ...current.slice(0, 4),
      ])
    }, 3600)

    return () => window.clearInterval(timer)
  }, [])

  return (
    <section className="panel enter-panel event-panel" style={{ animationDelay: "80ms" }}>
      <div className="panel-header">
        <div>
          <div className="flex items-center gap-2">
            <Radio className="size-4 text-success" />
            <h2 className="panel-title">实时事件</h2>
            <span className="live-indicator">LIVE</span>
          </div>
          <p className="panel-subtitle">最近进入运行时的事件</p>
        </div>
        <Button variant="ghost" size="icon-sm" aria-label="打开实时日志">
          <ArrowUpRight />
        </Button>
      </div>

      <div className="event-list" aria-live="polite">
        {events.map((event, index) => (
          <div className="event-row" key={event.id} style={{ animationDelay: index === 0 ? "0ms" : undefined }}>
            <span className={`event-tone is-${event.tone}`} />
            <span className="event-time">{event.time}</span>
            <div className="min-w-0 flex-1">
              <div className="truncate text-xs font-semibold text-foreground">{event.message}</div>
              <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground">{event.source}</div>
            </div>
          </div>
        ))}
      </div>
    </section>
  )
}
