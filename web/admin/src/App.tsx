import { useCallback, useEffect, useMemo, useState } from "react"
import { LockKeyhole, Settings2, X } from "lucide-react"
import { toast, Toaster } from "sonner"

import logoUrl from "../../../logo.jpg"
import { Sidebar } from "@/components/layout/sidebar"
import { Topbar } from "@/components/layout/topbar"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"
import { TooltipProvider } from "@/components/ui/tooltip"
import { api, ApiError, setApiToken, type AdminSnapshot } from "@/lib/api"
import { AuditPage } from "@/pages/audit-page"
import { BotsPage } from "@/pages/bots-page"
import { ConfigPage } from "@/pages/config-page"
import { LogsPage } from "@/pages/logs-page"
import { OverviewPage } from "@/pages/overview-page"
import { PluginsPage } from "@/pages/plugins-page"

type Page = "overview" | "bots" | "logs" | "plugins" | "configuration" | "audit"

function Tweaks({
  open,
  dark,
  compact,
  motion,
  onOpenChange,
  onDarkChange,
  onCompactChange,
  onMotionChange,
}: {
  open: boolean
  dark: boolean
  compact: boolean
  motion: boolean
  onOpenChange: (open: boolean) => void
  onDarkChange: (dark: boolean) => void
  onCompactChange: (compact: boolean) => void
  onMotionChange: (motion: boolean) => void
}) {
  return (
    <div className="tweaks-root">
      {open && (
        <section className="tweaks-panel" aria-label="Tweaks">
          <div className="flex items-center justify-between border-b border-border px-3.5 py-3">
            <div>
              <h2 className="text-sm font-extrabold text-foreground">Tweaks</h2>
              <p className="mt-0.5 text-[10px] text-muted-foreground">本地显示偏好</p>
            </div>
            <Button variant="ghost" size="icon-sm" onClick={() => onOpenChange(false)} aria-label="关闭 Tweaks">
              <X />
            </Button>
          </div>
          <div className="space-y-3 p-3.5">
            <label className="tweak-row">
              <span><strong>暗色模式</strong><small>中性石墨表面</small></span>
              <Switch checked={dark} onCheckedChange={onDarkChange} />
            </label>
            <label className="tweak-row">
              <span><strong>紧凑密度</strong><small>减少纵向间距</small></span>
              <Switch checked={compact} onCheckedChange={onCompactChange} />
            </label>
            <label className="tweak-row">
              <span><strong>界面动效</strong><small>状态与页面过渡</small></span>
              <Switch checked={motion} onCheckedChange={onMotionChange} />
            </label>
          </div>
        </section>
      )}
      <Button
        variant="default"
        size="icon"
        className="tweaks-trigger"
        onClick={() => onOpenChange(!open)}
        aria-label="打开 Tweaks"
        aria-expanded={open}
      >
        <Settings2 />
      </Button>
    </div>
  )
}

function App() {
  const [active, setActive] = useState<Page>("overview")
  const [collapsed, setCollapsed] = useState(false)
  const [dark, setDark] = useState(() => localStorage.getItem("qimen-theme") === "dark")
  const [compact, setCompact] = useState(false)
  const [motion, setMotion] = useState(true)
  const [tweaksOpen, setTweaksOpen] = useState(false)
  const [snapshot, setSnapshot] = useState<AdminSnapshot>()
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [authRequired, setAuthRequired] = useState(false)
  const [authToken, setAuthToken] = useState("")

  const refreshSnapshot = useCallback(async () => {
    try {
      const next = await api.snapshot()
      setSnapshot(next)
      setError(null)
      setAuthRequired(false)
    } catch (caught) {
      if (caught instanceof ApiError && caught.status === 401) setAuthRequired(true)
      setError(caught instanceof Error ? caught.message : "管理 API 连接失败")
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshSnapshot()
    const timer = window.setInterval(() => void refreshSnapshot(), 2500)
    return () => window.clearInterval(timer)
  }, [refreshSnapshot])

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark)
    document.documentElement.classList.toggle("compact", compact)
    document.documentElement.classList.toggle("reduce-motion", !motion)
    localStorage.setItem("qimen-theme", dark ? "dark" : "light")
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", dark ? "#171816" : "#f6f7f5")
  }, [dark, compact, motion])

  const handleBotAction = useCallback(
    async (id: string, action: "start" | "stop" | "reconnect") => {
      try {
        const result = await api.botAction(id, action)
        toast.success(result.message)
        await refreshSnapshot()
      } catch (caught) {
        toast.error(caught instanceof Error ? caught.message : "Bot 操作失败")
      }
    },
    [refreshSnapshot],
  )

  const content = useMemo(() => {
    if (active === "overview") {
      return (
        <OverviewPage
          snapshot={snapshot}
          loading={loading}
          error={error}
          onRefresh={refreshSnapshot}
          onNavigate={(page) => setActive(page as Page)}
          onBotAction={handleBotAction}
        />
      )
    }
    if (active === "bots") return <BotsPage snapshotBots={snapshot?.bots ?? []} onRefreshSnapshot={refreshSnapshot} />
    if (active === "logs") return <LogsPage />
    if (active === "plugins") return <PluginsPage />
    if (active === "configuration") return <ConfigPage onRefreshSnapshot={refreshSnapshot} />
    return <AuditPage />
  }, [active, error, handleBotAction, loading, refreshSnapshot, snapshot])

  if (authRequired) {
    return (
      <main className="auth-screen">
        <section className="auth-panel">
          <img src={logoUrl} alt="QimenBot" />
          <div className="auth-icon"><LockKeyhole /></div>
          <h1>连接管理面板</h1>
          <p>此宿主已启用 Bearer Token，请输入管理 Token。</p>
          <Input
            type="password"
            value={authToken}
            onChange={(event) => setAuthToken(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                setApiToken(authToken)
                setLoading(true)
                void refreshSnapshot()
              }
            }}
            placeholder="QIMEN_ADMIN_TOKEN"
            autoFocus
          />
          <Button
            className="w-full"
            disabled={!authToken.trim() || loading}
            onClick={() => {
              setApiToken(authToken)
              setLoading(true)
              void refreshSnapshot()
            }}
          >
            {loading ? "正在验证" : "进入面板"}
          </Button>
          {error && <span className="text-xs text-destructive">{error}</span>}
        </section>
      </main>
    )
  }

  return (
    <TooltipProvider delayDuration={350}>
      <div className={"app-shell " + (collapsed ? "sidebar-collapsed" : "")}>
        <Sidebar
          active={active}
          collapsed={collapsed}
          logoUrl={logoUrl}
          snapshot={snapshot}
          onNavigate={(page) => setActive(page as Page)}
          onCollapse={() => setCollapsed((value) => !value)}
        />
        <div className="app-content">
          <Topbar dark={dark} logoUrl={logoUrl} snapshot={snapshot} error={error} onToggleTheme={() => setDark((value) => !value)} />
          <div key={active} className="page-transition">{content}</div>
        </div>
        <Tweaks
          open={tweaksOpen}
          dark={dark}
          compact={compact}
          motion={motion}
          onOpenChange={setTweaksOpen}
          onDarkChange={setDark}
          onCompactChange={setCompact}
          onMotionChange={setMotion}
        />
        <Toaster position="bottom-right" richColors closeButton theme={dark ? "dark" : "light"} />
      </div>
    </TooltipProvider>
  )
}

export default App
