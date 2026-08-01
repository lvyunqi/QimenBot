import { useEffect, useState } from "react"
import { RefreshCw, ShieldCheck } from "lucide-react"
import { toast } from "sonner"

import { api, type AuditEntry } from "@/lib/api"
import { formatClock } from "@/lib/format"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"

export function AuditPage() {
  const [entries, setEntries] = useState<AuditEntry[]>([])
  const [busy, setBusy] = useState(false)

  const load = async () => {
    setBusy(true)
    try {
      setEntries(await api.audit())
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "审计日志读取失败")
    } finally {
      setBusy(false)
    }
  }

  useEffect(() => {
    void load()
  }, [])

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>安全审计</h1>
            <span className="environment-tag">{entries.length} EVENTS</span>
          </div>
          <p>记录面板发起的配置、Bot 和插件管理操作。</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy}>
          <RefreshCw className={busy ? "animate-spin-slow" : ""} />
          刷新
        </Button>
      </div>
      <section className="panel enter-panel">
        <div className="audit-list">
          {entries.map((entry) => (
            <article className="audit-row" key={entry.id}>
              <div className="placeholder-symbol audit-symbol">
                <ShieldCheck />
              </div>
              <div className="min-w-0">
                <div className="flex flex-wrap items-center gap-2">
                  <strong className="font-mono text-xs text-foreground">{entry.action}</strong>
                  <Badge variant={entry.outcome === "success" ? "success" : "warning"}>{entry.outcome}</Badge>
                  <span className="font-mono text-[10px] text-muted-foreground">{formatClock(entry.timestamp)}</span>
                </div>
                <p className="mt-1 text-xs text-muted-foreground">{entry.resource} · {entry.detail}</p>
              </div>
            </article>
          ))}
          {entries.length === 0 && <div className="empty-panel">还没有面板操作记录。</div>}
        </div>
      </section>
    </main>
  )
}
