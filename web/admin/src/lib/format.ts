import type { BotMutation, BotView, GeneralConfigView, GeneralMutation } from "@/lib/api"

export function formatNumber(value: number) {
  return new Intl.NumberFormat("zh-CN").format(value)
}

export function formatBytes(value: number) {
  if (value < 1024) return value + " B"
  if (value < 1024 * 1024) return (value / 1024).toFixed(1) + " KB"
  return (value / 1024 / 1024).toFixed(1) + " MB"
}

export function formatUptime(seconds: number) {
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  const minutes = Math.floor((seconds % 3600) / 60)
  if (days > 0) return days + " 天 " + hours + " 小时"
  if (hours > 0) return hours + " 小时 " + minutes + " 分钟"
  return Math.max(minutes, 0) + " 分钟"
}

export function formatClock(input?: string | number | null) {
  if (!input) return "-"
  const date = typeof input === "number" ? new Date(input) : new Date(input)
  if (Number.isNaN(date.getTime())) return "-"
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(date)
}

export function relativeTime(epochMs?: number | null) {
  if (!epochMs) return "尚无事件"
  const diff = Math.max(0, Date.now() - epochMs)
  if (diff < 10_000) return "刚刚"
  if (diff < 60_000) return Math.floor(diff / 1000) + " 秒前"
  if (diff < 3_600_000) return Math.floor(diff / 60_000) + " 分钟前"
  return Math.floor(diff / 3_600_000) + " 小时前"
}

export function listToText(values?: string[] | null) {
  return (values ?? []).join(", ")
}

export function textToList(value: string) {
  return value
    .split(",")
    .map((item) => item.trim())
    .filter(Boolean)
}

export function botToMutation(bot: BotView, secrets?: { access_token?: string; secret?: string }): BotMutation {
  return {
    id: bot.id.trim(),
    account_id: emptyToNull(bot.account_id),
    protocol: bot.protocol.trim(),
    transport: bot.transport.trim(),
    endpoint: emptyToNull(bot.endpoint),
    bind: emptyToNull(bot.bind),
    path: emptyToNull(bot.path),
    access_token: secrets?.access_token === undefined ? null : secrets.access_token,
    appid: emptyToNull(bot.appid),
    secret: secrets?.secret === undefined ? null : secrets.secret,
    intents: bot.intents,
    sandbox: bot.sandbox,
    enabled: bot.configured_enabled,
    enabled_modules: bot.enabled_modules,
    owners: bot.owners,
    admins: bot.admins,
    auto_approve_friend_requests: bot.auto_approve_friend_requests,
    auto_approve_group_invites: bot.auto_approve_group_invites,
    auto_reply_poke_enabled: bot.auto_reply_poke_enabled,
    auto_reply_poke_message: emptyToNull(bot.auto_reply_poke_message),
    limiter: bot.limiter,
  }
}

export function defaultBot(): BotView {
  return {
    id: "new-bot",
    account_id: null,
    protocol: "qq-official",
    transport: "gateway",
    endpoint: null,
    bind: null,
    path: null,
    appid: null,
    secret_configured: false,
    access_token_configured: false,
    intents: ["GROUP_AND_C2C_EVENT", "PUBLIC_GUILD_MESSAGES", "DIRECT_MESSAGE"],
    sandbox: false,
    configured_enabled: false,
    desired_enabled: false,
    state: "disabled",
    state_since_epoch_ms: 0,
    last_event_epoch_ms: null,
    events_received: 0,
    reconnect_count: 0,
    last_error: null,
    enabled_modules: ["command", "admin"],
    owners: [],
    admins: [],
    auto_approve_friend_requests: false,
    auto_approve_group_invites: false,
    auto_reply_poke_enabled: true,
    auto_reply_poke_message: "",
    limiter: { enable: false, rate: 5, capacity: 10, timeout_secs: 0 },
  }
}

export function generalToMutation(
  general: GeneralConfigView,
  secrets?: { admin_access_token?: string; webhook_access_token?: string },
): GeneralMutation {
  return {
    environment: general.environment,
    shutdown_timeout_secs: general.shutdown_timeout_secs,
    task_grace_secs: general.task_grace_secs,
    log_level: general.log_level,
    json_logs: general.json_logs,
    admin_enabled: general.admin_enabled,
    admin_bind: general.admin_bind,
    admin_access_token: secrets?.admin_access_token === undefined ? null : secrets.admin_access_token,
    log_capacity: general.log_capacity,
    audit_path: general.audit_path,
    marketplace_enabled: general.marketplace_enabled,
    marketplace_cache_dir: general.marketplace_cache_dir,
    marketplace_lock_path: general.marketplace_lock_path,
    marketplace_request_timeout_secs: general.marketplace_request_timeout_secs,
    marketplace_allow_prerelease: general.marketplace_allow_prerelease,
    marketplace_auto_update: general.marketplace_auto_update,
    builtin_modules: general.builtin_modules,
    plugin_modules: general.plugin_modules,
    plugin_state_path: general.plugin_state_path,
    plugin_bin_dir: general.plugin_bin_dir,
    dynamic_plugin_timeout_secs: general.dynamic_plugin_timeout_secs,
    proactive_queue_capacity: general.proactive_queue_capacity,
    proactive_offline_ttl_secs: general.proactive_offline_ttl_secs,
    webhook_enabled: general.webhook_enabled,
    webhook_bind: general.webhook_bind,
    webhook_base_path: general.webhook_base_path,
    webhook_max_body_bytes: general.webhook_max_body_bytes,
    webhook_request_timeout_ms: general.webhook_request_timeout_ms,
    webhook_max_in_flight: general.webhook_max_in_flight,
    webhook_access_token: secrets?.webhook_access_token === undefined ? null : secrets.webhook_access_token,
  }
}

function emptyToNull(value?: string | null) {
  const trimmed = value?.trim()
  return trimmed ? trimmed : null
}
