import { useCallback, useEffect, useMemo, useState } from "react"
import * as AlertDialogPrimitive from "@radix-ui/react-alert-dialog"
import * as DialogPrimitive from "@radix-ui/react-dialog"
import {
  AlertCircle,
  CheckCircle2,
  FileCog,
  KeyRound,
  LoaderCircle,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
  X,
} from "lucide-react"
import { toast } from "sonner"

import {
  api,
  ApiError,
  type JsonValue,
  type PluginConfigMutation,
  type PluginConfigView,
  type PluginView,
} from "@/lib/api"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { SchemaForm } from "./schema-form"
import {
  applySchemaDefaults,
  asSchema,
  compileFormValidator,
  type FormValidator,
  type JsonObject,
  type SecretDraft,
} from "./schema-utils"

interface PluginConfigDrawerProps {
  plugin: PluginView | null
  onOpenChange: (open: boolean) => void
  onSaved: () => void | Promise<void>
}

interface ValidationState {
  tone: "success" | "warning" | "danger"
  message: string
}

export function PluginConfigDrawer({ plugin, onOpenChange, onSaved }: PluginConfigDrawerProps) {
  const [view, setView] = useState<PluginConfigView | null>(null)
  const [values, setValues] = useState<JsonObject>({})
  const [diskValues, setDiskValues] = useState<JsonObject>({})
  const [secrets, setSecrets] = useState<SecretDraft>({})
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(false)
  const [loadError, setLoadError] = useState("")
  const [validation, setValidation] = useState<ValidationState | null>(null)
  const [conflict, setConflict] = useState(false)
  const [confirmClose, setConfirmClose] = useState(false)
  const [validator, setValidator] = useState<FormValidator | null>(null)

  const load = useCallback(async () => {
    if (!plugin) return
    setLoading(true)
    setLoadError("")
    setConflict(false)
    try {
      const next = await api.pluginConfig(plugin.id)
      const schema = asSchema(next.schema)
      const rawValues = isObject(next.values) ? structuredClone(next.values) : {}
      const hydrated = applySchemaDefaults(schema, rawValues)
      setView(next)
      setDiskValues(rawValues)
      setValues(hydrated)
      setSecrets(Object.fromEntries(
        next.secrets
          .filter((secret) => secret.configured)
          .map((secret) => [secret.pointer, { source: secret.pointer }]),
      ))
      setValidation(null)
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : "插件配置读取失败")
      setView(null)
    } finally {
      setLoading(false)
    }
  }, [plugin])

  useEffect(() => {
    if (plugin) void load()
    else {
      setView(null)
      setValues({})
      setDiskValues({})
      setSecrets({})
      setLoadError("")
      setValidator(null)
    }
  }, [load, plugin])

  useEffect(() => {
    let active = true
    setValidator(null)
    if (view) {
      void compileFormValidator(asSchema(view.schema)).then((compiled) => {
        if (active) setValidator(() => compiled)
      })
    }
    return () => { active = false }
  }, [view])

  const errors = useMemo(() => validator ? validator(values, secrets) : [], [secrets, validator, values])
  const validatorReady = Boolean(validator)
  const dirty = useMemo(() => {
    if (JSON.stringify(values) !== JSON.stringify(diskValues)) return true
    return Object.entries(secrets).some(([pointer, entry]) => entry.update !== undefined || Boolean(entry.source && entry.source !== pointer))
  }, [diskValues, secrets, values])

  const mutation = (): PluginConfigMutation | null => {
    if (!view) return null
    const secretUpdates: Record<string, string | null> = {}
    const secretReferences: Record<string, string> = {}
    for (const [pointer, entry] of Object.entries(secrets)) {
      if (entry.update !== undefined) secretUpdates[pointer] = entry.update
      else if (entry.source) secretReferences[pointer] = entry.source
    }
    return {
      revision: view.revision,
      values,
      secret_updates: secretUpdates,
      secret_references: secretReferences,
    }
  }

  const validate = async () => {
    if (!plugin || !validatorReady || errors.length > 0) return
    const payload = mutation()
    if (!payload) return
    setBusy(true)
    setValidation(null)
    try {
      const result = await api.validatePluginConfig(plugin.id, payload)
      setValidation({ tone: "success", message: result.message })
    } catch (error) {
      setValidation({ tone: "danger", message: error instanceof Error ? error.message : "配置校验失败" })
      if (error instanceof ApiError && error.status === 409) setConflict(true)
    } finally {
      setBusy(false)
    }
  }

  const save = async () => {
    if (!plugin || !validatorReady || errors.length > 0 || !dirty) return
    const payload = mutation()
    if (!payload) return
    setBusy(true)
    setValidation(null)
    try {
      const result = await api.savePluginConfig(plugin.id, payload)
      toast.success(result.message)
      await onSaved()
      await load()
    } catch (error) {
      const message = error instanceof Error ? error.message : "插件配置保存失败"
      setValidation({ tone: "danger", message })
      if (error instanceof ApiError && error.status === 409) setConflict(true)
      else toast.error(message)
    } finally {
      setBusy(false)
    }
  }

  const requestClose = () => {
    if (dirty && !busy) setConfirmClose(true)
    else onOpenChange(false)
  }

  const updateValues = (next: JsonObject) => {
    setValues(next)
    setValidation(null)
    setConflict(false)
  }

  const updateSecrets = (next: SecretDraft) => {
    setSecrets(next)
    setValidation(null)
    setConflict(false)
  }

  return (
    <>
      <DialogPrimitive.Root open={Boolean(plugin)} onOpenChange={(open) => { if (!open) requestClose() }}>
        <DialogPrimitive.Portal>
          <DialogPrimitive.Overlay className="plugin-config-overlay" />
          <DialogPrimitive.Content className="plugin-config-drawer" onEscapeKeyDown={(event) => { if (dirty) event.preventDefault() }}>
            <header className="plugin-config-header">
              <div className="plugin-config-heading-icon"><FileCog /></div>
              <div className="min-w-0 flex-1">
                <div className="plugin-config-title-row">
                  <DialogPrimitive.Title>{plugin?.name || plugin?.id || "插件配置"}</DialogPrimitive.Title>
                  {view && <Badge variant="neutral">配置 v{view.config_version}</Badge>}
                  {view && <Badge variant={applyModeBadge(view.apply_mode)}>{applyModeLabel(view.apply_mode)}</Badge>}
                </div>
                <DialogPrimitive.Description>
                  {plugin?.id ?? "dynamic-plugin"}
                  {view && <span> · 插件 v{view.plugin_version}</span>}
                </DialogPrimitive.Description>
              </div>
              <Button variant="ghost" size="icon" aria-label="关闭插件配置" onClick={requestClose} disabled={busy}>
                <X />
              </Button>
            </header>

            <div className="plugin-config-statusbar">
              <span><KeyRound />密钥只写不读</span>
              {view?.validates_config && <span><ShieldCheck />插件语义校验</span>}
              {view && <span className={view.loaded ? "is-loaded" : ""}><span className="status-dot" />{view.loaded ? "插件已加载" : "下次加载生效"}</span>}
              {view?.config_file && <code title={view.config_file}>{view.config_file}</code>}
            </div>

            <div className="plugin-config-scroll">
              {loading ? (
                <div className="plugin-config-state"><LoaderCircle className="animate-spin-slow" /><span>正在读取插件配置</span></div>
              ) : loadError ? (
                <div className="plugin-config-state is-error">
                  <AlertCircle />
                  <strong>配置无法打开</strong>
                  <span>{loadError}</span>
                  <Button variant="outline" size="sm" onClick={() => void load()}><RefreshCw />重试</Button>
                </div>
              ) : view ? (
                <>
                  {conflict && (
                    <div className="plugin-config-notice is-warning" role="alert">
                      <AlertCircle />
                      <div><strong>磁盘配置已经变化</strong><span>重新读取后才能继续保存。</span></div>
                      <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy}><RefreshCw />重新读取</Button>
                    </div>
                  )}
                  {validation && (
                    <div className={`plugin-config-notice is-${validation.tone}`} role="status">
                      {validation.tone === "success" ? <CheckCircle2 /> : <AlertCircle />}
                      <div><strong>{validation.tone === "success" ? "配置有效" : "配置未通过"}</strong><span>{validation.message}</span></div>
                    </div>
                  )}
                  {typeof view.schema.title === "string" && (
                    <div className="plugin-config-schema-heading">
                      <h3>{view.schema.title}</h3>
                      {typeof view.schema.description === "string" && <p>{view.schema.description}</p>}
                    </div>
                  )}
                  <SchemaForm
                    schema={asSchema(view.schema)}
                    uiSchema={asSchema(view.ui_schema)}
                    value={values}
                    errors={errors}
                    secrets={secrets}
                    disabled={busy || conflict}
                    onChange={updateValues}
                    onSecretsChange={updateSecrets}
                  />
                </>
              ) : null}
            </div>

            <footer className="plugin-config-footer">
                <div className="plugin-config-save-state">
                <span className={!validatorReady ? "is-pending" : dirty ? "is-dirty" : errors.length ? "is-error" : "is-clean"} />
                <div>
                  <strong>{!validatorReady ? "正在准备本地校验" : errors.length ? `${errors.length} 处配置需要处理` : dirty ? "有未保存更改" : "配置与磁盘一致"}</strong>
                  <small>{view ? `revision ${view.revision.slice(0, 12)}` : "等待配置数据"}</small>
                </div>
              </div>
              <div className="plugin-config-footer-actions">
                <Button variant="ghost" size="sm" onClick={() => void load()} disabled={busy || loading || !view || !dirty}>
                  <RotateCcw />撤销
                </Button>
                <Button variant="outline" size="sm" onClick={() => void validate()} disabled={busy || loading || !view || !validatorReady || errors.length > 0 || conflict}>
                  <ShieldCheck />检查配置
                </Button>
                <Button size="sm" onClick={() => void save()} disabled={busy || loading || !view || !validatorReady || !dirty || errors.length > 0 || conflict}>
                  {busy ? <LoaderCircle className="animate-spin-slow" /> : <Save />}
                  {busy ? "处理中" : "保存配置"}
                </Button>
              </div>
            </footer>
          </DialogPrimitive.Content>
        </DialogPrimitive.Portal>
      </DialogPrimitive.Root>

      <AlertDialogPrimitive.Root open={confirmClose} onOpenChange={setConfirmClose}>
        <AlertDialogPrimitive.Portal>
          <AlertDialogPrimitive.Overlay className="plugin-config-confirm-overlay" />
          <AlertDialogPrimitive.Content className="plugin-config-confirm">
            <AlertDialogPrimitive.Title>放弃未保存更改？</AlertDialogPrimitive.Title>
            <AlertDialogPrimitive.Description>当前插件配置还没有写入磁盘。</AlertDialogPrimitive.Description>
            <div>
              <AlertDialogPrimitive.Cancel asChild><Button variant="outline">继续编辑</Button></AlertDialogPrimitive.Cancel>
              <AlertDialogPrimitive.Action asChild>
                <Button variant="destructive" onClick={() => onOpenChange(false)}>放弃更改</Button>
              </AlertDialogPrimitive.Action>
            </div>
          </AlertDialogPrimitive.Content>
        </AlertDialogPrimitive.Portal>
      </AlertDialogPrimitive.Root>
    </>
  )
}

function isObject(value: Record<string, JsonValue>): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function applyModeLabel(mode: PluginConfigView["apply_mode"]) {
  if (mode === "live") return "即时生效"
  if (mode === "reload") return "重载生效"
  return "重启生效"
}

function applyModeBadge(mode: PluginConfigView["apply_mode"]): "success" | "default" | "warning" {
  if (mode === "live") return "success"
  if (mode === "reload") return "default"
  return "warning"
}
