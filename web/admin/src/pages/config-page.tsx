import { useCallback, useEffect, useId, useRef, useState } from "react"
import type React from "react"
import * as TabsPrimitive from "@radix-ui/react-tabs"
import {
  Gauge,
  History,
  PanelTop,
  Plus,
  Puzzle,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
  Webhook,
  X,
} from "lucide-react"
import { toast } from "sonner"

import { api, type ConfigView, type GeneralConfigView, type RevisionView } from "@/lib/api"
import { formatBytes, formatClock, generalToMutation } from "@/lib/format"
import { cn } from "@/lib/utils"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

type ConfigTab = "runtime" | "panel" | "plugins" | "webhook" | "history"

const tabs = [
  { id: "runtime", label: "运行时", icon: Gauge },
  { id: "panel", label: "面板安全", icon: PanelTop },
  { id: "plugins", label: "插件", icon: Puzzle },
  { id: "webhook", label: "Webhook", icon: Webhook },
  { id: "history", label: "配置版本", icon: History },
] satisfies Array<{ id: ConfigTab; label: string; icon: typeof Gauge }>

export function ConfigPage({ onRefreshSnapshot }: { onRefreshSnapshot: () => void }) {
  const [config, setConfig] = useState<ConfigView | null>(null)
  const [general, setGeneral] = useState<GeneralConfigView | null>(null)
  const [revisions, setRevisions] = useState<RevisionView[]>([])
  const [adminToken, setAdminToken] = useState("")
  const [webhookToken, setWebhookToken] = useState("")
  const [activeTab, setActiveTab] = useState<ConfigTab>("runtime")
  const [dirty, setDirty] = useState(false)
  const [busy, setBusy] = useState(false)
  const tabsRef = useRef<HTMLDivElement>(null)

  const load = useCallback(async () => {
    const [nextConfig, nextRevisions] = await Promise.all([api.config(), api.revisions()])
    setConfig(nextConfig)
    setGeneral(structuredClone(nextConfig.general))
    setRevisions(nextRevisions)
    setAdminToken("")
    setWebhookToken("")
    setDirty(false)
  }, [])

  useEffect(() => {
    void load().catch((error) => toast.error(error.message))
  }, [load])

  useEffect(() => {
    const list = tabsRef.current
    const activeTrigger = list?.querySelector<HTMLElement>('[data-state="active"]')
    if (!list || !activeTrigger) return

    const triggerLeft = activeTrigger.offsetLeft
    const triggerRight = triggerLeft + activeTrigger.offsetWidth
    const viewLeft = list.scrollLeft
    const viewRight = viewLeft + list.clientWidth
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches
    const behavior = reduceMotion ? "auto" : "smooth"

    if (triggerLeft < viewLeft) {
      list.scrollTo({ left: Math.max(0, triggerLeft - 8), behavior })
    } else if (triggerRight > viewRight) {
      list.scrollTo({ left: triggerRight - list.clientWidth + 8, behavior })
    }
  }, [activeTab])

  const patch = (next: Partial<GeneralConfigView>) => {
    if (!general) return
    setGeneral({ ...general, ...next })
    setDirty(true)
  }

  const updateAdminToken = (value: string) => {
    setAdminToken(value)
    setDirty(true)
  }

  const updateWebhookToken = (value: string) => {
    setWebhookToken(value)
    setDirty(true)
  }

  const save = async () => {
    if (!config || !general) return
    setBusy(true)
    try {
      const result = await api.updateGeneral(
        config.revision,
        generalToMutation(general, {
          admin_access_token: adminToken || undefined,
          webhook_access_token: webhookToken || undefined,
        }),
      )
      toast.success(result.message)
      setAdminToken("")
      setWebhookToken("")
      await load()
      onRefreshSnapshot()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "配置保存失败")
    } finally {
      setBusy(false)
    }
  }

  const rollback = async (revision: string) => {
    setBusy(true)
    try {
      const result = await api.rollback(revision)
      toast.success(result.message)
      await load()
      onRefreshSnapshot()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "回滚失败")
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex flex-wrap items-center gap-2.5">
            <h1>配置</h1>
            <span className="environment-tag">{config?.revision.slice(0, 8) ?? "REV"}</span>
            <Badge variant="success"><ShieldCheck />校验后写入</Badge>
            {config?.restart_required && <Badge variant="warning">需要重启</Badge>}
          </div>
          <p>按功能分区维护宿主配置，密钥字段只写不读。</p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void load()}
            disabled={busy || dirty}
            title={dirty ? "请先保存或撤销未保存修改" : undefined}
          >
            <RefreshCw />
            刷新
          </Button>
          <Button size="sm" onClick={save} disabled={busy || !general || !dirty}>
            <Save />
            {busy ? "保存中" : dirty ? "保存更改" : "已保存"}
          </Button>
        </div>
      </div>

      {general ? (
        <>
          <section className="config-summary enter-panel" aria-label="配置摘要">
            <SummaryItem
              icon={Gauge}
              label="运行环境"
              value={general.environment}
              tone="default"
              active={activeTab === "runtime"}
              onClick={() => setActiveTab("runtime")}
            />
            <SummaryItem
              icon={PanelTop}
              label="管理面板"
              value={general.admin_enabled ? "已启用" : "已关闭"}
              tone={general.admin_enabled ? "success" : "neutral"}
              active={activeTab === "panel"}
              onClick={() => setActiveTab("panel")}
            />
            <SummaryItem
              icon={Puzzle}
              label="加载模块"
              value={String(general.builtin_modules.length + general.plugin_modules.length)}
              tone="default"
              active={activeTab === "plugins"}
              onClick={() => setActiveTab("plugins")}
            />
            <SummaryItem
              icon={Webhook}
              label="Webhook"
              value={general.webhook_enabled ? "已启用" : "已关闭"}
              tone={general.webhook_enabled ? "success" : "neutral"}
              active={activeTab === "webhook"}
              onClick={() => setActiveTab("webhook")}
            />
          </section>

          <TabsPrimitive.Root
            className="panel config-workbench enter-panel"
            value={activeTab}
            onValueChange={(value) => setActiveTab(value as ConfigTab)}
          >
            <TabsPrimitive.List ref={tabsRef} className="config-tabs" aria-label="配置分区">
              {tabs.map((tab) => {
                const Icon = tab.icon
                return (
                  <TabsPrimitive.Trigger
                    key={tab.id}
                    value={tab.id}
                    className="config-tab"
                  >
                    <Icon />
                    <span>{tab.label}</span>
                    {tab.id === "history" && <Badge variant="neutral">{revisions.length}</Badge>}
                  </TabsPrimitive.Trigger>
                )
              })}
            </TabsPrimitive.List>

            <div className="config-pane">
              <TabsPrimitive.Content value="runtime" className="config-tabpanel">
                <RuntimeSection general={general} patch={patch} />
              </TabsPrimitive.Content>
              <TabsPrimitive.Content value="panel" className="config-tabpanel">
                <PanelSection
                  general={general}
                  patch={patch}
                  token={adminToken}
                  setToken={updateAdminToken}
                />
              </TabsPrimitive.Content>
              <TabsPrimitive.Content value="plugins" className="config-tabpanel">
                <PluginsSection general={general} patch={patch} />
              </TabsPrimitive.Content>
              <TabsPrimitive.Content value="webhook" className="config-tabpanel">
                <WebhookSection
                  general={general}
                  patch={patch}
                  token={webhookToken}
                  setToken={updateWebhookToken}
                />
              </TabsPrimitive.Content>
              <TabsPrimitive.Content value="history" className="config-tabpanel">
                <HistorySection revisions={revisions} busy={busy} onRollback={rollback} />
              </TabsPrimitive.Content>
            </div>

            <div className="config-actionbar">
              <div className="flex min-w-0 items-center gap-2">
                <span className={"save-dot " + (dirty ? "is-dirty" : "")} />
                <div className="min-w-0">
                  <strong>{dirty ? "有未保存更改" : "配置与磁盘一致"}</strong>
                  <small>{dirty ? "保存时会先校验并备份当前版本" : "修改任意字段后可在这里统一保存"}</small>
                </div>
              </div>
              <div className="config-actionbar-actions">
                {dirty && (
                  <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy}>
                    <RotateCcw />
                    撤销
                  </Button>
                )}
                <Button size="sm" onClick={save} disabled={busy || !dirty}>
                  <Save />
                  {busy ? "保存中" : "保存"}
                </Button>
              </div>
            </div>
          </TabsPrimitive.Root>
        </>
      ) : (
        <section className="panel empty-panel">正在读取配置。</section>
      )}
    </main>
  )
}

function RuntimeSection({
  general,
  patch,
}: {
  general: GeneralConfigView
  patch: (next: Partial<GeneralConfigView>) => void
}) {
  return (
    <ConfigSection
      title="运行时与日志"
      description="控制宿主生命周期、终端日志格式和内存日志容量。"
      badge={<Badge variant="default">{general.environment}</Badge>}
    >
      <div className="config-form-grid">
        <Field label="运行环境" hint="仅作为环境标识，不会自动合并 dev.toml 或 prod.toml。">
          <Select value={general.environment} onChange={(event) => patch({ environment: event.target.value })}>
            <option value="dev">dev</option>
            <option value="prod">prod</option>
            <option value="test">test</option>
          </Select>
        </Field>
        <Field label="日志级别" hint="日常运行建议 info，排查问题时临时使用 debug。">
          <Select value={general.log_level} onChange={(event) => patch({ log_level: event.target.value })}>
            <option value="trace">trace</option>
            <option value="debug">debug</option>
            <option value="info">info</option>
            <option value="warn">warn</option>
            <option value="error">error</option>
          </Select>
        </Field>
        <NumberField
          label="关闭超时"
          hint="收到关闭信号后等待任务退出的最长时间。"
          value={general.shutdown_timeout_secs}
          unit="秒"
          onChange={(shutdown_timeout_secs) => patch({ shutdown_timeout_secs })}
        />
        <NumberField
          label="任务宽限"
          hint="后台任务的优雅退出等待时间。"
          value={general.task_grace_secs}
          unit="秒"
          onChange={(task_grace_secs) => patch({ task_grace_secs })}
        />
        <NumberField
          label="日志缓冲容量"
          hint="面板可查询的最新结构化日志条数。"
          value={general.log_capacity}
          unit="条"
          onChange={(log_capacity) => patch({ log_capacity })}
        />
        <ToggleSetting
          label="JSON 终端日志"
          description="适合 ELK、Loki 等采集系统；面板仍使用结构化日志。"
          checked={general.json_logs}
          onChange={(json_logs) => patch({ json_logs })}
        />
      </div>
    </ConfigSection>
  )
}

function PanelSection({
  general,
  patch,
  token,
  setToken,
}: {
  general: GeneralConfigView
  patch: (next: Partial<GeneralConfigView>) => void
  token: string
  setToken: (token: string) => void
}) {
  return (
    <ConfigSection
      title="面板与安全"
      description="面板默认仅监听本机；监听非回环地址时必须设置管理 Token。"
      badge={<Badge variant={general.admin_enabled ? "success" : "neutral"}>{general.admin_enabled ? "已启用" : "已关闭"}</Badge>}
    >
      <div className="config-form-grid">
        <ToggleSetting
          label="启用 Web 管理面板"
          description="关闭后下次启动不再监听管理端口。"
          checked={general.admin_enabled}
          onChange={(admin_enabled) => patch({ admin_enabled })}
        />
        <Field label="监听地址" hint="本机管理建议保持 127.0.0.1。">
          <Input value={general.admin_bind} onChange={(event) => patch({ admin_bind: event.target.value })} />
        </Field>
        <SecretField
          label="管理 Token"
          configured={general.admin_token_configured}
          value={token}
          onChange={setToken}
          placeholder="QIMEN_ADMIN_TOKEN"
        />
        <Field label="审计日志" hint="Bot、插件和配置管理操作会追加到该文件。">
          <Input value={general.audit_path} onChange={(event) => patch({ audit_path: event.target.value })} />
        </Field>
      </div>
      <div className="security-note">
        <ShieldCheck />
        <div>
          <strong>密钥只写</strong>
          <span>Token 原值不会返回浏览器。输入框留空时保留当前值。</span>
        </div>
      </div>
    </ConfigSection>
  )
}

function PluginsSection({
  general,
  patch,
}: {
  general: GeneralConfigView
  patch: (next: Partial<GeneralConfigView>) => void
}) {
  return (
    <ConfigSection
      title="模块与插件"
      description="徽标代表配置中的模块 ID。点击徽标内的删除图标可移除，使用输入框添加。"
      badge={<Badge variant="default">{general.builtin_modules.length + general.plugin_modules.length} 个模块</Badge>}
    >
      <div className="config-form-grid">
        <Field label="内置模块" hint="宿主随框架编译的基础模块。" wide controlGroup>
          <TagEditor
            values={general.builtin_modules}
            tone="neutral"
            placeholder="输入模块 ID 后回车"
            ariaLabel="添加内置模块"
            onChange={(builtin_modules) => patch({ builtin_modules })}
          />
        </Field>
        <Field label="静态插件" hint="已编译进 qimenbotd 的插件模块。" wide controlGroup>
          <TagEditor
            values={general.plugin_modules}
            tone="default"
            placeholder="输入插件 module id 后回车"
            ariaLabel="添加静态插件"
            onChange={(plugin_modules) => patch({ plugin_modules })}
          />
        </Field>
        <Field label="插件状态文件" hint="插件启用和禁用状态的持久化位置。">
          <Input value={general.plugin_state_path} onChange={(event) => patch({ plugin_state_path: event.target.value })} />
        </Field>
        <Field label="动态插件目录" hint="扫描 DLL、SO 和 dylib 的目录。">
          <Input value={general.plugin_bin_dir} onChange={(event) => patch({ plugin_bin_dir: event.target.value })} />
        </Field>
        <NumberField
          label="动态插件超时"
          hint="单次 FFI 回调的执行上限。"
          value={general.dynamic_plugin_timeout_secs}
          unit="秒"
          onChange={(dynamic_plugin_timeout_secs) => patch({ dynamic_plugin_timeout_secs })}
        />
        <NumberField
          label="主动发送队列"
          hint="每个启用 Bot 的待发送队列容量。"
          value={general.proactive_queue_capacity}
          unit="条"
          onChange={(proactive_queue_capacity) => patch({ proactive_queue_capacity })}
        />
        <NumberField
          label="离线等待"
          hint="Bot 离线时主动发送请求的最长等待时间。"
          value={general.proactive_offline_ttl_secs}
          unit="秒"
          onChange={(proactive_offline_ttl_secs) => patch({ proactive_offline_ttl_secs })}
        />
      </div>
    </ConfigSection>
  )
}

function WebhookSection({
  general,
  patch,
  token,
  setToken,
}: {
  general: GeneralConfigView
  patch: (next: Partial<GeneralConfigView>) => void
  token: string
  setToken: (token: string) => void
}) {
  return (
    <ConfigSection
      title="动态插件 Webhook"
      description="为 API 0.5 动态插件提供宿主统一管理的 HTTP 入口。"
      badge={<Badge variant={general.webhook_enabled ? "success" : "neutral"}>{general.webhook_enabled ? "已启用" : "已关闭"}</Badge>}
    >
      <div className="config-form-grid">
        <ToggleSetting
          label="启用 Webhook Gateway"
          description="下次启动时安装动态插件声明的 HTTP 路由。"
          checked={general.webhook_enabled}
          onChange={(webhook_enabled) => patch({ webhook_enabled })}
        />
        <Field label="监听地址" hint="公网使用前应配置反向代理和 Token。">
          <Input value={general.webhook_bind} onChange={(event) => patch({ webhook_bind: event.target.value })} />
        </Field>
        <Field label="基础路径" hint="插件路由将挂载在该路径之下。">
          <Input value={general.webhook_base_path} onChange={(event) => patch({ webhook_base_path: event.target.value })} />
        </Field>
        <SecretField
          label="Bearer Token"
          configured={general.webhook_token_configured}
          value={token}
          onChange={setToken}
          placeholder="QIMEN_WEBHOOK_TOKEN"
        />
        <NumberField
          label="请求体上限"
          hint="单个请求体允许的最大字节数。"
          value={general.webhook_max_body_bytes}
          unit="bytes"
          onChange={(webhook_max_body_bytes) => patch({ webhook_max_body_bytes })}
        />
        <NumberField
          label="请求超时"
          hint="宿主等待插件回调返回的时间。"
          value={general.webhook_request_timeout_ms}
          unit="ms"
          onChange={(webhook_request_timeout_ms) => patch({ webhook_request_timeout_ms })}
        />
        <NumberField
          label="并发上限"
          hint="同时处理的 Webhook 请求数量。"
          value={general.webhook_max_in_flight}
          unit="个"
          onChange={(webhook_max_in_flight) => patch({ webhook_max_in_flight })}
        />
      </div>
    </ConfigSection>
  )
}

function HistorySection({
  revisions,
  busy,
  onRollback,
}: {
  revisions: RevisionView[]
  busy: boolean
  onRollback: (revision: string) => void
}) {
  return (
    <ConfigSection
      title="配置版本"
      description="每次保存前自动备份当前文件，最多保留最近 20 个版本。"
      badge={<Badge variant="neutral">{revisions.length} 个版本</Badge>}
    >
      <div className="revision-table">
        {revisions.length > 0 ? (
          revisions.map((revision) => (
            <div className="revision-row" key={revision.revision}>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <strong className="font-mono text-xs">{revision.revision.slice(0, 12)}</strong>
                  {revision.current && <Badge variant="success">当前版本</Badge>}
                </div>
                <p className="mt-1 text-[11px] text-muted-foreground">
                  {formatClock(revision.created_at)} · {formatBytes(revision.size_bytes)}
                </p>
              </div>
              <Button
                variant="outline"
                size="sm"
                onClick={() => onRollback(revision.revision)}
                disabled={revision.current || busy}
              >
                <RotateCcw />
                恢复
              </Button>
            </div>
          ))
        ) : (
          <div className="revision-empty">
            <History />
            <span>尚无可恢复的历史版本</span>
          </div>
        )}
      </div>
    </ConfigSection>
  )
}

function ConfigSection({
  title,
  description,
  badge,
  children,
}: {
  title: string
  description: string
  badge: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="config-section">
      <div className="config-section-heading">
        <div>
          <h2>{title}</h2>
          <p>{description}</p>
        </div>
        {badge}
      </div>
      {children}
    </div>
  )
}

function SummaryItem({
  icon: Icon,
  label,
  value,
  tone,
  active,
  onClick,
}: {
  icon: typeof Gauge
  label: string
  value: string
  tone: "default" | "success" | "neutral"
  active: boolean
  onClick: () => void
}) {
  return (
    <button type="button" className={cn("config-summary-item", active && "is-active")} onClick={onClick}>
      <span className="config-summary-icon"><Icon /></span>
      <span>
        <small>{label}</small>
        <Badge variant={tone}>{value}</Badge>
      </span>
    </button>
  )
}

function Field({
  label,
  hint,
  children,
  wide,
  controlGroup,
}: {
  label: string
  hint: string
  children: React.ReactNode
  wide?: boolean
  controlGroup?: boolean
}) {
  const content = (
    <>
      <span>
        <strong>{label}</strong>
        <small>{hint}</small>
      </span>
      {children}
    </>
  )

  return controlGroup ? (
    <div className={cn("config-field", wide && "is-wide")}>{content}</div>
  ) : (
    <label className={cn("config-field", wide && "is-wide")}>{content}</label>
  )
}

function NumberField({
  label,
  hint,
  value,
  unit,
  onChange,
}: {
  label: string
  hint: string
  value: number
  unit: string
  onChange: (value: number) => void
}) {
  return (
    <Field label={label} hint={hint}>
      <div className="number-input">
        <Input type="number" min={0} value={value} onChange={(event) => onChange(Number(event.target.value))} />
        <span>{unit}</span>
      </div>
    </Field>
  )
}

function ToggleSetting({
  label,
  description,
  checked,
  onChange,
}: {
  label: string
  description: string
  checked: boolean
  onChange: (checked: boolean) => void
}) {
  const id = useId()

  return (
    <div className="config-toggle">
      <label htmlFor={id}>
        <strong>{label}</strong>
        <small>{description}</small>
      </label>
      <Badge variant={checked ? "success" : "neutral"}>{checked ? "已启用" : "已关闭"}</Badge>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  )
}

function SecretField({
  label,
  configured,
  value,
  onChange,
  placeholder,
}: {
  label: string
  configured: boolean
  value: string
  onChange: (value: string) => void
  placeholder: string
}) {
  return (
    <Field label={label} hint="留空保留原值；保存后不会从 API 读回。">
      <div className="secret-input">
        <Input
          type="password"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder={configured ? "输入新值以替换" : placeholder}
        />
        <Badge variant={configured ? "success" : "neutral"}>{configured ? "已配置" : "未配置"}</Badge>
      </div>
    </Field>
  )
}

function TagEditor({
  values,
  tone,
  placeholder,
  ariaLabel,
  onChange,
}: {
  values: string[]
  tone: "default" | "neutral"
  placeholder: string
  ariaLabel: string
  onChange: (values: string[]) => void
}) {
  const [draft, setDraft] = useState("")

  const add = () => {
    const value = draft.trim()
    if (!value || values.includes(value)) return
    onChange([...values, value])
    setDraft("")
  }

  return (
    <div className="tag-editor">
      <div className="tag-list">
        {values.map((value) => (
          <Badge variant={tone} key={value}>
            {value}
            <button
              type="button"
              aria-label={"删除 " + value}
              onClick={() => onChange(values.filter((item) => item !== value))}
            >
              <X />
            </button>
          </Badge>
        ))}
      </div>
      <div className="tag-input-row">
        <Input
          value={draft}
          aria-label={ariaLabel}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === ",") {
              event.preventDefault()
              add()
            }
          }}
          placeholder={placeholder}
        />
        <Button type="button" variant="outline" size="icon" onClick={add} disabled={!draft.trim()} aria-label="添加徽标">
          <Plus />
        </Button>
      </div>
    </div>
  )
}
