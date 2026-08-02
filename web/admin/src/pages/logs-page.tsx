import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react"
import { ArrowDownToLine, Pause, Play, RefreshCw, Search } from "lucide-react"
import { toast } from "sonner"

import { api, openLogStream, type LogEntry } from "@/lib/api"
import { formatClock } from "@/lib/format"
import { levelVariant } from "@/lib/status"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

type FollowMode = "smart" | "always" | "off"

const followModeStorageKey = "qimen-log-follow-mode"
const followThreshold = 56

function initialFollowMode(): FollowMode {
  const stored = localStorage.getItem(followModeStorageKey)
  return stored === "always" || stored === "off" ? stored : "smart"
}

export function LogsPage() {
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [paused, setPaused] = useState(false)
  const [level, setLevel] = useState("all")
  const [query, setQuery] = useState("")
  const [connected, setConnected] = useState(false)
  const [followMode, setFollowMode] = useState<FollowMode>(initialFollowMode)
  const [atBottom, setAtBottom] = useState(true)
  const [pendingLogs, setPendingLogs] = useState(0)
  const terminalRef = useRef<HTMLDivElement>(null)
  const followModeRef = useRef(followMode)
  const nearBottomRef = useRef(true)
  const pendingScrollRef = useRef(false)

  const scrollToBottom = useCallback((behavior: ScrollBehavior = "auto") => {
    const terminal = terminalRef.current
    if (!terminal) return
    terminal.scrollTo({ top: terminal.scrollHeight, behavior })
    nearBottomRef.current = true
    setAtBottom(true)
    setPendingLogs(0)
  }, [])

  const load = useCallback(async () => {
    const params = new URLSearchParams()
    params.set("limit", "600")
    if (level !== "all") params.set("level", level)
    const result = await api.logs(params)
    pendingScrollRef.current = followModeRef.current !== "off"
    setPendingLogs(0)
    if (followModeRef.current === "off") {
      nearBottomRef.current = false
      setAtBottom(false)
    }
    setLogs(result.entries)
  }, [level])

  useEffect(() => {
    followModeRef.current = followMode
    localStorage.setItem(followModeStorageKey, followMode)
  }, [followMode])

  useEffect(() => {
    void load().catch((error) => toast.error(error.message))
  }, [load])

  useEffect(() => {
    const stream = openLogStream(
      (entry) => {
        setConnected(true)
        if (paused) return
        const mode = followModeRef.current
        const shouldFollow = mode === "always" || (mode === "smart" && nearBottomRef.current)
        pendingScrollRef.current = shouldFollow
        if (!shouldFollow) {
          nearBottomRef.current = false
          setAtBottom(false)
          setPendingLogs((current) => Math.min(current + 1, 999))
        }
        setLogs((current) => [...current.slice(-599), entry])
      },
      () => setConnected(false),
      () => setConnected(true),
    )
    return () => stream.close()
  }, [paused])

  useLayoutEffect(() => {
    if (!pendingScrollRef.current) return
    pendingScrollRef.current = false
    scrollToBottom()
  }, [logs, scrollToBottom])

  const handleTerminalScroll = () => {
    const terminal = terminalRef.current
    if (!terminal) return
    const remaining = terminal.scrollHeight - terminal.scrollTop - terminal.clientHeight
    const nextAtBottom = remaining <= followThreshold
    nearBottomRef.current = nextAtBottom
    setAtBottom((current) => (current === nextAtBottom ? current : nextAtBottom))
    if (nextAtBottom) setPendingLogs(0)
  }

  const updateFollowMode = (mode: FollowMode) => {
    followModeRef.current = mode
    setFollowMode(mode)
    if (mode === "off") return
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    scrollToBottom(reduceMotion ? "auto" : "smooth")
  }

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return logs
    return logs.filter((entry) =>
      [entry.message, entry.target, entry.level, ...Object.values(entry.fields)]
        .join(" ")
        .toLowerCase()
        .includes(needle),
    )
  }, [logs, query])

  return (
    <main className="page-shell logs-page">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>实时日志</h1>
            <span className={"environment-tag " + (connected ? "" : "is-muted")}>{connected ? "STREAMING" : "BUFFER"}</span>
          </div>
          <p>结构化 tracing 缓冲区，支持服务端查询和浏览器实时流。</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="topbar-search logs-search">
            <Search className="size-4 text-muted-foreground" />
            <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="过滤日志" className="h-7 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0" />
          </div>
          <select className="mini-select" value={level} onChange={(event) => setLevel(event.target.value)}>
            <option value="all">全部级别</option>
            <option value="error">error</option>
            <option value="warn">warn</option>
            <option value="info">info</option>
            <option value="debug">debug</option>
            <option value="trace">trace</option>
          </select>
          <div className={"log-follow-control is-" + followMode}>
            <ArrowDownToLine aria-hidden="true" />
            <select
              value={followMode}
              aria-label="自动滚动模式"
              title="智能跟随会在手动查看历史日志时暂停；始终跟随会持续回到底部"
              onChange={(event) => updateFollowMode(event.target.value as FollowMode)}
            >
              <option value="smart">智能跟随</option>
              <option value="always">始终跟随</option>
              <option value="off">关闭跟随</option>
            </select>
          </div>
          <Button variant="outline" size="sm" onClick={() => setPaused((value) => !value)}>
            {paused ? <Play /> : <Pause />}
            {paused ? "继续" : "暂停"}
          </Button>
          <Button variant="outline" size="sm" onClick={() => void load()}>
            <RefreshCw />
            重载
          </Button>
        </div>
      </div>
      <section className="panel enter-panel log-panel">
        <div className="log-terminal" ref={terminalRef} onScroll={handleTerminalScroll}>
          {visible.map((entry) => (
            <div className="log-line" key={entry.id}>
              <span className="log-time">{formatClock(entry.timestamp)}</span>
              <Badge variant={levelVariant(entry.level)}>{entry.level}</Badge>
              <span className="log-target" title={entry.target}>{entry.target}</span>
              <span className="log-message">{entry.message}</span>
            </div>
          ))}
          {visible.length === 0 && <div className="empty-panel">没有匹配日志。</div>}
        </div>
        {!atBottom && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="log-follow-resume"
            aria-live="polite"
            onClick={() => scrollToBottom(window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth")}
          >
            <ArrowDownToLine />
            {pendingLogs > 0 ? `${pendingLogs > 99 ? "99+" : pendingLogs} 条新日志` : "回到底部"}
          </Button>
        )}
      </section>
    </main>
  )
}
