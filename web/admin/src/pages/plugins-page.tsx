import { useEffect, useMemo, useState } from "react"
import { PlugZap, RefreshCw } from "lucide-react"
import { toast } from "sonner"

import { api, type PluginView } from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Switch } from "@/components/ui/switch"

export function PluginsPage() {
  const [plugins, setPlugins] = useState<PluginView[]>([])
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState("all")

  const load = async () => {
    setPlugins(await api.plugins())
  }

  useEffect(() => {
    void load().catch((error) => toast.error(error.message))
  }, [])

  const visible = useMemo(() => {
    if (filter === "all") return plugins
    return plugins.filter((plugin) => plugin.kind === filter)
  }, [filter, plugins])

  const toggle = async (plugin: PluginView, enabled: boolean) => {
    setBusy(true)
    try {
      const result = await api.togglePlugin(plugin.id, enabled)
      toast.success(result.message)
      await load()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "插件状态更新失败")
    } finally {
      setBusy(false)
    }
  }

  const reload = async () => {
    setBusy(true)
    try {
      const result = await api.reloadPlugins()
      toast.success(result.message)
      await load()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "插件重载失败")
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>插件</h1>
            <span className="environment-tag">{plugins.length} ITEMS</span>
          </div>
          <p>静态插件保存状态后重启生效，动态插件可即时扫描重载。</p>
        </div>
        <div className="flex items-center gap-2">
          <select className="mini-select" value={filter} onChange={(event) => setFilter(event.target.value)}>
            <option value="all">全部</option>
            <option value="builtin">内置</option>
            <option value="static">静态</option>
            <option value="dynamic">动态</option>
          </select>
          <Button variant="outline" size="sm" onClick={reload} disabled={busy}>
            <RefreshCw className={busy ? "animate-spin-slow" : ""} />
            重载动态插件
          </Button>
        </div>
      </div>

      <section className="panel enter-panel">
        <div className="plugin-grid">
          {visible.map((plugin, index) => (
            <article className="plugin-card" key={plugin.kind + plugin.id} style={{ animationDelay: index * 35 + "ms" }}>
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <PlugZap className="size-4 text-primary" />
                    <h2 className="truncate font-mono text-sm font-extrabold">{plugin.id}</h2>
                  </div>
                  <p className="mt-1 truncate text-[11px] text-muted-foreground">{plugin.file_name ?? plugin.kind}</p>
                </div>
                <Badge variant={plugin.loaded ? "success" : plugin.enabled ? "warning" : "neutral"}>
                  {plugin.loaded ? "loaded" : plugin.enabled ? "pending" : "disabled"}
                </Badge>
              </div>
              <div className="mt-4 grid grid-cols-3 gap-2 text-center">
                <MiniStat label="命令" value={plugin.commands.length} />
                <MiniStat label="路由" value={plugin.routes.length} />
                <MiniStat label="错误" value={plugin.failures} />
              </div>
              <div className="mt-4 min-h-12 text-[11px] text-muted-foreground">
                {plugin.commands.concat(plugin.routes, plugin.webhooks).slice(0, 4).join(" · ") || "未声明可展示入口"}
              </div>
              {plugin.last_error && <div className="mt-3 rounded-md border border-destructive/20 bg-destructive/8 p-2 text-[11px] text-destructive">{plugin.last_error}</div>}
              <div className="mt-4 flex items-center justify-between border-t border-border pt-3">
                <span className="text-[11px] font-semibold text-muted-foreground">{plugin.kind} {plugin.api_version ? " · API " + plugin.api_version : ""}</span>
                <Switch checked={plugin.enabled} disabled={busy || (!plugin.live_toggle && plugin.kind === "builtin")} onCheckedChange={(enabled) => void toggle(plugin, enabled)} />
              </div>
            </article>
          ))}
          {visible.length === 0 && <div className="empty-panel">没有匹配的插件。</div>}
        </div>
      </section>
    </main>
  )
}

function MiniStat({ label, value }: { label: string; value: number }) {
  return (
    <div className="mini-stat">
      <strong>{value}</strong>
      <span>{label}</span>
    </div>
  )
}
