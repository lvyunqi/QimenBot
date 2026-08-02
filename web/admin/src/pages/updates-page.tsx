import { useEffect, useMemo, useState } from "react"
import { CheckCircle2, Cloud, Download, ExternalLink, GitBranch, RefreshCw, RotateCcw, Server, ShieldCheck, TriangleAlert } from "lucide-react"
import { toast } from "sonner"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { api, type DeploymentKind, type UpdatePhase, type UpdateView } from "@/lib/api"
import { formatClock } from "@/lib/format"

const phaseLabels: Record<UpdatePhase, string> = {
  idle: "等待检查",
  checking: "检查中",
  up_to_date: "已是最新",
  available: "有新版本",
  downloading: "下载中",
  ready: "等待重启",
  applying: "安装中",
  restarting: "重启中",
  rolled_back: "已回滚",
  error: "更新失败",
}

const deploymentLabels: Record<DeploymentKind, string> = {
  binary_managed: "launcher 托管二进制",
  docker: "Docker 编排托管",
  direct_binary: "直接启动二进制",
}

function phaseVariant(phase?: UpdatePhase): "default" | "success" | "warning" | "danger" | "neutral" {
  if (phase === "available" || phase === "ready") return "warning"
  if (phase === "error" || phase === "rolled_back") return "danger"
  if (phase === "up_to_date") return "success"
  if (phase === "checking" || phase === "downloading" || phase === "applying" || phase === "restarting") return "default"
  return "neutral"
}

function DeploymentIcon({ kind }: { kind?: DeploymentKind }) {
  if (kind === "docker") return <Cloud />
  if (kind === "binary_managed") return <Server />
  return <TriangleAlert />
}

export function UpdatesPage() {
  const [view, setView] = useState<UpdateView>()
  const [busy, setBusy] = useState<"check" | "install" | "restart" | null>(null)

  const load = async () => {
    try {
      setView(await api.updates())
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "更新状态读取失败")
    }
  }

  useEffect(() => {
    void load()
    const timer = window.setInterval(() => void load(), 2500)
    return () => window.clearInterval(timer)
  }, [])

  const status = view?.status
  const progress = status?.progress_percent ?? 0
  const isWorking = status && ["checking", "downloading", "applying", "restarting"].includes(status.phase)
  const canInstall = view?.managed && status?.phase === "available"
  const canRestart = view?.managed && !isWorking
  const lastChecked = status?.checked_at_epoch_ms ? formatClock(new Date(status.checked_at_epoch_ms).toISOString()) : "尚未检查"

  const action = async (kind: "check" | "install" | "restart") => {
    setBusy(kind)
    try {
      const result = kind === "check" ? await api.checkUpdates() : kind === "install" ? await api.installUpdate() : await api.restartRuntime()
      toast.success(result.message)
      await load()
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "更新操作失败")
    } finally {
      setBusy(null)
    }
  }

  const summary = useMemo(() => {
    if (!view) return "正在读取部署方式"
    if (view.deployment === "docker") return "镜像由 Docker Hub 发布，更新应由 Compose 或编排平台拉取新镜像。"
    if (view.deployment === "direct_binary") return "当前进程没有 launcher 的监督与回滚能力。"
    return "launcher 会先下载并校验两个二进制，健康检查通过后才清理旧版本。"
  }, [view])

  return (
    <main className="page-shell">
      <div className="page-heading enter-item">
        <div>
          <div className="flex flex-wrap items-center gap-2.5">
            <h1>版本更新</h1>
            <span className="environment-tag">RELEASE CHANNEL</span>
          </div>
          <p>更新流程保留配置、插件和数据，只替换经过校验的程序文件。</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => void load()} disabled={busy !== null}>
          <RefreshCw className={busy ? "animate-spin-slow" : ""} />
          刷新状态
        </Button>
      </div>

      <section className="panel enter-panel overflow-hidden">
        <div className="flex flex-wrap items-start justify-between gap-4 border-b border-border px-5 py-5">
          <div className="flex min-w-0 items-start gap-3">
            <div className="metric-icon is-brand"><DeploymentIcon kind={view?.deployment} /></div>
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h2 className="text-base font-extrabold text-foreground">{view ? deploymentLabels[view.deployment] : "部署方式"}</h2>
                {status && <Badge variant={phaseVariant(status.phase)}>{phaseLabels[status.phase]}</Badge>}
              </div>
              <p className="mt-1 max-w-2xl text-xs leading-5 text-muted-foreground">{view?.message ?? summary}</p>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="sm" onClick={() => void action("check")} disabled={!view?.managed || busy !== null}>
              <GitBranch />
              检查更新
            </Button>
            <Button size="sm" onClick={() => void action("install")} disabled={!canInstall || busy !== null}>
              <Download />
              安装新版本
            </Button>
          </div>
        </div>

        {status && (
          <div className="grid gap-px border-b border-border bg-border sm:grid-cols-4">
            <div className="bg-surface px-5 py-4"><span className="text-[10px] font-bold uppercase text-muted-foreground">当前版本</span><strong className="mt-1 block font-mono text-sm">v{status.current_version}</strong></div>
            <div className="bg-surface px-5 py-4"><span className="text-[10px] font-bold uppercase text-muted-foreground">可用版本</span><strong className="mt-1 block font-mono text-sm">{status.available_version ? "v" + status.available_version : "-"}</strong></div>
            <div className="bg-surface px-5 py-4"><span className="text-[10px] font-bold uppercase text-muted-foreground">目标平台</span><strong className="mt-1 block truncate font-mono text-sm" title={status.target}>{status.target}</strong></div>
            <div className="bg-surface px-5 py-4"><span className="text-[10px] font-bold uppercase text-muted-foreground">上次检查</span><strong className="mt-1 block font-mono text-sm">{lastChecked}</strong></div>
          </div>
        )}

        {status?.phase === "downloading" && (
          <div className="border-b border-border px-5 py-4">
            <div className="mb-2 flex items-center justify-between text-xs"><span className="font-semibold text-foreground">正在下载并校验</span><span className="font-mono text-muted-foreground">{progress}%</span></div>
            <div className="h-2 overflow-hidden rounded-full bg-muted"><div className="h-full rounded-full bg-primary transition-[width] duration-300" style={{ width: progress + "%" }} /></div>
          </div>
        )}

        <div className="grid gap-4 p-5 lg:grid-cols-[1.2fr_.8fr]">
          <div className="rounded-md border border-border bg-muted/40 p-4">
            <div className="flex items-center gap-2 text-sm font-bold text-foreground"><ShieldCheck className="size-4 text-success" />更新保护</div>
            <p className="mt-2 text-xs leading-5 text-muted-foreground">{summary}</p>
            <div className="mt-4 grid gap-2 text-xs text-muted-foreground sm:grid-cols-2">
              <span className="flex items-center gap-2"><CheckCircle2 className="size-3.5 text-success" />SHA256 校验后替换</span>
              <span className="flex items-center gap-2"><CheckCircle2 className="size-3.5 text-success" />失败自动恢复旧版本</span>
              <span className="flex items-center gap-2"><CheckCircle2 className="size-3.5 text-success" />保留 config 与 plugins</span>
              <span className="flex items-center gap-2"><CheckCircle2 className="size-3.5 text-success" />健康端点确认启动</span>
            </div>
          </div>
          <div className="rounded-md border border-border p-4">
            <div className="flex items-center justify-between gap-3"><div><div className="text-sm font-bold text-foreground">重启运行时</div><p className="mt-1 text-xs text-muted-foreground">用于应用已保存的配置或主动恢复连接。</p></div><RotateCcw className="size-5 text-primary" /></div>
            <Button className="mt-4 w-full" variant="outline" onClick={() => void action("restart")} disabled={!canRestart || busy !== null}><RotateCcw />优雅重启</Button>
          </div>
        </div>

        {view?.deployment === "docker" && (
          <div className="border-t border-border bg-muted/30 px-5 py-4 text-xs text-muted-foreground">
            <div className="flex items-center gap-2 font-bold text-foreground"><Cloud className="size-4" />Docker Hub 更新命令</div>
            <code className="mt-3 block overflow-x-auto rounded-md bg-background px-3 py-2 font-mono text-[11px] text-foreground">docker compose pull qimenbot && docker compose up -d qimenbot</code>
          </div>
        )}
        {status?.release_url && status.phase === "available" && (
          <a className="flex items-center gap-1.5 border-t border-border px-5 py-3 text-xs font-semibold text-primary hover:underline" href={status.release_url} target="_blank" rel="noreferrer">查看 Release 说明 <ExternalLink className="size-3.5" /></a>
        )}
      </section>
    </main>
  )
}
