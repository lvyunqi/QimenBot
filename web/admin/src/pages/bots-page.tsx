import { useEffect, useMemo, useState } from "react"
import type React from "react"
import { Check, Plus, RefreshCw, Save, Trash2, X } from "lucide-react"
import { toast } from "sonner"

import { api, type BotView, type ConfigView } from "@/lib/api"
import { botToMutation, defaultBot, formatNumber, listToText, relativeTime, textToList } from "@/lib/format"
import { botStatusLabel, botStatusVariant } from "@/lib/status"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

interface BotsPageProps {
  snapshotBots: BotView[]
  onRefreshSnapshot: () => void
}

interface IntentOption {
  id: string
  label: string
  description: string
  aliases?: string[]
  recommended?: boolean
}

const qqOfficialIntents: IntentOption[] = [
  {
    id: "GROUP_AND_C2C_EVENT",
    label: "QQ 群与单聊消息",
    description: "接收 QQ 群消息、群内 @ 机器人消息和 QQ 单聊消息。",
    aliases: ["PUBLIC_MESSAGES"],
    recommended: true,
  },
  {
    id: "PUBLIC_GUILD_MESSAGES",
    label: "频道公域消息",
    description: "接收频道公域中的消息与 @ 机器人消息。",
    recommended: true,
  },
  {
    id: "DIRECT_MESSAGE",
    label: "频道私信",
    description: "接收用户通过频道发送给机器人的私信。",
    recommended: true,
  },
  { id: "INTERACTION", label: "按钮与菜单互动", description: "接收按钮点击和快捷菜单等互动事件。" },
  { id: "MESSAGE_AUDIT", label: "消息审核结果", description: "接收主动消息的审核通过或驳回结果。" },
  { id: "GUILDS", label: "频道变更", description: "接收频道创建、更新和删除事件。" },
  { id: "GUILD_MEMBERS", label: "频道成员变更", description: "接收频道成员加入、资料更新和退出事件。" },
  { id: "GUILD_MESSAGES", label: "频道私域消息", description: "接收需要相应权限的频道私域消息。" },
  { id: "GUILD_MESSAGE_REACTIONS", label: "频道表情回应", description: "接收频道消息表情回应的增加和移除事件。" },
  {
    id: "FORUMS_EVENT",
    label: "论坛事件",
    description: "接收论坛主题、帖子和回复相关事件。",
    aliases: ["FORUMS"],
  },
  { id: "OPEN_FORUM_EVENT", label: "论坛公域事件", description: "接收开放论坛场景中的公域事件。" },
  {
    id: "AUDIO_OR_LIVE_CHANNEL_MEMBER",
    label: "音频与直播成员",
    description: "接收音频频道或直播频道的成员变更事件。",
  },
  { id: "AUDIO_ACTION", label: "音频操作", description: "接收音频频道中的通用操作事件。" },
]

const builtinModuleLabels: Record<string, string> = {
  command: "命令系统",
  admin: "管理能力",
  scheduler: "任务调度",
  bridge: "消息桥接",
}

export function BotsPage({ snapshotBots, onRefreshSnapshot }: BotsPageProps) {
  const [config, setConfig] = useState<ConfigView | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [draft, setDraft] = useState<BotView | null>(null)
  const [accessToken, setAccessToken] = useState("")
  const [secret, setSecret] = useState("")
  const [busy, setBusy] = useState(false)

  const load = async () => {
    setConfig(await api.config())
  }

  useEffect(() => {
    void load().catch((error) => toast.error(error.message))
  }, [])

  const bots = useMemo(() => {
    const status = new Map(snapshotBots.map((bot) => [bot.id, bot]))
    return (config?.bots ?? snapshotBots).map((bot) => ({ ...bot, ...(status.get(bot.id) ?? {}) }))
  }, [config?.bots, snapshotBots])

  const availableModules = useMemo(() => {
    if (!config) return []
    return Array.from(new Set([...config.general.builtin_modules, ...config.general.plugin_modules]))
  }, [config])

  const selectBot = (bot: BotView) => {
    setSelectedId(bot.id)
    setDraft(structuredClone(bot))
    setAccessToken("")
    setSecret("")
  }

  const createBot = () => {
    const bot = defaultBot()
    setSelectedId(null)
    setDraft(bot)
    setAccessToken("")
    setSecret("")
  }

  const save = async () => {
    if (!draft || !config) return
    setBusy(true)
    try {
      const result = await api.saveBot(
        config.revision,
        botToMutation(draft, {
          access_token: accessToken || undefined,
          secret: secret || undefined,
        }),
        selectedId ?? undefined,
      )
      toast.success(result.message)
      await load()
      onRefreshSnapshot()
      setSelectedId(draft.id)
      setAccessToken("")
      setSecret("")
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "保存失败")
    } finally {
      setBusy(false)
    }
  }

  const remove = async () => {
    if (!draft || !config || !selectedId) return
    setBusy(true)
    try {
      const result = await api.deleteBot(config.revision, selectedId)
      toast.success(result.message)
      setDraft(null)
      setSelectedId(null)
      await load()
      onRefreshSnapshot()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "删除失败")
    } finally {
      setBusy(false)
    }
  }

  const action = async (bot: BotView, command: "start" | "stop" | "reconnect") => {
    setBusy(true)
    try {
      const result = await api.botAction(bot.id, command)
      toast.success(result.message)
      onRefreshSnapshot()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "操作失败")
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>机器人</h1>
            <span className="environment-tag">{config?.revision.slice(0, 8) ?? "CONFIG"}</span>
            {config?.restart_required && <Badge variant="warning">配置待重启</Badge>}
          </div>
          <p>动态启停、重连和配置维护</p>
        </div>
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy}>
            <RefreshCw className={busy ? "animate-spin-slow" : ""} />
            刷新
          </Button>
          <Button size="sm" onClick={createBot}>
            <Plus />
            添加机器人
          </Button>
        </div>
      </div>

      <div className="split-grid">
        <section className="panel enter-panel">
          <div className="panel-header">
            <div>
              <h2 className="panel-title">实例列表</h2>
              <p className="panel-subtitle">运行状态来自 Runtime，配置来自 base.toml</p>
            </div>
          </div>
          <div className="stack-list">
            {bots.map((bot) => (
              <button
                type="button"
                className={"stack-row " + (draft?.id === bot.id ? "is-active" : "")}
                key={bot.id}
                onClick={() => selectBot(bot)}
              >
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className={"status-dot is-" + bot.state} />
                    <strong className="truncate font-mono text-xs">{bot.id}</strong>
                    <Badge variant={botStatusVariant(bot.state)}>{botStatusLabel(bot.state)}</Badge>
                  </div>
                  <div className="mt-1 truncate text-[11px] text-muted-foreground">
                    {bot.protocol} / {bot.transport} · {relativeTime(bot.last_event_epoch_ms)}
                  </div>
                </div>
                <div className="text-right font-mono text-[11px] text-muted-foreground">
                  {formatNumber(bot.events_received)}
                </div>
              </button>
            ))}
            {bots.length === 0 && <div className="empty-panel">还没有机器人配置</div>}
          </div>
        </section>

        <section className="panel enter-panel">
          <div className="panel-header">
            <div>
              <h2 className="panel-title">{draft ? "编辑实例" : "选择一个实例"}</h2>
              <p className="panel-subtitle">密钥留空表示保留现有值</p>
            </div>
            {draft && (
              <div className="flex gap-2">
                {selectedId && (
                  <Button variant="ghost" size="sm" onClick={remove} disabled={busy}>
                    <Trash2 />
                    删除
                  </Button>
                )}
                <Button size="sm" onClick={save} disabled={busy || !config}>
                  <Save />
                  保存
                </Button>
              </div>
            )}
          </div>
          {draft ? (
            <BotEditor
              bot={draft}
              setBot={setDraft}
              accessToken={accessToken}
              secret={secret}
              setAccessToken={setAccessToken}
              setSecret={setSecret}
              onAction={(command) => void action(draft, command)}
              actionDisabled={busy || !selectedId}
              availableModules={availableModules}
            />
          ) : (
            <div className="empty-panel">从左侧选择机器人，或新建一个配置。</div>
          )}
        </section>
      </div>
    </main>
  )
}

function BotEditor({
  bot,
  setBot,
  accessToken,
  secret,
  setAccessToken,
  setSecret,
  onAction,
  actionDisabled,
  availableModules,
}: {
  bot: BotView
  setBot: (bot: BotView) => void
  accessToken: string
  secret: string
  setAccessToken: (value: string) => void
  setSecret: (value: string) => void
  onAction: (action: "start" | "stop" | "reconnect") => void
  actionDisabled: boolean
  availableModules: string[]
}) {
  const patch = (next: Partial<BotView>) => setBot({ ...bot, ...next })

  return (
    <div className="editor-grid">
      <EditorSectionTitle title="连接参数" description="用于识别机器人实例并建立协议连接。" />
      <Field label="实例 ID"><Input value={bot.id} onChange={(event) => patch({ id: event.target.value })} /></Field>
      <Field label="账号标识"><Input value={bot.account_id ?? ""} onChange={(event) => patch({ account_id: event.target.value })} placeholder="可选，主动发送稳定选择器" /></Field>
      <Field label="协议">
        <Select
          value={bot.protocol}
          onChange={(event) => {
            const protocol = event.target.value
            patch(
              protocol === "qq-official"
                ? { protocol, transport: "gateway" }
                : { protocol, transport: bot.transport === "gateway" ? "ws-forward" : bot.transport },
            )
          }}
        >
          <option value="qq-official">QQ 官方机器人（qq-official）</option>
          <option value="onebot11">OneBot 11</option>
          <option value="onebot12">OneBot 12</option>
          <option value="satori">Satori</option>
        </Select>
      </Field>
      <Field label="传输">
        <Select value={bot.transport} onChange={(event) => patch({ transport: event.target.value })}>
          {bot.protocol === "qq-official" ? (
            <option value="gateway">官方网关（gateway）</option>
          ) : (
            <>
              <option value="ws-forward">正向 WebSocket</option>
              <option value="ws-reverse">反向 WebSocket</option>
              <option value="http">HTTP</option>
            </>
          )}
        </Select>
      </Field>
      {bot.protocol === "qq-official" ? (
        <>
          <Field label="QQ AppID"><Input value={bot.appid ?? ""} onChange={(event) => patch({ appid: event.target.value })} /></Field>
          <Field label="QQ AppSecret"><Input type="password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder={bot.secret_configured ? "已配置，留空保留" : "未配置"} /></Field>
        </>
      ) : (
        <>
          <Field label="远端地址（Endpoint）"><Input value={bot.endpoint ?? ""} onChange={(event) => patch({ endpoint: event.target.value })} placeholder="ws://127.0.0.1:3001" /></Field>
          <Field label="监听地址（Bind）"><Input value={bot.bind ?? ""} onChange={(event) => patch({ bind: event.target.value })} placeholder="127.0.0.1:6701" /></Field>
          <Field label="连接路径（Path）"><Input value={bot.path ?? ""} onChange={(event) => patch({ path: event.target.value })} placeholder="/onebot/reverse" /></Field>
          <Field label="访问令牌（Access Token）"><Input type="password" value={accessToken} onChange={(event) => setAccessToken(event.target.value)} placeholder={bot.access_token_configured ? "已配置，留空保留" : "未配置"} /></Field>
        </>
      )}

      <EditorSectionTitle title="事件与模块" description="选择机器人接收的事件，以及该实例可以使用的模块。" />
      {bot.protocol === "qq-official" && (
        <Field
          label="QQ 官方事件订阅（Intents）"
          hint="常用的三项已默认选择；只订阅机器人实际需要处理的事件。"
          wide
          controlGroup
        >
          <IntentSelector values={bot.intents} onChange={(intents) => patch({ intents })} />
        </Field>
      )}
      <Field
        label="Bot 启用模块"
        hint="只能选择宿主已经加载的模块；留空时使用全部已加载的内置模块。"
        wide
        controlGroup
      >
        <BotModuleSelector
          options={availableModules}
          values={bot.enabled_modules}
          onChange={(enabled_modules) => patch({ enabled_modules })}
        />
      </Field>

      <EditorSectionTitle title="权限与运行策略" description="设置管理身份、请求策略和单实例限速。" />
      <Field label="所有者 QQ"><Input value={listToText(bot.owners)} onChange={(event) => patch({ owners: textToList(event.target.value) })} /></Field>
      <Field label="管理员 QQ"><Input value={listToText(bot.admins)} onChange={(event) => patch({ admins: textToList(event.target.value) })} /></Field>
      <Field label="限速 rate"><Input type="number" value={bot.limiter.rate} onChange={(event) => patch({ limiter: { ...bot.limiter, rate: Number(event.target.value) } })} /></Field>
      <Field label="限速容量"><Input type="number" value={bot.limiter.capacity} onChange={(event) => patch({ limiter: { ...bot.limiter, capacity: Number(event.target.value) } })} /></Field>
      <div className="toggle-grid wide">
        <label><Switch checked={bot.configured_enabled} onCheckedChange={(enabled) => patch({ configured_enabled: enabled, desired_enabled: enabled })} />配置启用</label>
        <label><Switch checked={bot.sandbox} onCheckedChange={(sandbox) => patch({ sandbox })} />沙箱环境</label>
        <label><Switch checked={bot.limiter.enable} onCheckedChange={(enable) => patch({ limiter: { ...bot.limiter, enable } })} />启用限速</label>
        <label><Switch checked={bot.auto_reply_poke_enabled} onCheckedChange={(auto_reply_poke_enabled) => patch({ auto_reply_poke_enabled })} />戳一戳自动回复</label>
        <label><Switch checked={bot.auto_approve_friend_requests} onCheckedChange={(auto_approve_friend_requests) => patch({ auto_approve_friend_requests })} />自动同意好友请求</label>
        <label><Switch checked={bot.auto_approve_group_invites} onCheckedChange={(auto_approve_group_invites) => patch({ auto_approve_group_invites })} />自动同意群邀请</label>
      </div>
      <div className="wide flex flex-wrap gap-2 border-t border-border pt-3">
        <Button variant="outline" size="sm" onClick={() => onAction("start")} disabled={actionDisabled}>启动</Button>
        <Button variant="outline" size="sm" onClick={() => onAction("stop")} disabled={actionDisabled}>停止</Button>
        <Button variant="outline" size="sm" onClick={() => onAction("reconnect")} disabled={actionDisabled}>重连</Button>
      </div>
    </div>
  )
}

function EditorSectionTitle({ title, description }: { title: string; description: string }) {
  return (
    <div className="editor-section-title wide">
      <strong>{title}</strong>
      <span>{description}</span>
    </div>
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
  hint?: string
  children: React.ReactNode
  wide?: boolean
  controlGroup?: boolean
}) {
  const content = (
    <>
      <span>
        <strong>{label}</strong>
        {hint && <small>{hint}</small>}
      </span>
      {children}
    </>
  )
  return controlGroup ? (
    <div className={"field " + (wide ? "wide" : "")}>{content}</div>
  ) : (
    <label className={"field " + (wide ? "wide" : "")}>{content}</label>
  )
}

function IntentSelector({ values, onChange }: { values: string[]; onChange: (values: string[]) => void }) {
  const normalize = (value: string) => value.trim().toUpperCase()
  const matches = (option: IntentOption, value: string) => {
    const normalized = normalize(value)
    return normalized === option.id || option.aliases?.includes(normalized)
  }
  const unknown = values.filter((value) => !qqOfficialIntents.some((option) => matches(option, value)))

  const toggle = (option: IntentOption) => {
    const selected = values.some((value) => matches(option, value))
    if (selected) {
      onChange(values.filter((value) => !matches(option, value)))
    } else {
      onChange([...values, option.id])
    }
  }

  return (
    <div className="intent-picker">
      <div className="intent-picker-summary">
        <span><strong>{qqOfficialIntents.filter((option) => values.some((value) => matches(option, value))).length}</strong> 项已选择</span>
        <small>保存时写入右侧显示的官方 Intent ID</small>
      </div>
      <div className="intent-option-grid" role="group" aria-label="QQ 官方事件订阅">
        {qqOfficialIntents.map((option) => {
          const selected = values.some((value) => matches(option, value))
          return (
            <button
              type="button"
              key={option.id}
              className={selected ? "is-selected" : ""}
              aria-pressed={selected}
              onClick={() => toggle(option)}
            >
              <span className="intent-check">{selected && <Check />}</span>
              <span className="intent-copy">
                <span>
                  <strong>{option.label}</strong>
                  {option.recommended && <Badge variant="success">常用</Badge>}
                </span>
                <code>{option.id}</code>
                <small>{option.description}</small>
              </span>
            </button>
          )
        })}
      </div>
      <UnknownSelections values={unknown} onRemove={(value) => onChange(values.filter((item) => item !== value))} />
    </div>
  )
}

function BotModuleSelector({
  options,
  values,
  onChange,
}: {
  options: string[]
  values: string[]
  onChange: (values: string[]) => void
}) {
  const unknown = values.filter((value) => !options.includes(value))
  const toggle = (value: string) => {
    onChange(values.includes(value) ? values.filter((item) => item !== value) : [...values, value])
  }

  return (
    <div className="bot-module-picker">
      <div className="bot-module-options" role="group" aria-label="Bot 启用模块">
        {options.map((option) => {
          const selected = values.includes(option)
          return (
            <button type="button" key={option} className={selected ? "is-selected" : ""} aria-pressed={selected} onClick={() => toggle(option)}>
              <span className="intent-check">{selected && <Check />}</span>
              <span>
                <strong>{builtinModuleLabels[option] ?? option}</strong>
                <code>{option}</code>
              </span>
            </button>
          )
        })}
        {options.length === 0 && <span className="module-picker-empty">宿主配置中还没有可选模块</span>}
      </div>
      <UnknownSelections values={unknown} onRemove={(value) => onChange(values.filter((item) => item !== value))} />
    </div>
  )
}

function UnknownSelections({ values, onRemove }: { values: string[]; onRemove: (value: string) => void }) {
  if (values.length === 0) return null
  return (
    <div className="unknown-selection-list">
      <span>配置中存在当前列表无法识别的值，保存前请确认：</span>
      <div>
        {values.map((value) => (
          <Badge variant="warning" key={value}>
            {value}
            <button type="button" aria-label={"移除 " + value} onClick={() => onRemove(value)}><X /></button>
          </Badge>
        ))}
      </div>
    </div>
  )
}
