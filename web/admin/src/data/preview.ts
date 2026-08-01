export type BotStatus = "online" | "reconnecting" | "offline"

export interface BotPreview {
  id: string
  protocol: string
  transport: string
  status: BotStatus
  latency: string
  events: string
  lastSeen: string
}

export const bots: BotPreview[] = [
  {
    id: "qq-main",
    protocol: "OneBot 11",
    transport: "WS Forward",
    status: "online",
    latency: "24 ms",
    events: "18.4k",
    lastSeen: "刚刚",
  },
  {
    id: "qq-official",
    protocol: "QQ Official",
    transport: "Gateway",
    status: "online",
    latency: "41 ms",
    events: "6.8k",
    lastSeen: "2 秒前",
  },
  {
    id: "qq-backup",
    protocol: "OneBot 11",
    transport: "WS Reverse",
    status: "reconnecting",
    latency: "--",
    events: "1.2k",
    lastSeen: "1 分钟前",
  },
]

export const throughput = [
  { time: "14:00", events: 142, replies: 96 },
  { time: "14:10", events: 176, replies: 118 },
  { time: "14:20", events: 158, replies: 110 },
  { time: "14:30", events: 214, replies: 146 },
  { time: "14:40", events: 198, replies: 139 },
  { time: "14:50", events: 268, replies: 172 },
  { time: "15:00", events: 242, replies: 164 },
  { time: "15:10", events: 306, replies: 204 },
  { time: "15:20", events: 282, replies: 191 },
  { time: "15:30", events: 338, replies: 226 },
  { time: "15:40", events: 321, replies: 218 },
  { time: "15:50", events: 374, replies: 248 },
]

export type EventTone = "info" | "success" | "warning"

export interface PreviewEvent {
  id: number
  time: string
  source: string
  message: string
  tone: EventTone
}

export const eventSeed: PreviewEvent[] = [
  { id: 1, time: "15:52:08", source: "qq-official", message: "收到 GROUP_AT_MESSAGE_CREATE", tone: "info" },
  { id: 2, time: "15:52:06", source: "example-basic", message: "命令 /ping 执行完成 · 18 ms", tone: "success" },
  { id: 3, time: "15:51:54", source: "qq-main", message: "WebSocket heartbeat acknowledged", tone: "info" },
  { id: 4, time: "15:51:42", source: "runtime", message: "主动发送队列 2 / 256", tone: "info" },
  { id: 5, time: "15:51:20", source: "qq-backup", message: "连接中断，2 秒后重试", tone: "warning" },
]

export const eventRotation: Omit<PreviewEvent, "id" | "time">[] = [
  { source: "qq-official", message: "OpenAPI action completed · 37 ms", tone: "success" },
  { source: "runtime", message: "消息去重窗口已更新", tone: "info" },
  { source: "example-message", message: "Markdown reply dispatched", tone: "success" },
  { source: "qq-main", message: "收到 group message event", tone: "info" },
]
