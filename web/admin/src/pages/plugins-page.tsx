import { lazy, Suspense, useEffect, useMemo, useState } from "react"
import {
  AlertTriangle,
  ArrowDownWideNarrow,
  Boxes,
  Braces,
  Check,
  CheckCircle2,
  Code2,
  FileCode2,
  Gauge,
  PlugZap,
  RefreshCw,
  Search,
  Settings2,
  ShieldCheck,
  Webhook,
  LoaderCircle,
} from "lucide-react"
import { toast } from "sonner"

import { api, type PluginView } from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Switch } from "@/components/ui/switch"

const PluginConfigDrawer = lazy(() =>
  import("@/components/plugin-config/plugin-config-drawer").then((module) => ({
    default: module.PluginConfigDrawer,
  })),
)

type PluginFilter = "all" | "builtin" | "static" | "dynamic" | "issues"
type PluginSort = "priority" | "name" | "kind"

const builtinCopy: Record<string, { name: string; description: string }> = {
  command: { name: "命令系统", description: "解析命令、匹配权限与作用域，并分发到已注册处理器。" },
  admin: { name: "管理能力", description: "提供宿主管理与运行维护相关的基础能力。" },
  scheduler: { name: "任务调度", description: "承载定时任务和周期性后台作业。" },
  bridge: { name: "消息桥接", description: "在已配置的机器人和消息端点之间转发事件。" },
}

const filterLabels: Array<{ id: PluginFilter; label: string }> = [
  { id: "all", label: "全部" },
  { id: "builtin", label: "内置" },
  { id: "static", label: "静态" },
  { id: "dynamic", label: "动态" },
  { id: "issues", label: "需处理" },
]

export function PluginsPage({ onOpenConfig }: { onOpenConfig?: () => void }) {
  const [plugins, setPlugins] = useState<PluginView[]>([])
  const [busy, setBusy] = useState(false)
  const [filter, setFilter] = useState<PluginFilter>("all")
  const [sort, setSort] = useState<PluginSort>("priority")
  const [query, setQuery] = useState("")
  const [configPlugin, setConfigPlugin] = useState<PluginView | null>(null)

  const load = async () => {
    setPlugins(await api.plugins())
  }

  useEffect(() => {
    void load().catch((error) => toast.error(error.message))
  }, [])

  const counts = useMemo(() => {
    const issues = plugins.filter(hasIssue).length
    return {
      all: plugins.length,
      builtin: plugins.filter((plugin) => plugin.kind === "builtin").length,
      static: plugins.filter((plugin) => plugin.kind === "static").length,
      dynamic: plugins.filter((plugin) => plugin.kind === "dynamic").length,
      issues,
      loaded: plugins.filter((plugin) => plugin.loaded).length,
      endpoints: plugins.reduce(
        (total, plugin) => total + plugin.commands.length + plugin.routes.length + (plugin.system_plugins?.length ?? 0) + plugin.webhooks.length,
        0,
      ),
      failures: plugins.reduce((total, plugin) => total + plugin.failures, 0),
    }
  }, [plugins])

  const visible = useMemo(() => {
    const needle = query.trim().toLowerCase()
    const filtered = plugins.filter((plugin) => {
      if (filter === "issues" ? !hasIssue(plugin) : filter !== "all" && plugin.kind !== filter) return false
      if (!needle) return true
      return [
        plugin.id,
        plugin.name,
        plugin.description,
        plugin.file_name,
        plugin.version,
        plugin.api_version,
        plugin.config_apply_mode,
        ...plugin.commands,
        ...plugin.routes,
        ...(plugin.system_plugins ?? []),
        ...plugin.webhooks,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase()
        .includes(needle)
    })
    return filtered.sort((left, right) => comparePlugins(left, right, sort))
  }, [filter, plugins, query, sort])

  const rankById = useMemo(() => {
    const ranked = plugins
      .filter((plugin) => plugin.kind !== "builtin")
      .slice()
      .sort((left, right) => comparePlugins(left, right, "priority"))
    return new Map(ranked.map((plugin, index) => [plugin.id, index + 1]))
  }, [plugins])

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

  const updatePriority = async (plugin: PluginView, priority: number) => {
    setBusy(true)
    try {
      const result = await api.updatePluginPriority(plugin.id, priority)
      toast.success(result.message)
      await load()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "插件优先级更新失败")
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>插件</h1>
            <span className="environment-tag">{plugins.length} DISCOVERED</span>
          </div>
          <p>查看实际注册能力并调整命令优先级；数值越大，命令冲突时越先匹配。</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {onOpenConfig && (
            <Button variant="outline" size="sm" onClick={onOpenConfig}>
              <Settings2 />
              配置模块
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={reload} disabled={busy}>
            <RefreshCw className={busy ? "animate-spin-slow" : ""} />
            重载动态插件
          </Button>
        </div>
      </div>

      <section className="plugin-summary enter-panel" aria-label="插件运行摘要">
        <SummaryMetric icon={CheckCircle2} label="已加载" value={counts.loaded} tone="success" />
        <SummaryMetric icon={Boxes} label="已发现" value={counts.all} />
        <SummaryMetric icon={Braces} label="命令与路由" value={counts.endpoints} />
        <SummaryMetric icon={AlertTriangle} label="失败次数" value={counts.failures} tone={counts.failures > 0 ? "warning" : "neutral"} />
      </section>

      <section className="plugin-toolbar enter-panel" aria-label="插件筛选">
        <div className="topbar-search plugin-search">
          <Search className="size-4 text-muted-foreground" />
          <Input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索 ID、命令或文件名"
            className="h-7 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
          />
        </div>
        <div className="plugin-filter" role="group" aria-label="插件类型">
          {filterLabels.map((item) => (
            <button
              type="button"
              key={item.id}
              className={filter === item.id ? "is-active" : ""}
              aria-pressed={filter === item.id}
              onClick={() => setFilter(item.id)}
            >
              <span>{item.label}</span>
              <span>{counts[item.id]}</span>
            </button>
          ))}
        </div>
        <label className="plugin-sort-control">
          <ArrowDownWideNarrow />
          <span>排序</span>
          <select value={sort} onChange={(event) => setSort(event.target.value as PluginSort)}>
            <option value="priority">优先级</option>
            <option value="name">插件名称</option>
            <option value="kind">插件类型</option>
          </select>
        </label>
      </section>

      <section className="plugin-grid" aria-live="polite">
        {visible.map((plugin, index) => (
          <PluginCard
            plugin={plugin}
            busy={busy}
            onToggle={toggle}
            onOpenConfig={onOpenConfig}
            onConfigure={setConfigPlugin}
            onPriorityChange={updatePriority}
            rank={rankById.get(plugin.id)}
            key={plugin.kind + plugin.id}
            delay={Math.min(index, 8) * 35}
          />
        ))}
        {visible.length === 0 && (
          <div className="panel empty-panel plugin-empty">
            {plugins.length === 0 ? "当前二进制没有发现可展示的模块或插件。" : "没有符合筛选条件的插件。"}
          </div>
        )}
      </section>
    </main>
    {configPlugin && (
      <Suspense fallback={
        <div className="plugin-config-lazy" role="status">
          <LoaderCircle className="animate-spin-slow" />
          <span>正在打开插件配置</span>
        </div>
      }>
        <PluginConfigDrawer
          plugin={configPlugin}
          onOpenChange={(open) => { if (!open) setConfigPlugin(null) }}
          onSaved={load}
        />
      </Suspense>
    )}
    </>
  )
}

function PluginCard({
  plugin,
  busy,
  onToggle,
  onOpenConfig,
  onConfigure,
  onPriorityChange,
  rank,
  delay,
}: {
  plugin: PluginView
  busy: boolean
  onToggle: (plugin: PluginView, enabled: boolean) => Promise<void>
  onOpenConfig?: () => void
  onConfigure: (plugin: PluginView) => void
  onPriorityChange: (plugin: PluginView, priority: number) => Promise<void>
  rank?: number
  delay: number
}) {
  const status = pluginStatus(plugin)
  const copy = plugin.kind === "builtin" ? builtinCopy[plugin.id] : undefined
  const name = copy?.name ?? plugin.name ?? plugin.id
  const description = copy?.description ?? plugin.description
  const systemPlugins = plugin.system_plugins ?? []
  const interceptorCount = plugin.interceptors ?? 0
  const configured = plugin.configured !== false
  const available = plugin.available !== false
  const canToggle = configured && available && plugin.kind !== "builtin"
  const capabilityCount = plugin.commands.length + plugin.routes.length + systemPlugins.length + plugin.webhooks.length + interceptorCount
  const KindIcon = plugin.kind === "builtin" ? ShieldCheck : plugin.kind === "dynamic" ? PlugZap : FileCode2

  return (
    <article className="plugin-card" style={{ animationDelay: delay + "ms" }}>
      <div className="plugin-card-head">
        <span className={"plugin-kind-icon is-" + plugin.kind}><KindIcon /></span>
        <div className="min-w-0 flex-1">
          <div className="plugin-title-row">
            <h2 title={name}>{name}</h2>
            <Badge variant="neutral">{kindLabel(plugin.kind)}</Badge>
            {rank && <span className="plugin-priority-rank">#{String(rank).padStart(2, "0")}</span>}
          </div>
          <code title={plugin.id}>{plugin.id}</code>
        </div>
        <Badge variant={status.variant}>{status.label}</Badge>
      </div>

      <p className={description ? "plugin-description" : "plugin-description is-muted"}>
        {description || "该模块没有声明说明文本。"}
      </p>

      <div className="plugin-metadata" aria-label="插件元数据">
        <Metadata label="版本" value={plugin.version || "未声明"} />
        <Metadata label="API" value={plugin.api_version || (plugin.kind === "builtin" ? "内部" : "未声明")} />
        <Metadata label="来源" value={plugin.file_name || kindLabel(plugin.kind)} title={plugin.file_name ?? undefined} />
        <Metadata label="配置" value={configCapabilityLabel(plugin)} />
      </div>

      <div className="plugin-capability-counts" aria-label="能力统计">
        <CapabilityCount label="命令" value={plugin.commands.length} />
        <CapabilityCount label="事件路由" value={plugin.routes.length + systemPlugins.length} />
        <CapabilityCount label="Webhook" value={plugin.webhooks.length} />
        <CapabilityCount label="拦截器" value={interceptorCount} />
      </div>

      <div className="plugin-capabilities">
        <CapabilityList icon={Code2} label="命令" values={plugin.commands} />
        <CapabilityList icon={Braces} label="事件路由" values={[...systemPlugins, ...plugin.routes]} />
        <CapabilityList icon={Webhook} label="Webhook" values={plugin.webhooks} />
        {capabilityCount === 0 && <span className="plugin-no-capability">未声明可展示的命令或事件入口</span>}
      </div>

      {plugin.last_error && (
        <div className="plugin-error">
          <AlertTriangle />
          <span>{plugin.last_error}</span>
        </div>
      )}

      <div className="plugin-priority-row">
        <div className="plugin-priority-summary">
          <span className="plugin-priority-icon"><Gauge /></span>
          <span>
            <small>命令优先级</small>
            <strong>{plugin.priority}</strong>
          </span>
          <Badge variant={plugin.priority_custom ? "default" : "neutral"}>
            {plugin.kind === "builtin" ? "系统保留" : plugin.priority_custom ? "管理员设置" : "默认规则"}
          </Badge>
        </div>
        {plugin.kind !== "builtin" && (
          <PriorityEditor plugin={plugin} busy={busy} onSave={onPriorityChange} />
        )}
      </div>

      <div className="plugin-card-footer">
        <div className="plugin-activation-copy">
          <strong>{activationTitle(plugin)}</strong>
          <span>{activationDescription(plugin)}</span>
        </div>
        {!configured && onOpenConfig ? (
          <Button variant="outline" size="sm" onClick={onOpenConfig}>
            <Settings2 />
            添加
          </Button>
        ) : plugin.kind === "builtin" ? (
          <Badge variant="neutral">由配置管理</Badge>
        ) : (
          <div className="plugin-card-actions">
            {plugin.configurable && (
              <Button variant="outline" size="sm" onClick={() => onConfigure(plugin)} disabled={busy}>
                <Settings2 />
                {plugin.config_file_exists ? "配置" : "设置"}
              </Button>
            )}
            <div className="plugin-switch">
              <span>{plugin.enabled ? "已启用" : "已停用"}</span>
              <Switch
                checked={plugin.enabled}
                disabled={busy || !canToggle}
                aria-label={(plugin.enabled ? "停用 " : "启用 ") + plugin.id}
                onCheckedChange={(enabled) => void onToggle(plugin, enabled)}
              />
            </div>
          </div>
        )}
      </div>
    </article>
  )
}

function PriorityEditor({
  plugin,
  busy,
  onSave,
}: {
  plugin: PluginView
  busy: boolean
  onSave: (plugin: PluginView, priority: number) => Promise<void>
}) {
  const [value, setValue] = useState(String(plugin.priority))

  useEffect(() => setValue(String(plugin.priority)), [plugin.priority])

  const priority = Number(value)
  const valid = value.trim() !== "" && Number.isInteger(priority) && priority >= 0 && priority <= 1_000
  const dirty = valid && priority !== plugin.priority
  return (
    <div className="plugin-priority-editor">
      <Input
        type="number"
        min={0}
        max={1_000}
        step={1}
        value={value}
        disabled={busy}
        aria-invalid={!valid}
        aria-label={`${plugin.id} 命令优先级`}
        title="请输入 0 到 1000 之间的整数"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && dirty) void onSave(plugin, priority)
        }}
      />
      <Button
        type="button"
        variant="outline"
        size="icon-sm"
        disabled={busy || !dirty}
        title="应用优先级"
        aria-label={`应用 ${plugin.id} 优先级`}
        onClick={() => void onSave(plugin, priority)}
      >
        <Check />
      </Button>
    </div>
  )
}

function SummaryMetric({
  icon: Icon,
  label,
  value,
  tone = "default",
}: {
  icon: typeof Boxes
  label: string
  value: number
  tone?: "default" | "success" | "warning" | "neutral"
}) {
  return (
    <div className={"plugin-summary-item is-" + tone}>
      <Icon />
      <span>
        <small>{label}</small>
        <strong>{value}</strong>
      </span>
    </div>
  )
}

function Metadata({ label, value, title }: { label: string; value: string; title?: string }) {
  return (
    <span title={title}>
      <small>{label}</small>
      <strong>{value}</strong>
    </span>
  )
}

function CapabilityCount({ label, value }: { label: string; value: number }) {
  return (
    <span>
      <strong>{value}</strong>
      <small>{label}</small>
    </span>
  )
}

function CapabilityList({ icon: Icon, label, values }: { icon: typeof Code2; label: string; values: string[] }) {
  if (values.length === 0) return null
  return (
    <div className="plugin-capability-group">
      <span><Icon />{label}</span>
      <div>
        {values.slice(0, 6).map((value) => <code key={value} title={value}>{value}</code>)}
        {values.length > 6 && <code>+{values.length - 6}</code>}
      </div>
    </div>
  )
}

function pluginStatus(plugin: PluginView): { label: string; variant: "success" | "warning" | "danger" | "neutral" } {
  if (plugin.available === false) return { label: "不可用", variant: "danger" }
  if (plugin.configured === false) return { label: "可添加", variant: "neutral" }
  if (plugin.last_error) return { label: "异常", variant: "danger" }
  if (plugin.failures > 0) return { label: "有失败", variant: "warning" }
  if (!plugin.enabled) return { label: "已停用", variant: "neutral" }
  if (plugin.loaded) return { label: "已加载", variant: "success" }
  return { label: "待重启", variant: "warning" }
}

function hasIssue(plugin: PluginView) {
  return plugin.available === false || Boolean(plugin.last_error) || plugin.failures > 0 || (plugin.enabled && !plugin.loaded)
}

function kindLabel(kind: string) {
  if (kind === "builtin") return "内置模块"
  if (kind === "static") return "静态插件"
  if (kind === "dynamic") return "动态插件"
  return kind
}

function activationTitle(plugin: PluginView) {
  if (plugin.available === false) return "当前构建不可用"
  if (plugin.configured === false) return "尚未加入宿主配置"
  if (plugin.kind === "builtin") return "跟随宿主生命周期"
  return plugin.live_toggle ? "支持即时切换" : "状态在重启后生效"
}

function activationDescription(plugin: PluginView) {
  if (plugin.available === false) return "配置已保留，请检查当前二进制是否包含该模块。"
  if (plugin.configured === false) return "前往配置页选择后，保存并重启宿主。"
  if (plugin.kind === "builtin") return "内置模块通过全局模块配置加载。"
  if (plugin.live_toggle) return "启停与重新扫描会直接应用到运行时。"
  return "开关写入插件状态文件，不会中断当前进程。"
}

function configCapabilityLabel(plugin: PluginView) {
  if (plugin.kind !== "dynamic") return "宿主配置"
  if (!plugin.configurable) return "未声明"
  if (plugin.config_apply_mode === "live") return plugin.config_file_exists ? "即时 · 已配置" : "即时 · 待设置"
  if (plugin.config_apply_mode === "reload") return plugin.config_file_exists ? "重载 · 已配置" : "重载 · 待设置"
  return plugin.config_file_exists ? "重启 · 已配置" : "重启 · 待设置"
}

function comparePlugins(left: PluginView, right: PluginView, sort: PluginSort) {
  if (sort === "priority") {
    return right.priority - left.priority || left.id.localeCompare(right.id)
  }
  if (sort === "name") {
    return (left.name ?? left.id).localeCompare(right.name ?? right.id, "zh-CN")
  }
  return left.kind.localeCompare(right.kind) || left.id.localeCompare(right.id)
}
