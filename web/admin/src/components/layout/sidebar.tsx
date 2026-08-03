import type { LucideIcon } from "lucide-react"
import {
  Bot,
  ChevronLeft,
  LayoutDashboard,
  Puzzle,
  ScrollText,
  Settings2,
  ShieldCheck,
  Store,
  UploadCloud,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { AdminSnapshot } from "@/lib/api"
import { formatUptime } from "@/lib/format"
import { cn } from "@/lib/utils"

interface NavItem {
  id: string
  label: string
  icon: LucideIcon
  count?: string
}

interface SidebarProps {
  active: string
  collapsed: boolean
  logoUrl: string
  snapshot?: AdminSnapshot
  onNavigate: (id: string) => void
  onCollapse: () => void
}

function NavButton({
  item,
  active,
  collapsed,
  onClick,
}: {
  item: NavItem
  active: boolean
  collapsed: boolean
  onClick: () => void
}) {
  const Icon = item.icon
  const button = (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
      className={cn(
        "group relative flex h-9 w-full items-center gap-3 rounded-md px-2.5 text-sm font-semibold outline-none transition-colors duration-150 focus-visible:ring-2 focus-visible:ring-ring",
        active
          ? "bg-sidebar-active text-sidebar-active-foreground"
          : "text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-foreground",
        collapsed && "justify-center px-0",
      )}
    >
      <Icon className={cn("size-[17px] shrink-0", active && "text-primary")} strokeWidth={1.9} />
      {!collapsed && <span className="min-w-0 flex-1 truncate text-left">{item.label}</span>}
      {!collapsed && item.count && (
        <span className="min-w-5 rounded-full bg-sidebar-count px-1.5 py-0.5 text-center font-mono text-[10px] font-semibold text-sidebar-muted">
          {item.count}
        </span>
      )}
    </button>
  )

  if (!collapsed) return button

  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right">{item.label}</TooltipContent>
    </Tooltip>
  )
}

export function Sidebar({ active, collapsed, logoUrl, snapshot, onNavigate, onCollapse }: SidebarProps) {
  const primaryNav: NavItem[] = [
    { id: "overview", label: "总览", icon: LayoutDashboard },
    { id: "bots", label: "机器人", icon: Bot, count: snapshot ? String(snapshot.metrics.configured_bots) : undefined },
    { id: "logs", label: "实时日志", icon: ScrollText, count: snapshot ? String(snapshot.resources.log_entries) : undefined },
    { id: "plugins", label: "插件", icon: Puzzle, count: snapshot ? String(snapshot.metrics.loaded_dynamic_plugins) : undefined },
    { id: "marketplace", label: "插件商城", icon: Store },
  ]
  const secondaryNav: NavItem[] = [
    { id: "configuration", label: "配置", icon: Settings2, count: snapshot?.server.restart_required ? "!" : undefined },
    { id: "updates", label: "版本更新", icon: UploadCloud },
    { id: "audit", label: "安全审计", icon: ShieldCheck },
  ]

  return (
    <aside className={cn("sidebar-shell", collapsed && "is-collapsed")}>
      <div className="flex h-16 items-center gap-3 border-b border-sidebar-border px-3">
        <img src={logoUrl} alt="QimenBot" className="size-9 shrink-0 rounded-full border border-sidebar-border object-cover" />
        {!collapsed && (
          <div className="min-w-0">
            <div className="truncate font-display text-[15px] font-extrabold text-sidebar-foreground">QimenBot</div>
            <div className="truncate font-mono text-[10px] uppercase text-sidebar-muted">Control plane</div>
          </div>
        )}
      </div>

      <nav className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-2 py-4" aria-label="主导航">
        <div className="space-y-1">
          {!collapsed && <p className="mb-2 px-2.5 text-[10px] font-bold uppercase text-sidebar-label">Workspace</p>}
          {primaryNav.map((item) => (
            <NavButton
              key={item.id}
              item={item}
              active={active === item.id}
              collapsed={collapsed}
              onClick={() => onNavigate(item.id)}
            />
          ))}
        </div>
        <div className="space-y-1">
          {!collapsed && <p className="mb-2 px-2.5 text-[10px] font-bold uppercase text-sidebar-label">System</p>}
          {secondaryNav.map((item) => (
            <NavButton
              key={item.id}
              item={item}
              active={active === item.id}
              collapsed={collapsed}
              onClick={() => onNavigate(item.id)}
            />
          ))}
        </div>
      </nav>

      <div className="border-t border-sidebar-border p-2">
        <div className={cn("mb-2 flex items-center gap-2 rounded-md bg-sidebar-inset p-2", collapsed && "justify-center")}>
          <span className="relative flex size-2.5 shrink-0">
            <span className="absolute inline-flex size-full animate-ping rounded-full bg-success opacity-35" />
            <span className="relative inline-flex size-2.5 rounded-full bg-success" />
          </span>
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="text-xs font-bold text-sidebar-foreground">{snapshot ? "宿主运行中" : "连接中"}</div>
              <div className="font-mono text-[10px] text-sidebar-muted">
                {snapshot ? snapshot.server.version + " · " + formatUptime(snapshot.server.uptime_secs) : "waiting"}
              </div>
            </div>
          )}
        </div>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size={collapsed ? "icon-sm" : "sm"}
              className={cn("w-full text-sidebar-muted hover:bg-sidebar-hover hover:text-sidebar-foreground", !collapsed && "justify-start")}
              onClick={onCollapse}
              aria-label={collapsed ? "展开侧栏" : "收起侧栏"}
            >
              <ChevronLeft className={cn("transition-transform duration-200", collapsed && "rotate-180")} />
              {!collapsed && <span>收起侧栏</span>}
            </Button>
          </TooltipTrigger>
          {collapsed && <TooltipContent side="right">展开侧栏</TooltipContent>}
        </Tooltip>
      </div>
    </aside>
  )
}
