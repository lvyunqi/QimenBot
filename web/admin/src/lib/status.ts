import type { BotView, LogEntry } from "@/lib/api"

export function botStatusLabel(state: BotView["state"]) {
  const labels = {
    disabled: "已停用",
    starting: "启动中",
    online: "在线",
    reconnecting: "重连中",
    stopped: "已停止",
    error: "异常",
  } satisfies Record<BotView["state"], string>
  return labels[state]
}

export function botStatusVariant(state: BotView["state"]): "success" | "warning" | "danger" | "neutral" {
  if (state === "online") return "success"
  if (state === "starting" || state === "reconnecting") return "warning"
  if (state === "error") return "danger"
  return "neutral"
}

export function logTone(entry: LogEntry): "info" | "success" | "warning" {
  if (entry.level === "warn" || entry.level === "error") return "warning"
  if (entry.message.includes("connected") || entry.message.includes("succeeded")) return "success"
  return "info"
}

export function levelVariant(level: string): "success" | "warning" | "danger" | "neutral" | "default" {
  if (level === "error") return "danger"
  if (level === "warn") return "warning"
  if (level === "info") return "default"
  if (level === "debug" || level === "trace") return "neutral"
  return "neutral"
}
