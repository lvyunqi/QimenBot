import { Bell, Command, Moon, Search, Sun } from "lucide-react"

import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { AdminSnapshot } from "@/lib/api"

interface TopbarProps {
  dark: boolean
  logoUrl: string
  snapshot?: AdminSnapshot
  error?: string | null
  onToggleTheme: () => void
}

export function Topbar({ dark, logoUrl, snapshot, error, onToggleTheme }: TopbarProps) {
  return (
    <header className="topbar-shell">
      <div className="topbar-search" role="search">
        <Search className="size-4 text-muted-foreground" />
        <span className="truncate text-sm text-muted-foreground">搜索机器人、插件或日志</span>
        <span className="ml-auto hidden items-center gap-1 rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground sm:flex">
          <Command className="size-3" />K
        </span>
      </div>

      <div className="ml-auto flex items-center gap-1.5">
        <Badge variant={error ? "danger" : snapshot?.server.restart_required ? "warning" : "success"} className="hidden sm:inline-flex">
          <span className={"size-1.5 rounded-full " + (error ? "bg-destructive" : snapshot?.server.restart_required ? "bg-warning" : "bg-success")} />
          {error ? "API 断开" : snapshot?.server.restart_required ? "待重启" : "实时"}
        </Badge>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={onToggleTheme} aria-label={dark ? "切换到亮色" : "切换到暗色"}>
              {dark ? <Sun /> : <Moon />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>{dark ? "亮色模式" : "暗色模式"}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" aria-label="通知" className="relative">
              <Bell />
              <span className="absolute right-2 top-2 size-1.5 rounded-full bg-destructive ring-2 ring-background" />
            </Button>
          </TooltipTrigger>
          <TooltipContent>通知</TooltipContent>
        </Tooltip>
        <Avatar className="ml-1 size-8">
          <AvatarImage src={logoUrl} alt="QimenBot" />
          <AvatarFallback>QB</AvatarFallback>
        </Avatar>
      </div>
    </header>
  )
}
