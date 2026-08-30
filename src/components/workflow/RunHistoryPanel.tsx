// 工作流运行历史面板：从 Workflow.tsx 抽出；列表页（折叠）与历史页（直接展开）共用
// Workflow run history panel: extracted from Workflow.tsx; shared by the list page
// (collapsed) and the dedicated history page (always expanded)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { History, ChevronDown, ChevronRight } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import type { Workflow, WorkflowRun, WorkflowStepLog } from "@/types";

// 单步日志行：节点 / 插件动作 / 输入截断 / 耗时 / 状态 / 失败错误
// One step log row: node / plugin.action / truncated input / duration / status / failure error
function StepRow({ step }: { step: WorkflowStepLog }) {
  const { t } = useTranslation()

  const statusVariant: 'default' | 'destructive' | 'secondary' =
    step.status === 'success'
      ? 'default'
      : step.status === 'failed'
        ? 'destructive'
        : 'secondary'

  return (
    <div className="grid grid-cols-[auto_1fr_1fr_auto] items-center gap-2 rounded border bg-card px-3 py-2 text-xs">
      {/* 节点标识 / Node identifier */}
      <span className="font-mono text-muted-foreground">{step.step_id}</span>

      {/* 插件 / action */}
      <span className="text-muted-foreground">
        {step.plugin_id && step.action ? (
          <span>
            {step.plugin_id}
            <span className="text-foreground">.{step.action}</span>
          </span>
        ) : (
          <span className="italic">—</span>
        )}
      </span>

      {/* 输入（截断 / truncated） */}
      <span
        className="max-w-48 truncate font-mono text-muted-foreground"
        title={step.input ?? ''}
      >
        {step.input
          ? (() => {
              try {
                const parsed = JSON.parse(step.input)
                return JSON.stringify(parsed).slice(0, 60)
              } catch {
                return step.input.slice(0, 60)
              }
            })()
          : '—'}
      </span>

      {/* 耗时 + 状态 / Duration + status */}
      <div className="flex items-center gap-2">
        {step.duration_ms != null && (
          <span className="text-muted-foreground">{step.duration_ms}ms</span>
        )}
        <Badge variant={statusVariant} className="font-normal">
          {t(`workflow.status.${step.status}` as const)}
        </Badge>
      </div>

      {/* 失败错误 / Failure error */}
      {step.status === 'failed' && step.error && (
        <div className="col-span-4 mt-1 rounded border border-destructive/30 bg-destructive/10 px-3 py-1.5 text-[11px] text-destructive">
          {step.error}
        </div>
      )}
    </div>
  )
}

// 运行历史列表 + 单条 run 的 step 详情（受控展开/收起）
// Run history list + step detail for the selected run (controlled expand/collapse)
function RunList({
  workflow,
  initiallyExpanded,
}: {
  workflow: Workflow
  initiallyExpanded: boolean
}) {
  const { t } = useTranslation()
  const { getRuns, getRunSteps } = useWorkflowsStore()
  const [runs, setRuns] = useState<WorkflowRun[]>([])
  const [loadingRuns, setLoadingRuns] = useState(false)
  const [selectedRun, setSelectedRun] = useState<WorkflowRun | null>(null)
  const [steps, setSteps] = useState<WorkflowStepLog[]>([])
  const [loadingSteps, setLoadingSteps] = useState(false)

  // 挂载即拉取最近 20 次运行（历史页用；列表页用折叠卡片的 lazy 加载）
  // Fetch the latest 20 runs on mount (used by the history page; the list page
  // uses the collapsed card with lazy loading)
  useEffect(() => {
    let cancelled = false
    setLoadingRuns(true)
    getRuns(workflow.id, 20)
      .then((r) => {
        if (!cancelled) setRuns(r)
      })
      .catch((e) => toast.error(String(e)))
      .finally(() => {
        if (!cancelled) setLoadingRuns(false)
      })
    return () => {
      cancelled = true
    }
  }, [workflow.id, getRuns])

  // 选中 run 后懒加载 step 详情
  // Lazy-load step detail when a run is selected
  useEffect(() => {
    if (!selectedRun) {
      setSteps([])
      return
    }
    let cancelled = false
    setLoadingSteps(true)
    getRunSteps(selectedRun.id)
      .then((s) => {
        if (!cancelled) setSteps(s)
      })
      .catch((e) => toast.error(String(e)))
      .finally(() => {
        if (!cancelled) setLoadingSteps(false)
      })
    return () => {
      cancelled = true
    }
  }, [selectedRun, getRunSteps])

  return (
    <>
      {loadingRuns ? (
        <Skeleton className="h-8 w-full rounded" />
      ) : runs.length === 0 ? (
        <p className="py-2 text-center text-xs text-muted-foreground">
          {t('workflow.history.empty')}
        </p>
      ) : (
        // run 列表：可点击展开 step
        // Run list: click to expand steps
        <div className="space-y-1.5">
          {runs.map((run) => (
            <div key={run.id}>
              <button
                className={`flex w-full items-center gap-2 rounded px-3 py-2 text-left text-xs transition-colors hover:bg-muted/60 ${
                  selectedRun?.id === run.id ? 'bg-muted' : ''
                }`}
                onClick={() =>
                  setSelectedRun((prev) => (prev?.id === run.id ? null : run))
                }
              >
                {selectedRun?.id === run.id ? (
                  <ChevronDown className="h-3 w-3 shrink-0 text-muted-foreground" />
                ) : (
                  <ChevronRight className="h-3 w-3 shrink-0 text-muted-foreground" />
                )}
                <span className="font-mono text-muted-foreground">#{run.id}</span>
                <span className="truncate">{run.started_at}</span>
                {run.trigger_type && (
                  <Badge variant="outline" className="shrink-0 font-normal">
                    {run.trigger_type}
                  </Badge>
                )}
                <Badge
                  variant={
                    run.status === 'success'
                      ? 'default'
                      : run.status === 'failed'
                        ? 'destructive'
                        : 'secondary'
                  }
                  className="ml-auto shrink-0 font-normal"
                >
                  {t(`workflow.status.${run.status}` as const)}
                </Badge>
              </button>

              {/* 选中的 run：step 详情 */}
              {/* Selected run: show step detail */}
              {selectedRun?.id === run.id && (
                <div className="ml-6 space-y-1.5 border-l-2 border-border pl-3">
                  {/* run 元信息 / Run metadata */}
                  <div className="mb-2 flex flex-wrap gap-x-4 gap-y-0.5 text-[11px] text-muted-foreground">
                    <span>
                      {t('workflow.history.triggerType')}: {run.trigger_type ?? '—'}
                    </span>
                    <span>
                      {t('workflow.history.startedAt')}: {run.started_at}
                    </span>
                    {run.finished_at && (
                      <span>
                        {t('workflow.history.finishedAt')}: {run.finished_at}
                      </span>
                    )}
                    {run.error && (
                      <span className="text-destructive">
                        {t('workflow.history.stepError')}: {run.error}
                      </span>
                    )}
                  </div>

                  {/* 列头 / Column header */}
                  <div className="grid grid-cols-[auto_1fr_1fr_auto] gap-2 px-3 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
                    <span>{t('workflow.history.step')}</span>
                    <span>plugin.action</span>
                    <span>{t('workflow.history.stepInput')}</span>
                    <span>{t('workflow.history.stepDuration')}</span>
                  </div>

                  {loadingSteps ? (
                    <Skeleton className="h-10 w-full rounded" />
                  ) : steps.length === 0 ? (
                    <p className="text-xs text-muted-foreground">—</p>
                  ) : (
                    steps.map((s) => <StepRow key={s.id} step={s} />)
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {/* 占位以满足 strict 模板规则（无选中 run 也不显示） */}
      {/* Suppress unused-import warning when no run is ever selected */}
      {initiallyExpanded ? null : null}
    </>
  )
}

// 折叠卡片模式（列表页使用）：默认收起，点击头部展开并懒加载
// Collapsed card mode (used by the list page): collapsed by default, lazy-loads on click
function CollapsibleCard({ workflow }: { workflow: Workflow }) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(false)

  return (
    <Card>
      <button
        className="flex w-full items-center justify-between px-4 py-3 text-left hover:bg-muted/50"
        onClick={() => setExpanded((v) => !v)}
        aria-expanded={expanded}
      >
        <span className="flex items-center gap-2 text-sm font-medium">
          <History className="h-4 w-4" />
          {t('workflow.history.title')}
        </span>
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronRight className="h-4 w-4 text-muted-foreground" />
        )}
      </button>

      {expanded && (
        <CardContent className="pb-4">
          <RunList workflow={workflow} initiallyExpanded={true} />
        </CardContent>
      )}
    </Card>
  )
}

// 整页展开模式（/workflow/:id/runs 使用）：标题 + 列表，无折叠头
// Always-expanded page mode (used by /workflow/:id/runs): title + list, no collapse header
function FullPanel({ workflow }: { workflow: Workflow }) {
  const { t } = useTranslation()
  return (
    <Card>
      <div className="flex items-center gap-2 border-b px-4 py-3 text-sm font-medium">
        <History className="h-4 w-4" />
        {t('workflow.history.title')}
      </div>
      <CardContent className="pt-4">
        <RunList workflow={workflow} initiallyExpanded={true} />
      </CardContent>
    </Card>
  )
}

// 入口：根据 mode 决定卡片或整页样式
// Entry: choose card or full panel by mode
export function RunHistoryPanel({
  workflow,
  mode = 'card',
}: {
  workflow: Workflow
  mode?: 'card' | 'full'
}) {
  return mode === 'full' ? (
    <FullPanel workflow={workflow} />
  ) : (
    <CollapsibleCard workflow={workflow} />
  )
}