import { useEffect, useMemo, useState } from "react"
import type React from "react"
import { Plus, RefreshCw, Save, Trash2 } from "lucide-react"
import { toast } from "sonner"

import { api, type BotView, type ConfigView } from "@/lib/api"
import { botToMutation, defaultBot, formatNumber, listToText, relativeTime, textToList } from "@/lib/format"
import { botStatusLabel, botStatusVariant } from "@/lib/status"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input, Textarea } from "@/components/ui/input"
import { Select } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"

interface BotsPageProps {
  snapshotBots: BotView[]
  onRefreshSnapshot: () => void
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
}: {
  bot: BotView
  setBot: (bot: BotView) => void
  accessToken: string
  secret: string
  setAccessToken: (value: string) => void
  setSecret: (value: string) => void
  onAction: (action: "start" | "stop" | "reconnect") => void
  actionDisabled: boolean
}) {
  const patch = (next: Partial<BotView>) => setBot({ ...bot, ...next })

  return (
    <div className="editor-grid">
      <Field label="实例 ID"><Input value={bot.id} onChange={(event) => patch({ id: event.target.value })} /></Field>
      <Field label="账号标识"><Input value={bot.account_id ?? ""} onChange={(event) => patch({ account_id: event.target.value })} placeholder="可选，主动发送稳定选择器" /></Field>
      <Field label="协议">
        <Select value={bot.protocol} onChange={(event) => patch({ protocol: event.target.value })}>
          <option value="qq-official">qq-official</option>
          <option value="onebot11">onebot11</option>
          <option value="onebot12">onebot12</option>
          <option value="satori">satori</option>
        </Select>
      </Field>
      <Field label="传输">
        <Select value={bot.transport} onChange={(event) => patch({ transport: event.target.value })}>
          <option value="gateway">gateway</option>
          <option value="ws-forward">ws-forward</option>
          <option value="ws-reverse">ws-reverse</option>
          <option value="http">http</option>
        </Select>
      </Field>
      <Field label="Endpoint"><Input value={bot.endpoint ?? ""} onChange={(event) => patch({ endpoint: event.target.value })} placeholder="ws://127.0.0.1:3001" /></Field>
      <Field label="Bind"><Input value={bot.bind ?? ""} onChange={(event) => patch({ bind: event.target.value })} placeholder="127.0.0.1:6701" /></Field>
      <Field label="Path"><Input value={bot.path ?? ""} onChange={(event) => patch({ path: event.target.value })} placeholder="/onebot/reverse" /></Field>
      <Field label="AppID"><Input value={bot.appid ?? ""} onChange={(event) => patch({ appid: event.target.value })} /></Field>
      <Field label="Access Token"><Input type="password" value={accessToken} onChange={(event) => setAccessToken(event.target.value)} placeholder={bot.access_token_configured ? "已配置，留空保留" : "未配置"} /></Field>
      <Field label="Secret"><Input type="password" value={secret} onChange={(event) => setSecret(event.target.value)} placeholder={bot.secret_configured ? "已配置，留空保留" : "未配置"} /></Field>
      <Field label="Intents" wide><Textarea value={listToText(bot.intents)} onChange={(event) => patch({ intents: textToList(event.target.value) })} /></Field>
      <Field label="启用模块" wide><Input value={listToText(bot.enabled_modules)} onChange={(event) => patch({ enabled_modules: textToList(event.target.value) })} /></Field>
      <Field label="Owners"><Input value={listToText(bot.owners)} onChange={(event) => patch({ owners: textToList(event.target.value) })} /></Field>
      <Field label="Admins"><Input value={listToText(bot.admins)} onChange={(event) => patch({ admins: textToList(event.target.value) })} /></Field>
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

function Field({ label, children, wide }: { label: string; children: React.ReactNode; wide?: boolean }) {
  return (
    <label className={"field " + (wide ? "wide" : "")}>
      <span>{label}</span>
      {children}
    </label>
  )
}
