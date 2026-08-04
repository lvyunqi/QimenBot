export interface ApiEnvelope<T> {
  data: T
}

export interface LogEntry {
  id: number
  timestamp: string
  level: string
  target: string
  message: string
  fields: Record<string, string>
}

export interface ThroughputPoint {
  minute_epoch: number
  events: number
  replies: number
}

export interface RateLimiterView {
  enable: boolean
  rate: number
  capacity: number
  timeout_secs: number
}

export interface BotView {
  id: string
  account_id?: string | null
  protocol: string
  transport: string
  endpoint?: string | null
  bind?: string | null
  path?: string | null
  appid?: string | null
  secret_configured: boolean
  access_token_configured: boolean
  intents: string[]
  sandbox: boolean
  configured_enabled: boolean
  desired_enabled: boolean
  state: "disabled" | "starting" | "online" | "reconnecting" | "stopped" | "error"
  state_since_epoch_ms: number
  last_event_epoch_ms?: number | null
  events_received: number
  reconnect_count: number
  last_error?: string | null
  enabled_modules: string[]
  owners: string[]
  admins: string[]
  auto_approve_friend_requests: boolean
  auto_approve_group_invites: boolean
  auto_reply_poke_enabled: boolean
  auto_reply_poke_message?: string | null
  limiter: RateLimiterView
}

export interface GeneralConfigView {
  environment: string
  shutdown_timeout_secs: number
  task_grace_secs: number
  log_level: string
  json_logs: boolean
  admin_enabled: boolean
  admin_bind: string
  admin_token_configured: boolean
  log_capacity: number
  audit_path: string
  marketplace_enabled: boolean
  marketplace_cache_dir: string
  marketplace_lock_path: string
  marketplace_request_timeout_secs: number
  marketplace_allow_prerelease: boolean
  marketplace_auto_update: boolean
  builtin_modules: string[]
  plugin_modules: string[]
  plugin_state_path: string
  plugin_bin_dir: string
  plugin_config_dir: string
  dynamic_plugin_timeout_secs: number
  proactive_queue_capacity: number
  proactive_offline_ttl_secs: number
  webhook_enabled: boolean
  webhook_bind: string
  webhook_base_path: string
  webhook_max_body_bytes: number
  webhook_request_timeout_ms: number
  webhook_max_in_flight: number
  webhook_token_configured: boolean
}

export interface ConfigView {
  revision: string
  restart_required: boolean
  general: GeneralConfigView
  bots: BotView[]
}

export interface AdminSnapshot {
  server: {
    version: string
    environment: string
    uptime_secs: number
    now: string
    config_revision: string
    restart_required: boolean
  }
  metrics: {
    events_total: number
    replies_total: number
    online_bots: number
    configured_bots: number
    loaded_dynamic_plugins: number
    warning_count: number
    throughput: ThroughputPoint[]
  }
  resources: {
    log_entries: number
    log_capacity: number
    dynamic_plugin_failures: number
    active_bot_supervisors: number
  }
  bots: BotView[]
  recent_logs: LogEntry[]
}

export interface LogsView {
  entries: LogEntry[]
  total_buffered: number
  capacity: number
}

export interface PluginView {
  id: string
  kind: "builtin" | "static" | "dynamic" | string
  name?: string | null
  description?: string | null
  version?: string | null
  api_version?: string | null
  configured?: boolean
  available?: boolean
  enabled: boolean
  loaded: boolean
  file_name?: string | null
  commands: string[]
  routes: string[]
  system_plugins?: string[]
  interceptors?: number
  webhooks: string[]
  failures: number
  last_error?: string | null
  live_toggle: boolean
  configurable: boolean
  config_apply_mode?: "live" | "reload" | "restart" | null
  config_version?: number | null
  config_file_exists: boolean
}

export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue }

export interface PluginSecretState {
  pointer: string
  configured: boolean
}

export interface PluginConfigView {
  plugin_id: string
  plugin_version: string
  config_version: number
  apply_mode: "live" | "reload" | "restart"
  loaded: boolean
  validates_config: boolean
  applies_live: boolean
  exists: boolean
  revision: string
  config_file: string
  schema: Record<string, JsonValue>
  ui_schema: Record<string, JsonValue>
  values: Record<string, JsonValue>
  secrets: PluginSecretState[]
}

export interface PluginConfigMutation {
  revision: string
  values: Record<string, JsonValue>
  secret_updates: Record<string, string | null>
  secret_references?: Record<string, string>
}

export interface PluginConfigValidationView {
  valid: boolean
  message: string
}

export type MarketplacePluginKind = "dynamic" | "static"
export type MarketplaceTrust = "community" | "verified-build" | "official"
export type MarketplaceChannel = "stable" | "prerelease"
export type MarketplaceDriver = "onebot11" | "qq-official"
export type MarketplaceMessageScene = "private" | "group" | "group-at" | "channel" | "channel-at" | "channel-private"
export type MarketplaceDriverEvent = "message" | "notice" | "request" | "meta"
export type MarketplaceOutboundCapability = "reply" | "proactive" | "rich-message"

export interface MarketplaceDriverSupport {
  driver: MarketplaceDriver
  scenes: MarketplaceMessageScene[]
  events: MarketplaceDriverEvent[]
  outbound: MarketplaceOutboundCapability[]
}

export interface MarketplaceVersionView {
  version: string
  released_at: string
  channel: MarketplaceChannel
  qimenbot: string
  dynamic_api?: string | null
  yanked: boolean
  data_schema_version: number
  rollback_safe: boolean
  changelog: string
  drivers: MarketplaceDriverSupport[]
  compatible: boolean
  installable: boolean
  asset_name?: string | null
  asset_target?: string | null
  asset_size_bytes?: number | null
  asset_sha256?: string | null
  min_glibc?: string | null
  github_attestation: boolean
  issues: string[]
}

export interface MarketplaceInstalledView {
  version: string
  active_file: string
  target: string
  sha256: string
  installed_at: string
  pinned: boolean
  active: boolean
  loaded: boolean
  update_available: boolean
  can_rollback: boolean
  data_schema_version: number
}

export interface MarketplaceUnmanagedView {
  version: string
  file_name: string
  sha256?: string | null
  can_adopt: boolean
  reason: string
}

export type MarketplaceFilter = "all" | "dynamic" | "static" | "installed" | "updates"

export interface MarketplacePluginSummaryView {
  id: string
  name: string
  summary: string
  kind: MarketplacePluginKind
  license: string
  trust: MarketplaceTrust
  catalog_listed: boolean
  latest_compatible?: string | null
  drivers: MarketplaceDriverSupport[]
  installed?: MarketplaceInstalledView | null
  unmanaged?: MarketplaceUnmanagedView | null
}

export interface MarketplacePluginView extends MarketplacePluginSummaryView {
  description: string
  repository: string
  repository_url: string
  repository_id: number
  authors: string[]
  categories: string[]
  keywords: string[]
  versions: MarketplaceVersionView[]
}

export interface MarketplaceListParams {
  page: number
  page_size: number
  query: string
  filter: MarketplaceFilter
}

export interface MarketplaceView {
  enabled: boolean
  allow_prerelease: boolean
  auto_update: boolean
  source?: "network" | "cache" | null
  fetched_at?: string | null
  warning?: string | null
  host: {
    qimenbot_version: string
    target: string
    os: string
    arch: string
    environment: string
    glibc?: string | null
    dynamic_loading: boolean
    supported_dynamic_apis: string[]
  }
  counts: Record<MarketplaceFilter, number>
  pagination: {
    page: number
    page_size: number
    total_items: number
    total_pages: number
  }
  plugins: MarketplacePluginSummaryView[]
}

export interface RevisionView {
  revision: string
  created_at: string
  size_bytes: number
  current: boolean
}

export interface AuditEntry {
  id: string
  timestamp: string
  action: string
  resource: string
  outcome: string
  detail: string
}

export interface MutationResult {
  revision?: string | null
  restart_required: boolean
  message: string
}

export type DeploymentKind = "binary_managed" | "docker" | "direct_binary"
export type UpdatePhase =
  | "idle"
  | "checking"
  | "up_to_date"
  | "available"
  | "downloading"
  | "ready"
  | "applying"
  | "restarting"
  | "rolled_back"
  | "error"

export interface UpdateStatus {
  schema_version: number
  deployment: DeploymentKind
  phase: UpdatePhase
  current_version: string
  launcher_version: string
  target: string
  channel: string
  auto_install: boolean
  available_version?: string | null
  release_url?: string | null
  progress_percent?: number | null
  message: string
  checked_at_epoch_ms?: number | null
  updated_at_epoch_ms: number
}

export interface UpdateView {
  deployment: DeploymentKind
  managed: boolean
  status?: UpdateStatus | null
  message: string
}

export class ApiError extends Error {
  status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = "ApiError"
    this.status = status
  }
}

interface ApiErrorBody {
  error?: {
    code?: string
    message?: string
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers)
  const token = sessionStorage.getItem("qimen-admin-token")
  if (token) headers.set("authorization", "Bearer " + token)
  if (init?.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json")
  }
  const response = await fetch("/api/v1" + path, { ...init, headers })
  if (!response.ok) {
    let message = response.status + " " + response.statusText
    try {
      const parsed = (await response.json()) as ApiErrorBody
      message = parsed.error?.message ?? message
    } catch {
      // Keep HTTP status fallback.
    }
    throw new ApiError(response.status, message)
  }
  const envelope = (await response.json()) as ApiEnvelope<T>
  return envelope.data
}

function marketplacePath(path: string, params: MarketplaceListParams) {
  const query = new URLSearchParams({
    page: String(params.page),
    page_size: String(params.page_size),
    query: params.query,
    filter: params.filter,
  })
  return `${path}?${query}`
}

export const api = {
  snapshot: () => request<AdminSnapshot>("/snapshot"),
  bots: () => request<BotView[]>("/bots"),
  config: () => request<ConfigView>("/config"),
  plugins: () => request<PluginView[]>("/plugins"),
  marketplace: (params: MarketplaceListParams) =>
    request<MarketplaceView>(marketplacePath("/marketplace", params)),
  refreshMarketplace: (params: MarketplaceListParams) =>
    request<MarketplaceView>(marketplacePath("/marketplace/refresh", params), { method: "POST" }),
  marketplacePlugin: (id: string) =>
    request<MarketplacePluginView>("/marketplace/plugins/" + encodeURIComponent(id)),
  logs: (params?: URLSearchParams) => request<LogsView>("/logs" + (params ? "?" + params : "")),
  audit: () => request<AuditEntry[]>("/audit"),
  updates: () => request<UpdateView>("/updates"),
  revisions: () => request<RevisionView[]>("/config/revisions"),
  botAction: (id: string, action: "start" | "stop" | "reconnect") =>
    request<MutationResult>("/bots/" + encodeURIComponent(id) + "/actions", {
      method: "POST",
      body: JSON.stringify({ action }),
    }),
  saveBot: (revision: string, bot: BotMutation, existingId?: string) =>
    request<MutationResult>(existingId ? "/bots/" + encodeURIComponent(existingId) : "/bots", {
      method: existingId ? "PUT" : "POST",
      body: JSON.stringify({ revision, bot }),
    }),
  deleteBot: (revision: string, id: string) =>
    request<MutationResult>("/bots/" + encodeURIComponent(id), {
      method: "DELETE",
      body: JSON.stringify({ revision }),
    }),
  togglePlugin: (id: string, enabled: boolean) =>
    request<MutationResult>("/plugins/" + encodeURIComponent(id), {
      method: "PUT",
      body: JSON.stringify({ enabled }),
    }),
  reloadPlugins: () =>
    request<MutationResult>("/plugins/reload", {
      method: "POST",
    }),
  pluginConfig: (id: string) =>
    request<PluginConfigView>("/plugins/" + encodeURIComponent(id) + "/config"),
  validatePluginConfig: (id: string, config: PluginConfigMutation) =>
    request<PluginConfigValidationView>("/plugins/" + encodeURIComponent(id) + "/config/validate", {
      method: "POST",
      body: JSON.stringify(config),
    }),
  savePluginConfig: (id: string, config: PluginConfigMutation) =>
    request<MutationResult>("/plugins/" + encodeURIComponent(id) + "/config", {
      method: "PUT",
      body: JSON.stringify(config),
    }),
  installMarketplacePlugin: (id: string, version?: string) =>
    request<MutationResult>("/marketplace/plugins/" + encodeURIComponent(id) + "/install", {
      method: "POST",
      body: JSON.stringify({ version: version || null }),
    }),
  adoptMarketplacePlugin: (id: string, version?: string) =>
    request<MutationResult>("/marketplace/plugins/" + encodeURIComponent(id) + "/adopt", {
      method: "POST",
      body: JSON.stringify({ version: version || null }),
    }),
  pinMarketplacePlugin: (id: string, pinned: boolean) =>
    request<MutationResult>("/marketplace/plugins/" + encodeURIComponent(id) + "/pin", {
      method: "PUT",
      body: JSON.stringify({ pinned }),
    }),
  rollbackMarketplacePlugin: (id: string) =>
    request<MutationResult>("/marketplace/plugins/" + encodeURIComponent(id) + "/rollback", {
      method: "POST",
    }),
  uninstallMarketplacePlugin: (id: string) =>
    request<MutationResult>("/marketplace/plugins/" + encodeURIComponent(id), {
      method: "DELETE",
    }),
  updateGeneral: (revision: string, general: GeneralMutation) =>
    request<MutationResult>("/config/general", {
      method: "PUT",
      body: JSON.stringify({ revision, general }),
    }),
  rollback: (revision: string) =>
    request<MutationResult>("/config/rollback", {
      method: "POST",
      body: JSON.stringify({ revision }),
    }),
  checkUpdates: () => request<MutationResult>("/updates/check", { method: "POST" }),
  installUpdate: () => request<MutationResult>("/updates/install", { method: "POST" }),
  restartRuntime: () => request<MutationResult>("/updates/restart", { method: "POST" }),
}

export function setApiToken(token: string) {
  if (token.trim()) sessionStorage.setItem("qimen-admin-token", token.trim())
  else sessionStorage.removeItem("qimen-admin-token")
}

export interface BotMutation {
  id: string
  account_id?: string | null
  protocol: string
  transport: string
  endpoint?: string | null
  bind?: string | null
  path?: string | null
  access_token?: string | null
  appid?: string | null
  secret?: string | null
  intents: string[]
  sandbox: boolean
  enabled: boolean
  enabled_modules: string[]
  owners: string[]
  admins: string[]
  auto_approve_friend_requests: boolean
  auto_approve_group_invites: boolean
  auto_reply_poke_enabled: boolean
  auto_reply_poke_message?: string | null
  limiter: RateLimiterView
}

export interface GeneralMutation {
  environment: string
  shutdown_timeout_secs: number
  task_grace_secs: number
  log_level: string
  json_logs: boolean
  admin_enabled: boolean
  admin_bind: string
  admin_access_token?: string | null
  log_capacity: number
  audit_path: string
  marketplace_enabled: boolean
  marketplace_cache_dir: string
  marketplace_lock_path: string
  marketplace_request_timeout_secs: number
  marketplace_allow_prerelease: boolean
  marketplace_auto_update: boolean
  builtin_modules: string[]
  plugin_modules: string[]
  plugin_state_path: string
  plugin_bin_dir: string
  plugin_config_dir: string
  dynamic_plugin_timeout_secs: number
  proactive_queue_capacity: number
  proactive_offline_ttl_secs: number
  webhook_enabled: boolean
  webhook_bind: string
  webhook_base_path: string
  webhook_max_body_bytes: number
  webhook_request_timeout_ms: number
  webhook_max_in_flight: number
  webhook_access_token?: string | null
}

export function openLogStream(onEntry: (entry: LogEntry) => void, onError?: () => void, onOpen?: () => void) {
  const controller = new AbortController()
  const token = sessionStorage.getItem("qimen-admin-token")
  const headers = new Headers()
  if (token) headers.set("authorization", "Bearer " + token)

  void fetch("/api/v1/logs/stream", { headers, signal: controller.signal })
    .then(async (response) => {
      if (!response.ok || !response.body) throw new Error("log stream unavailable")
      onOpen?.()
      const reader = response.body.getReader()
      const decoder = new TextDecoder()
      let buffer = ""
      while (true) {
        const { done, value } = await reader.read()
        if (done) break
        buffer += decoder.decode(value, { stream: true }).replace(/\r\n/g, "\n")
        let boundary = buffer.indexOf("\n\n")
        while (boundary >= 0) {
          const block = buffer.slice(0, boundary)
          buffer = buffer.slice(boundary + 2)
          const data = block
            .split("\n")
            .filter((line) => line.startsWith("data:"))
            .map((line) => line.slice(5).trimStart())
            .join("\n")
          if (data && data !== "keep-alive") onEntry(JSON.parse(data) as LogEntry)
          boundary = buffer.indexOf("\n\n")
        }
      }
    })
    .catch(() => {
      if (!controller.signal.aborted) onError?.()
    })
  return { close: () => controller.abort() }
}
