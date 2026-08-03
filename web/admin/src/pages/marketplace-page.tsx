import { useCallback, useEffect, useRef, useState } from "react"
import * as AlertDialog from "@radix-ui/react-alert-dialog"
import {
  AlertTriangle,
  BadgeCheck,
  Bell,
  Box,
  Cable,
  Check,
  ChevronLeft,
  ChevronRight,
  CircleOff,
  Download,
  ExternalLink,
  FileCode2,
  Fingerprint,
  GitBranch,
  History,
  LoaderCircle,
  LockKeyhole,
  MessageSquare,
  PackageCheck,
  PackageOpen,
  PackageSearch,
  Pin,
  PinOff,
  RefreshCw,
  RotateCcw,
  Search,
  Send,
  ShieldAlert,
  ShieldCheck,
  Sparkles,
  Trash2,
  Users,
} from "lucide-react"
import { toast } from "sonner"

import {
  api,
  type MarketplaceDriverSupport,
  type MarketplaceFilter,
  type MarketplacePluginSummaryView,
  type MarketplacePluginView,
  type MarketplaceVersionView,
  type MarketplaceView,
} from "@/lib/api"
import { formatBytes } from "@/lib/format"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Select } from "@/components/ui/select"

type PendingAction = {
  kind: "install" | "adopt" | "rollback" | "uninstall"
  plugin: MarketplacePluginView
  version?: string
}

const filterLabels: Array<{ id: MarketplaceFilter; label: string }> = [
  { id: "all", label: "全部" },
  { id: "dynamic", label: "动态" },
  { id: "static", label: "静态" },
  { id: "installed", label: "已安装" },
  { id: "updates", label: "可更新" },
]

const pageSizeOptions = [10, 20, 50]

export function MarketplacePage({ onOpenConfig }: { onOpenConfig?: () => void }) {
  const [market, setMarket] = useState<MarketplaceView | null>(null)
  const [loading, setLoading] = useState(true)
  const [refreshing, setRefreshing] = useState(false)
  const [busyId, setBusyId] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [query, setQuery] = useState("")
  const [debouncedQuery, setDebouncedQuery] = useState("")
  const [filter, setFilter] = useState<MarketplaceFilter>("all")
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [selected, setSelected] = useState<MarketplacePluginView | null>(null)
  const [detailLoading, setDetailLoading] = useState(false)
  const [detailError, setDetailError] = useState<string | null>(null)
  const [catalogRevision, setCatalogRevision] = useState(0)
  const [selectedVersions, setSelectedVersions] = useState<Record<string, string>>({})
  const [pending, setPending] = useState<PendingAction | null>(null)
  const listRequestId = useRef(0)
  const detailRequestId = useRef(0)

  const load = useCallback(async (refresh = false) => {
    const requestId = ++listRequestId.current
    if (refresh) setRefreshing(true)
    else setLoading(true)
    try {
      const params = { page, page_size: pageSize, query: debouncedQuery, filter }
      const next = refresh ? await api.refreshMarketplace(params) : await api.marketplace(params)
      if (requestId !== listRequestId.current) return
      setMarket(next)
      setPage(next.pagination.page)
      setError(null)
      setSelectedId((current) =>
        current && next.plugins.some((plugin) => plugin.id === current)
          ? current
          : next.plugins[0]?.id ?? null,
      )
      setCatalogRevision((current) => current + 1)
    } catch (caught) {
      if (requestId !== listRequestId.current) return
      setError(caught instanceof Error ? caught.message : "插件目录连接失败")
    } finally {
      if (requestId === listRequestId.current) {
        setLoading(false)
        setRefreshing(false)
      }
    }
  }, [debouncedQuery, filter, page, pageSize])

  useEffect(() => {
    void load()
  }, [load])

  useEffect(() => {
    const timer = window.setTimeout(() => setDebouncedQuery(query.trim()), 250)
    return () => window.clearTimeout(timer)
  }, [query])

  useEffect(() => {
    const requestId = ++detailRequestId.current
    if (!selectedId) {
      setSelected(null)
      setDetailError(null)
      setDetailLoading(false)
      return
    }
    setSelected(null)
    setDetailError(null)
    setDetailLoading(true)
    void api.marketplacePlugin(selectedId)
      .then((plugin) => {
        if (requestId !== detailRequestId.current) return
        setSelected(plugin)
      })
      .catch((caught) => {
        if (requestId !== detailRequestId.current) return
        setDetailError(caught instanceof Error ? caught.message : "插件详情读取失败")
      })
      .finally(() => {
        if (requestId === detailRequestId.current) setDetailLoading(false)
      })
  }, [catalogRevision, selectedId])

  const counts = market?.counts ?? { all: 0, dynamic: 0, static: 0, installed: 0, updates: 0 }
  const plugins = market?.plugins ?? []
  const selectedVersion = selected ? resolveSelectedVersion(selected, selectedVersions[selected.id]) : null

  const run = async (action: PendingAction) => {
    setBusyId(action.plugin.id)
    try {
      const result =
        action.kind === "install"
          ? await api.installMarketplacePlugin(action.plugin.id, action.version)
          : action.kind === "adopt"
            ? await api.adoptMarketplacePlugin(action.plugin.id, action.version)
            : action.kind === "rollback"
              ? await api.rollbackMarketplacePlugin(action.plugin.id)
              : await api.uninstallMarketplacePlugin(action.plugin.id)
      toast.success(result.message)
      await load()
    } catch (caught) {
      toast.error(caught instanceof Error ? caught.message : "插件操作失败")
    } finally {
      setBusyId(null)
    }
  }

  const togglePin = async (plugin: MarketplacePluginView) => {
    if (!plugin.installed) return
    setBusyId(plugin.id)
    try {
      const result = await api.pinMarketplacePlugin(plugin.id, !plugin.installed.pinned)
      toast.success(result.message)
      await load()
    } catch (caught) {
      toast.error(caught instanceof Error ? caught.message : "版本固定状态更新失败")
    } finally {
      setBusyId(null)
    }
  }

  const disabled = market && !market.enabled

  return (
    <main className="page-shell marketplace-page">
      <div className="page-heading enter-item">
        <div>
          <div className="flex flex-wrap items-center gap-2.5">
            <h1>插件商城</h1>
            <span className="environment-tag">GITHUB CATALOG</span>
            {market?.source && (
              <Badge variant={market.source === "network" ? "success" : "neutral"}>
                {market.source === "network" ? "目录已同步" : "本地缓存"}
              </Badge>
            )}
          </div>
          <p>从公开源码仓库安装与当前宿主匹配的动态插件。</p>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {onOpenConfig && (
            <Button variant="outline" size="sm" onClick={onOpenConfig}>
              <LockKeyhole />
              商城设置
            </Button>
          )}
          <Button variant="outline" size="sm" onClick={() => void load(true)} disabled={refreshing || Boolean(busyId)}>
            <RefreshCw className={refreshing ? "animate-spin-slow" : ""} />
            {refreshing ? "同步中" : "刷新目录"}
          </Button>
        </div>
      </div>

      {market && <HostStrip market={market} />}

      <section className="marketplace-safety enter-panel" aria-label="第三方插件安全说明">
        <ShieldAlert />
        <div>
          <strong>动态插件与宿主运行在同一进程</strong>
          <span>安装会执行第三方代码。源码公开不等于安全，请先检查仓库、权限和版本变更。</span>
        </div>
        <Badge variant="warning">自动更新关闭</Badge>
      </section>

      {(market?.warning || error) && (
        <section className="marketplace-warning" role="status">
          <AlertTriangle />
          <span>{error || market?.warning}</span>
          <Button variant="ghost" size="sm" onClick={() => void load(true)}>重试</Button>
        </section>
      )}

      {disabled ? (
        <section className="panel marketplace-disabled enter-panel">
          <CircleOff />
          <div>
            <h2>插件商城已关闭</h2>
            <p>在配置页开启商城后，目录才会连接 GitHub Pages。现有插件不会受到影响。</p>
          </div>
          {onOpenConfig && <Button onClick={onOpenConfig}>打开配置</Button>}
        </section>
      ) : (
        <>
          <section className="marketplace-toolbar enter-panel" aria-label="商城筛选">
            <div className="topbar-search marketplace-search">
              <Search />
              <Input
                value={query}
                onChange={(event) => {
                  setQuery(event.target.value)
                  setPage(1)
                }}
                placeholder="搜索插件、驱动、场景或分类"
                className="h-7 border-0 bg-transparent px-0 shadow-none focus-visible:ring-0"
              />
            </div>
            <div className="plugin-filter marketplace-filter" role="group" aria-label="插件类型">
              {filterLabels.map((item) => (
                <button
                  type="button"
                  key={item.id}
                  className={filter === item.id ? "is-active" : ""}
                  aria-pressed={filter === item.id}
                  onClick={() => {
                    setFilter(item.id)
                    setPage(1)
                  }}
                >
                  <span>{item.label}</span>
                  <span>{counts[item.id]}</span>
                </button>
              ))}
            </div>
          </section>

          <section className="marketplace-workspace enter-panel" aria-live="polite">
            <div className="marketplace-catalog" aria-label="插件目录">
              <div className="marketplace-list-head">
                <span>目录</span>
                <small>{loading ? "正在读取" : `${market?.pagination.total_items ?? 0} 个结果`}</small>
              </div>
              {loading ? (
                <MarketplaceSkeleton />
              ) : plugins.length ? (
                <div className="marketplace-list">
                  {plugins.map((plugin) => (
                    <PluginRow
                      key={plugin.id}
                      plugin={plugin}
                      selected={plugin.id === selectedId}
                      busy={busyId === plugin.id}
                      onSelect={() => setSelectedId(plugin.id)}
                    />
                  ))}
                </div>
              ) : (
                <div className="marketplace-empty">
                  <PackageSearch />
                  <strong>{counts.all ? "没有符合条件的插件" : "目录暂时没有插件"}</strong>
                  <span>{counts.all ? "调整搜索词或筛选条件。" : "首个通过审核的插件会显示在这里。"}</span>
                </div>
              )}
              {market && market.pagination.total_items > 0 && (
                <MarketplacePagination
                  pagination={market.pagination}
                  pageSize={pageSize}
                  disabled={loading || refreshing || Boolean(busyId)}
                  onPageChange={setPage}
                  onPageSizeChange={(nextPageSize) => {
                    setPageSize(nextPageSize)
                    setPage(1)
                  }}
                />
              )}
            </div>

            <div className="marketplace-detail-wrap">
              {detailLoading ? (
                <div className="marketplace-detail-empty">
                  <LoaderCircle className="animate-spin-slow" />
                  <span>正在读取插件详情。</span>
                </div>
              ) : detailError ? (
                <div className="marketplace-detail-empty is-error">
                  <AlertTriangle />
                  <span>{detailError}</span>
                  {selectedId && <Button variant="outline" size="sm" onClick={() => setCatalogRevision((current) => current + 1)}>重试</Button>}
                </div>
              ) : selected ? (
                <PluginDetail
                  plugin={selected}
                  selectedVersion={selectedVersion}
                  busy={busyId === selected.id}
                  onVersionChange={(version) =>
                    setSelectedVersions((current) => ({ ...current, [selected.id]: version }))
                  }
                  onInstall={(version) => setPending({ kind: "install", plugin: selected, version })}
                  onAdopt={() => setPending({ kind: "adopt", plugin: selected, version: selected.unmanaged?.version })}
                  onPin={() => void togglePin(selected)}
                  onRollback={() => setPending({ kind: "rollback", plugin: selected })}
                  onUninstall={() => setPending({ kind: "uninstall", plugin: selected })}
                />
              ) : (
                <div className="marketplace-detail-empty">
                  <Box />
                  <span>选择一个插件查看版本和安装信息。</span>
                </div>
              )}
            </div>
          </section>
        </>
      )}

      <ConfirmAction pending={pending} busy={Boolean(busyId)} onClose={() => setPending(null)} onConfirm={run} />
    </main>
  )
}

function HostStrip({ market }: { market: MarketplaceView }) {
  return (
    <section className="marketplace-host enter-panel" aria-label="当前宿主兼容信息">
      <HostFact icon={PackageCheck} label="QimenBot" value={market.host.qimenbot_version} />
      <HostFact icon={GitBranch} label="目标平台" value={market.host.target} />
      <HostFact
        icon={Fingerprint}
        label="动态 ABI"
        value={market.host.supported_dynamic_apis.at(-1) ? `0.1-${market.host.supported_dynamic_apis.at(-1)}` : "未知"}
      />
      <HostFact
        icon={market.host.dynamic_loading ? Check : CircleOff}
        label="动态加载"
        value={market.host.dynamic_loading ? (market.host.glibc ? `glibc ${market.host.glibc}` : "可用") : "不可用"}
        tone={market.host.dynamic_loading ? "success" : "danger"}
      />
    </section>
  )
}

function HostFact({
  icon: Icon,
  label,
  value,
  tone = "default",
}: {
  icon: typeof PackageCheck
  label: string
  value: string
  tone?: "default" | "success" | "danger"
}) {
  return (
    <div className={cn("marketplace-host-fact", `is-${tone}`)}>
      <Icon />
      <span><small>{label}</small><strong title={value}>{value}</strong></span>
    </div>
  )
}

function PluginRow({
  plugin,
  selected,
  busy,
  onSelect,
}: {
  plugin: MarketplacePluginSummaryView
  selected: boolean
  busy: boolean
  onSelect: () => void
}) {
  const Icon = plugin.kind === "dynamic" ? PackageOpen : FileCode2
  const state = pluginState(plugin)
  return (
    <button type="button" className={cn("marketplace-row", selected && "is-selected")} onClick={onSelect}>
      <span className={cn("marketplace-row-icon", plugin.kind === "static" && "is-static")}>
        {busy ? <LoaderCircle className="animate-spin-slow" /> : <Icon />}
      </span>
      <span className="marketplace-row-copy">
        <span className="marketplace-row-title">
          <strong>{plugin.name}</strong>
          <code>{plugin.id}</code>
        </span>
        <span className="marketplace-row-summary">{plugin.summary}</span>
        {plugin.drivers.length > 0 && (
          <span className="marketplace-row-drivers">
            {plugin.drivers.map((support) => <DriverBadge key={support.driver} driver={support.driver} compact />)}
          </span>
        )}
        <span className="marketplace-row-meta">
          <TrustBadge trust={plugin.trust} compact />
          <span>{plugin.kind === "dynamic" ? "动态插件" : "静态源码"}</span>
          <span>{plugin.license}</span>
          {(plugin.installed?.version || plugin.latest_compatible) && <span>v{plugin.installed?.version || plugin.latest_compatible}</span>}
        </span>
      </span>
      <span className="marketplace-row-state">
        <Badge variant={state.variant}>{state.label}</Badge>
        <ChevronRight />
      </span>
    </button>
  )
}

function PluginDetail({
  plugin,
  selectedVersion,
  busy,
  onVersionChange,
  onInstall,
  onAdopt,
  onPin,
  onRollback,
  onUninstall,
}: {
  plugin: MarketplacePluginView
  selectedVersion: MarketplaceVersionView | null
  busy: boolean
  onVersionChange: (version: string) => void
  onInstall: (version: string) => void
  onAdopt: () => void
  onPin: () => void
  onRollback: () => void
  onUninstall: () => void
}) {
  const installed = plugin.installed
  const versionRelation = selectedVersion && installed ? compareSemver(selectedVersion.version, installed.version) : 1
  const canInstall = plugin.kind === "dynamic" && selectedVersion?.installable && !plugin.unmanaged
  const primaryLabel = installed ? "更新到此版本" : "安装此版本"

  return (
    <article className="marketplace-detail">
      <div className="marketplace-detail-head">
        <div className="marketplace-detail-title">
          <span className={cn("marketplace-detail-icon", plugin.kind === "static" && "is-static")}>
            {plugin.kind === "dynamic" ? <PackageOpen /> : <FileCode2 />}
          </span>
          <div>
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              <h2>{plugin.name}</h2>
              <TrustBadge trust={plugin.trust} />
            </div>
            <code>{plugin.id}</code>
          </div>
        </div>
        <p>{plugin.description || plugin.summary}</p>
        <div className="marketplace-detail-links">
          {plugin.repository_url && (
            <Button variant="outline" size="sm" asChild>
              <a href={plugin.repository_url} target="_blank" rel="noreferrer">
                <ExternalLink />源码仓库
              </a>
            </Button>
          )}
          <Badge variant="neutral">{plugin.license}</Badge>
          <Badge variant="neutral">仓库 ID {plugin.repository_id}</Badge>
        </div>
      </div>

      {installed && (
        <section className="marketplace-installed-band">
          <div>
            <PackageCheck />
            <span>
              <small>{installed.loaded ? "当前已加载" : installed.active ? "文件已激活" : "活动文件缺失"}</small>
              <strong>v{installed.version}</strong>
            </span>
          </div>
          <div className="marketplace-installed-actions">
            <Button variant="ghost" size="icon-sm" onClick={onPin} disabled={busy} title={installed.pinned ? "取消固定版本" : "固定当前版本"}>
              {installed.pinned ? <PinOff /> : <Pin />}
            </Button>
            <Button variant="ghost" size="icon-sm" onClick={onRollback} disabled={busy || !installed.can_rollback} title="回滚上一个审核版本">
              <RotateCcw />
            </Button>
            <Button variant="ghost" size="icon-sm" onClick={onUninstall} disabled={busy} title="卸载活动二进制">
              <Trash2 />
            </Button>
          </div>
        </section>
      )}

      {plugin.unmanaged && (
        <section className={cn("marketplace-adopt-band", !plugin.unmanaged.can_adopt && "is-blocked")}>
          <Fingerprint />
          <div>
            <strong>发现未关联的本地插件 v{plugin.unmanaged.version}</strong>
            <span>{plugin.unmanaged.reason}</span>
          </div>
          <Button size="sm" variant="outline" onClick={onAdopt} disabled={busy || !plugin.unmanaged.can_adopt}>关联</Button>
        </section>
      )}

      <section className="marketplace-version-section">
        <div className="marketplace-section-title">
          <div><History /><span><strong>版本</strong><small>{plugin.versions.length} 个已登记版本</small></span></div>
          {plugin.versions.length > 0 && (
            <Select
              className="marketplace-version-select"
              value={selectedVersion?.version ?? ""}
              onChange={(event) => onVersionChange(event.target.value)}
              aria-label="选择插件版本"
            >
              {plugin.versions.map((version) => (
                <option key={version.version} value={version.version}>
                  {version.version} · {version.installable ? "兼容" : version.yanked ? "已撤回" : "不兼容"}
                </option>
              ))}
            </Select>
          )}
        </div>

        {selectedVersion ? (
          <VersionDetails version={selectedVersion} />
        ) : (
          <div className="marketplace-no-version">当前目录没有可展示的版本信息。</div>
        )}
      </section>

      <footer className="marketplace-detail-footer">
        <div>
          <strong>{actionTitle(plugin, selectedVersion, versionRelation)}</strong>
          <span>{actionHint(plugin, selectedVersion, versionRelation)}</span>
        </div>
        {plugin.kind === "static" ? (
          plugin.repository_url && (
            <Button asChild>
              <a href={plugin.repository_url} target="_blank" rel="noreferrer"><FileCode2 />查看构建说明</a>
            </Button>
          )
        ) : plugin.unmanaged ? (
          <Button onClick={onAdopt} disabled={busy || !plugin.unmanaged.can_adopt}><Fingerprint />关联本地插件</Button>
        ) : (
          <Button
            onClick={() => selectedVersion && onInstall(selectedVersion.version)}
            disabled={busy || !canInstall || (Boolean(installed) && (versionRelation <= 0 || Boolean(installed?.pinned)))}
          >
            {busy ? <LoaderCircle className="animate-spin-slow" /> : installed ? <Sparkles /> : <Download />}
            {busy ? "处理中" : primaryLabel}
          </Button>
        )}
      </footer>
    </article>
  )
}

function VersionDetails({ version }: { version: MarketplaceVersionView }) {
  return (
    <div className="marketplace-version-details">
      <div className="marketplace-version-status">
        <Badge variant={version.installable ? "success" : version.yanked ? "danger" : "warning"}>
          {version.installable ? <Check /> : <AlertTriangle />}
          {version.installable ? "当前宿主兼容" : version.yanked ? "版本已撤回" : "当前宿主不兼容"}
        </Badge>
        <span>{formatDate(version.released_at)}</span>
      </div>
      <div className="marketplace-version-facts">
        <VersionFact label="QimenBot" value={version.qimenbot} />
        <VersionFact label="动态 API" value={version.dynamic_api || "不适用"} />
        <VersionFact label="目标平台" value={version.asset_target || "源码构建"} />
        <VersionFact label="最低 glibc" value={version.min_glibc || "不适用"} />
        <VersionFact label="资产大小" value={version.asset_size_bytes ? formatBytes(version.asset_size_bytes) : "未提供"} />
        <VersionFact label="数据版本" value={String(version.data_schema_version)} />
      </div>
      {version.drivers.length > 0 && <DriverMatrix drivers={version.drivers} />}
      {version.asset_sha256 && (
        <div className="marketplace-checksum">
          <Fingerprint />
          <span className="marketplace-checksum-value"><small>SHA256</small><code title={version.asset_sha256}>{version.asset_sha256}</code></span>
          {version.github_attestation && <Badge variant="success" className="marketplace-attestation-badge"><BadgeCheck />构建证明</Badge>}
        </div>
      )}
      {version.issues.length > 0 && (
        <div className="marketplace-version-issues">
          {version.issues.map((issue) => <span key={issue}><AlertTriangle />{issue}</span>)}
        </div>
      )}
      {version.changelog && <p className="marketplace-changelog">{version.changelog}</p>}
    </div>
  )
}

function VersionFact({ label, value }: { label: string; value: string }) {
  return <span><small>{label}</small><strong title={value}>{value}</strong></span>
}

const driverLabels = {
  onebot11: { label: "OneBot 11", detail: "普通消息驱动" },
  "qq-official": { label: "官方 QQ Bot", detail: "开放平台驱动" },
} as const

const sceneLabels = {
  private: "私聊",
  group: "群聊",
  "group-at": "群内 @",
  channel: "频道消息",
  "channel-at": "频道 @",
  "channel-private": "频道私信",
} as const

const eventLabels = {
  message: "消息",
  notice: "通知",
  request: "请求",
  meta: "元事件",
} as const

const outboundLabels = {
  reply: "回复",
  proactive: "主动发送",
  "rich-message": "富媒体",
} as const

function DriverMatrix({ drivers }: { drivers: MarketplaceDriverSupport[] }) {
  return (
    <section className="marketplace-driver-matrix" aria-label="消息驱动兼容性">
      <div className="marketplace-driver-matrix-head">
        <Cable />
        <span><small>驱动兼容</small><strong>{drivers.length} 个已验证驱动</strong></span>
      </div>
      {drivers.map((support) => (
        <div className="marketplace-driver-row" key={support.driver}>
          <div className="marketplace-driver-title">
            <DriverBadge driver={support.driver} />
            <span>{driverLabels[support.driver].detail}</span>
          </div>
          <CapabilityLine icon={MessageSquare} label="场景" values={support.scenes.map((scene) => sceneLabels[scene])} />
          <CapabilityLine icon={Bell} label="事件" values={support.events.map((event) => eventLabels[event])} />
          <CapabilityLine icon={Send} label="发送" values={support.outbound.map((capability) => outboundLabels[capability])} />
        </div>
      ))}
    </section>
  )
}

function DriverBadge({ driver, compact = false }: { driver: MarketplaceDriverSupport["driver"]; compact?: boolean }) {
  return (
    <Badge
      variant={driver === "qq-official" ? "default" : "neutral"}
      className={cn("marketplace-driver-badge", driver === "qq-official" && "is-qq", compact && "is-compact")}
    >
      {driver === "qq-official" ? <MessageSquare /> : <Cable />}
      {driverLabels[driver].label}
    </Badge>
  )
}

function CapabilityLine({
  icon: Icon,
  label,
  values,
}: {
  icon: typeof MessageSquare
  label: string
  values: string[]
}) {
  return (
    <div className="marketplace-capability-line">
      <span className="marketplace-capability-label"><Icon />{label}</span>
      <span className="marketplace-capability-values">
        {values.length ? values.map((value) => <span key={value}>{value}</span>) : <span className="is-empty">未声明</span>}
      </span>
    </div>
  )
}

function TrustBadge({ trust, compact = false }: { trust: MarketplacePluginSummaryView["trust"]; compact?: boolean }) {
  const content =
    trust === "official"
      ? { label: "官方", icon: ShieldCheck, variant: "success" as const }
      : trust === "verified-build"
        ? { label: "构建已验证", icon: BadgeCheck, variant: "default" as const }
        : { label: "社区", icon: Users, variant: "neutral" as const }
  const Icon = content.icon
  return <Badge variant={content.variant}>{!compact && <Icon />}{content.label}</Badge>
}

function MarketplaceSkeleton() {
  return (
    <div className="marketplace-skeleton" aria-label="正在读取插件目录">
      {[0, 1, 2, 3].map((item) => (
        <div key={item}><span /><p><i /><i /><i /></p></div>
      ))}
    </div>
  )
}

function MarketplacePagination({
  pagination,
  pageSize,
  disabled,
  onPageChange,
  onPageSizeChange,
}: {
  pagination: MarketplaceView["pagination"]
  pageSize: number
  disabled: boolean
  onPageChange: (page: number) => void
  onPageSizeChange: (pageSize: number) => void
}) {
  const start = (pagination.page - 1) * pagination.page_size + 1
  const end = Math.min(pagination.page * pagination.page_size, pagination.total_items)
  return (
    <div className="marketplace-pagination">
      <span className="marketplace-pagination-range">
        <strong>{start}-{end}</strong>
        <small>共 {pagination.total_items} 个</small>
      </span>
      <div className="marketplace-pagination-controls" aria-label="商城分页">
        <Select
          value={String(pageSize)}
          onChange={(event) => onPageSizeChange(Number(event.target.value))}
          disabled={disabled}
          aria-label="每页插件数量"
        >
          {pageSizeOptions.map((size) => <option key={size} value={size}>每页 {size}</option>)}
        </Select>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onPageChange(pagination.page - 1)}
          disabled={disabled || pagination.page <= 1}
          title="上一页"
        >
          <ChevronLeft />
        </Button>
        <span className="marketplace-pagination-page">
          <strong>{pagination.page}</strong>
          <small>/ {pagination.total_pages}</small>
        </span>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onPageChange(pagination.page + 1)}
          disabled={disabled || pagination.page >= pagination.total_pages}
          title="下一页"
        >
          <ChevronRight />
        </Button>
      </div>
    </div>
  )
}

function ConfirmAction({
  pending,
  busy,
  onClose,
  onConfirm,
}: {
  pending: PendingAction | null
  busy: boolean
  onClose: () => void
  onConfirm: (action: PendingAction) => Promise<void>
}) {
  if (!pending) return null
  const copy = confirmationCopy(pending)
  return (
    <AlertDialog.Root open onOpenChange={(open) => !open && onClose()}>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="marketplace-dialog-overlay" />
        <AlertDialog.Content className="marketplace-dialog">
          <span className={cn("marketplace-dialog-icon", pending.kind === "uninstall" && "is-danger")}>
            {pending.kind === "uninstall" ? <Trash2 /> : pending.kind === "rollback" ? <RotateCcw /> : <ShieldAlert />}
          </span>
          <AlertDialog.Title>{copy.title}</AlertDialog.Title>
          <AlertDialog.Description>{copy.description}</AlertDialog.Description>
          <div className="marketplace-dialog-facts">
            <span><small>插件</small><strong>{pending.plugin.name}</strong></span>
            <span><small>版本</small><strong>{pending.version || pending.plugin.installed?.version || "当前"}</strong></span>
            <span><small>来源</small><strong>{pending.plugin.repository || "目录已下架"}</strong></span>
          </div>
          <div className="marketplace-dialog-actions">
            <AlertDialog.Cancel asChild><Button variant="outline" disabled={busy}>取消</Button></AlertDialog.Cancel>
            <Button
              variant={pending.kind === "uninstall" ? "destructive" : "default"}
              disabled={busy}
              onClick={() => {
                const action = pending
                onClose()
                void onConfirm(action)
              }}
            >
              {copy.action}
            </Button>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  )
}

function resolveSelectedVersion(plugin: MarketplacePluginView, selected?: string) {
  const version = selected
    ? plugin.versions.find((item) => item.version === selected)
    : plugin.versions.find((item) => item.version === plugin.latest_compatible)
      ?? plugin.versions.find((item) => item.version === plugin.installed?.version)
      ?? plugin.versions[0]
  return version ?? null
}

function pluginState(plugin: MarketplacePluginSummaryView): { label: string; variant: "success" | "warning" | "danger" | "neutral" | "default" } {
  if (!plugin.catalog_listed) return { label: "目录已下架", variant: "danger" }
  if (plugin.unmanaged) return { label: plugin.unmanaged.can_adopt ? "待关联" : "校验不符", variant: "warning" }
  if (plugin.installed?.update_available) return { label: "可更新", variant: "default" }
  if (plugin.installed) return { label: plugin.installed.loaded ? "已安装" : "需检查", variant: plugin.installed.loaded ? "success" : "warning" }
  if (plugin.kind === "static") return { label: "源码构建", variant: "neutral" }
  if (!plugin.latest_compatible) return { label: "不兼容", variant: "warning" }
  return { label: "可安装", variant: "neutral" }
}

function actionTitle(plugin: MarketplacePluginView, version: MarketplaceVersionView | null, relation: number) {
  if (!plugin.catalog_listed) return "目录已不再收录"
  if (plugin.kind === "static") return "需要重新构建宿主"
  if (plugin.unmanaged) return plugin.unmanaged.can_adopt ? "可关联现有文件" : "本地文件无法关联"
  if (!version?.installable) return "所选版本不能安装"
  if (plugin.installed?.pinned) return "当前版本已固定"
  if (plugin.installed && relation === 0) return "当前版本已经安装"
  if (plugin.installed && relation < 0) return "不能直接降级"
  return plugin.installed ? "已通过兼容性检查" : "准备安装"
}

function actionHint(plugin: MarketplacePluginView, version: MarketplaceVersionView | null, relation: number) {
  if (!plugin.catalog_listed) return "仍可卸载本地二进制，配置和数据会保留。"
  if (plugin.kind === "static") return "商城只展示源码与兼容范围，不会修改 qimenbotd。"
  if (plugin.unmanaged) return plugin.unmanaged.reason
  if (!version?.installable) return version?.issues[0] || "选择另一个兼容版本。"
  if (plugin.installed?.pinned) return "取消固定后才会显示并安装新版本。"
  if (plugin.installed && relation === 0) return "SHA256 和活动文件由本地安装锁管理。"
  if (plugin.installed && relation < 0) return "需要恢复历史版本时使用回滚按钮。"
  return "下载前会重新核对仓库数字 ID、资产大小和 SHA256。"
}

function confirmationCopy(action: PendingAction) {
  if (action.kind === "uninstall") {
    return {
      title: "卸载活动插件",
      description: "宿主会先安全卸载动态库，再移除活动二进制。插件配置、数据和审核缓存不会删除。",
      action: "确认卸载",
    }
  }
  if (action.kind === "rollback") {
    return {
      title: "回滚上一个版本",
      description: "仅当当前版本声明可以安全回滚时执行。宿主会在加载失败时恢复现状。",
      action: "确认回滚",
    }
  }
  if (action.kind === "adopt") {
    return {
      title: "关联本地插件",
      description: "只会关联与商城审核资产大小和 SHA256 完全一致的文件，并默认固定当前版本。",
      action: "确认关联",
    }
  }
  return {
    title: action.plugin.installed ? "安装插件更新" : "安装第三方插件",
    description: "安装后，插件代码会在 QimenBot 进程内运行。请确认你已经检查源码、权限和版本说明。",
    action: action.plugin.installed ? "确认更新" : "确认安装",
  }
}

function compareSemver(left: string, right: string) {
  const parse = (value: string) => {
    const withoutBuild = value.split("+", 1)[0]
    const separator = withoutBuild.indexOf("-")
    const core = separator === -1 ? withoutBuild : withoutBuild.slice(0, separator)
    const prerelease = separator === -1 ? [] : withoutBuild.slice(separator + 1).split(".")
    return { numbers: core.split("."), prerelease }
  }
  const compareNumeric = (leftValue: string, rightValue: string) => {
    const leftNumber = BigInt(leftValue)
    const rightNumber = BigInt(rightValue)
    return leftNumber === rightNumber ? 0 : leftNumber > rightNumber ? 1 : -1
  }
  const a = parse(left)
  const b = parse(right)
  for (let index = 0; index < 3; index += 1) {
    const difference = compareNumeric(a.numbers[index] ?? "0", b.numbers[index] ?? "0")
    if (difference) return difference
  }
  if (!a.prerelease.length && b.prerelease.length) return 1
  if (a.prerelease.length && !b.prerelease.length) return -1
  for (let index = 0; index < Math.max(a.prerelease.length, b.prerelease.length); index += 1) {
    const leftIdentifier = a.prerelease[index]
    const rightIdentifier = b.prerelease[index]
    if (leftIdentifier === undefined) return -1
    if (rightIdentifier === undefined) return 1
    if (leftIdentifier === rightIdentifier) continue
    const leftNumeric = /^\d+$/.test(leftIdentifier)
    const rightNumeric = /^\d+$/.test(rightIdentifier)
    if (leftNumeric && rightNumeric) return compareNumeric(leftIdentifier, rightIdentifier)
    if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1
    return leftIdentifier < rightIdentifier ? -1 : 1
  }
  return 0
}

function formatDate(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return new Intl.DateTimeFormat("zh-CN", { year: "numeric", month: "short", day: "numeric" }).format(date)
}
