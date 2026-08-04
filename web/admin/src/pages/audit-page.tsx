import { useCallback, useEffect, useState } from "react"
import { ChevronLeft, ChevronRight, RefreshCw, ShieldCheck } from "lucide-react"
import { toast } from "sonner"

import { api, type AuditEntry, type AuditView } from "@/lib/api"
import { formatClock } from "@/lib/format"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"

const pageSizeOptions = [20, 50, 100]

const emptyPagination: AuditView["pagination"] = {
  page: 1,
  page_size: 20,
  total_items: 0,
  total_pages: 1,
}

export function AuditPage() {
  const [entries, setEntries] = useState<AuditEntry[]>([])
  const [pagination, setPagination] = useState(emptyPagination)
  const [page, setPage] = useState(1)
  const [pageSize, setPageSize] = useState(20)
  const [busy, setBusy] = useState(false)

  const load = useCallback(async () => {
    setBusy(true)
    try {
      const result = await api.audit(page, pageSize)
      setEntries(result.entries)
      setPagination(result.pagination)
      setPage(result.pagination.page)
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "审计日志读取失败")
    } finally {
      setBusy(false)
    }
  }, [page, pageSize])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex items-center gap-2.5">
            <h1>安全审计</h1>
            <span className="environment-tag">{pagination.total_items} EVENTS</span>
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
        <AuditPagination
          pagination={pagination}
          pageSize={pageSize}
          disabled={busy}
          onPageChange={setPage}
          onPageSizeChange={(size) => {
            setPage(1)
            setPageSize(size)
          }}
        />
      </section>
    </main>
  )
}

function AuditPagination({
  pagination,
  pageSize,
  disabled,
  onPageChange,
  onPageSizeChange,
}: {
  pagination: AuditView["pagination"]
  pageSize: number
  disabled: boolean
  onPageChange: (page: number) => void
  onPageSizeChange: (pageSize: number) => void
}) {
  const start = pagination.total_items === 0 ? 0 : (pagination.page - 1) * pagination.page_size + 1
  const end = Math.min(pagination.page * pagination.page_size, pagination.total_items)
  return (
    <div className="audit-pagination">
      <span className="audit-pagination-range">
        <strong>{start}-{end}</strong>
        <small>共 {pagination.total_items} 条</small>
      </span>
      <div className="audit-pagination-controls" aria-label="审计日志分页">
        <Select
          value={String(pageSize)}
          onChange={(event) => onPageSizeChange(Number(event.target.value))}
          disabled={disabled}
          aria-label="每页审计日志数量"
        >
          {pageSizeOptions.map((size) => <option key={size} value={size}>每页 {size}</option>)}
        </Select>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onPageChange(pagination.page - 1)}
          disabled={disabled || pagination.page <= 1}
          title="上一页"
          aria-label="上一页审计日志"
        >
          <ChevronLeft />
        </Button>
        <span className="audit-pagination-page">
          <strong>{pagination.page}</strong>
          <small>/ {pagination.total_pages}</small>
        </span>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => onPageChange(pagination.page + 1)}
          disabled={disabled || pagination.page >= pagination.total_pages}
          title="下一页"
          aria-label="下一页审计日志"
        >
          <ChevronRight />
        </Button>
      </div>
    </div>
  )
}
