// 工作流管理页：列表 + 手动运行 + 内嵌编辑（v1 纯 JSON 编辑 graph）
// Workflow manager: list + manual run + inline editor (v1 edits graph JSON)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Play, Trash2, Save, X, Pencil } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import { isTauri } from "@/lib/tauri";
import type { Workflow, WorkflowRun } from "@/types";

// 空 graph 模板：单节点 + 空 edges/bindings，新工作流从这里起步
// Empty graph template: one node + empty edges/bindings; new workflows start from here
const EMPTY_GRAPH = JSON.stringify(
  {
    nodes: [
      { id: "n1", type: "plugin", plugin_id: "", action: "" },
    ],
    edges: [],
    bindings: [],
  },
  null,
  2,
);

// 运行状态 → i18n key 映射（status.running/success/failed/cancelled）
// Run status → i18n key (status.running/success/failed/cancelled)
function statusKey(s: string): string {
  return `workflow.status.${s}` as const
}

// 单张工作流卡片：信息 + 启停 + 运行 + 编辑/删除
// One workflow card: info + enable toggle + run + edit/delete
function WorkflowCard({
  workflow,
  latestRun,
  onRun,
  onEdit,
  onDelete,
  onToggle,
  busy,
}: {
  workflow: Workflow
  latestRun: WorkflowRun | null
  onRun: (w: Workflow) => void
  onEdit: (w: Workflow) => void
  onDelete: (w: Workflow) => void
  onToggle: (w: Workflow, enabled: boolean) => void
  busy: boolean
}) {
  const { t } = useTranslation()

  const nodeCount = workflow.graph.nodes.length
  const edgeCount = workflow.graph.edges.length
  const bindingCount = workflow.graph.bindings.length

  return (
    <Card className={workflow.is_enabled ? undefined : "opacity-60"}>
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="truncate font-semibold">{workflow.name}</h3>
              {!workflow.is_enabled && (
                <Badge variant="outline">{t("workflow.disabled")}</Badge>
              )}
              {latestRun && (
                <Badge
                  variant={latestRun.status === "success" ? "default" : "destructive"}
                >
                  {t(statusKey(latestRun.status))}
                </Badge>
              )}
            </div>
            {workflow.description && (
              <p className="mt-1 line-clamp-2 text-xs text-muted-foreground">
                {workflow.description}
              </p>
            )}
            <p className="mt-1 text-[10px] text-muted-foreground/70">
              {t("workflow.nodes")}: {nodeCount} · {t("workflow.edges")}: {edgeCount} ·{" "}
              {t("workflow.bindings")}: {bindingCount}
            </p>
          </div>
          <Switch
            checked={workflow.is_enabled}
            onCheckedChange={(v) => onToggle(workflow, v)}
            aria-label={t("workflow.enable")}
          />
        </div>
      </CardHeader>
      <CardContent className="flex items-center justify-between gap-2 pt-0">
        <div className="text-[10px] text-muted-foreground">
          {t("workflow.lastRun")}:{" "}
          {latestRun
            ? `${latestRun.started_at} · ${t(statusKey(latestRun.status))}`
            : t("workflow.neverRun")}
        </div>
        <div className="flex items-center gap-1">
          <Button
            size="sm"
            variant="outline"
            disabled={busy || !workflow.is_enabled}
            onClick={() => onRun(workflow)}
          >
            <Play />
            {busy ? t("workflow.running") : t("workflow.run")}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => onEdit(workflow)}
            aria-label={t("workflow.edit")}
          >
            <Pencil />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="text-destructive hover:text-destructive"
            onClick={() => onDelete(workflow)}
            aria-label={t("workflow.delete")}
          >
            <Trash2 />
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

// 内嵌编辑器：名称 + 描述 + graph JSON（v1 直编；Phase 7 切到 React Flow）
// Inline editor: name + description + graph JSON (v1 direct edit; Phase 7 swaps in React Flow)
function WorkflowEditor({
  initial,
  onSave,
  onCancel,
}: {
  initial: Workflow
  onSave: (w: Workflow) => void
  onCancel: () => void
}) {
  const { t } = useTranslation()
  const [name, setName] = useState(initial.name)
  const [description, setDescription] = useState(initial.description ?? "")
  const [graphText, setGraphText] = useState(JSON.stringify(initial.graph, null, 2))

  const handleSave = () => {
    if (!name.trim()) {
      toast.error("workflow.dialog.name 不能为空")
      return
    }
    let parsed: unknown
    try {
      parsed = JSON.parse(graphText)
    } catch (e) {
      toast.error(`graph JSON 解析失败：${String(e)}`)
      return
    }
    onSave({
      ...initial,
      name: name.trim(),
      description: description.trim() || null,
      graph: parsed as Workflow["graph"],
    })
  }

  return (
    <Card>
      <CardHeader className="pb-3">
        <h3 className="font-semibold">
          {initial.id
            ? t("workflow.dialog.titleEdit")
            : t("workflow.dialog.titleNew")}
        </h3>
      </CardHeader>
      <CardContent className="space-y-3">
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">
            {t("workflow.dialog.name")}
          </label>
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("workflow.dialog.namePlaceholder")}
          />
        </div>
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">
            {t("workflow.dialog.description")}
          </label>
          <Input
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={t("workflow.dialog.descriptionPlaceholder")}
          />
        </div>
        <div className="space-y-1">
          <label className="text-xs text-muted-foreground">
            {t("workflow.dialog.graphJson")}
          </label>
          <textarea
            value={graphText}
            onChange={(e) => setGraphText(e.target.value)}
            rows={10}
            className="w-full rounded-md border bg-background px-3 py-2 font-mono text-xs"
            spellCheck={false}
          />
          <p className="text-[10px] text-muted-foreground/70">
            {t("workflow.dialog.graphHint")}
          </p>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onCancel}>
            <X />
            {t("workflow.dialog.cancel")}
          </Button>
          <Button onClick={handleSave}>
            <Save />
            {t("workflow.dialog.save")}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

// 生成新工作流的 id（前端时间戳 + 随机后缀；落库后引擎接管）
// Generate a new workflow id (timestamp + random suffix; the engine takes over after save)
function newId(): string {
  return `wf-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`
}

// 空工作流骨架：仅 id/name/graph 三项，其余取默认
// Empty workflow skeleton: only id/name/graph; the rest falls back to defaults
function newWorkflow(): Workflow {
  return {
    id: newId(),
    name: "",
    description: null,
    graph: JSON.parse(EMPTY_GRAPH),
    is_enabled: true,
    created_at: "",
    updated_at: "",
  }
}

export default function Workflow() {
  const { t } = useTranslation()
  const { workflows, loading, load, save, remove, run, getRuns } =
    useWorkflowsStore()
  const [editing, setEditing] = useState<Workflow | null>(null)
  // 每张工作流最近一次运行（按 workflow_id → run），key 为组件局部状态
  // Latest run per workflow (workflow_id → run), keyed by the component-local state
  const [latestRuns, setLatestRuns] = useState<Record<string, WorkflowRun>>({})
  const [busyId, setBusyId] = useState<string | null>(null)

  useEffect(() => {
    if (!isTauri()) return
    load().catch((e) => toast.error(String(e)))
  }, [load])

  // 列表拉取后顺手取每个工作流的最近一次运行（用于卡片状态徽章）
  // After the list loads, also fetch each workflow's latest run (for the status badge)
  useEffect(() => {
    if (!isTauri() || workflows.length === 0) return
    let cancelled = false
    Promise.all(
      workflows.map(async (w) => {
        try {
          const runs = await getRuns(w.id, 1)
          return [w.id, runs[0] ?? null] as const
        } catch {
          return [w.id, null] as const
        }
      })
    ).then((entries) => {
      if (cancelled) return
      const next: Record<string, WorkflowRun> = {}
      for (const [id, r] of entries) {
        if (r) next[id] = r
      }
      setLatestRuns(next)
    })
    return () => {
      cancelled = true
    }
  }, [workflows, getRuns])

  const handleRun = async (w: Workflow) => {
    setBusyId(w.id)
    try {
      const runRow = await run(w.id)
      setLatestRuns((prev) => ({ ...prev, [w.id]: runRow }))
      const duration =
        runRow.finished_at && runRow.started_at
          ? Math.max(
              0,
              new Date(runRow.finished_at).getTime() -
                new Date(runRow.started_at).getTime(),
            )
          : 0
      if (runRow.status === "success") {
        toast.success(t("workflow.toastRunSuccess", { name: w.name, duration }))
      } else {
        toast.error(
          t("workflow.toastRunFailed", {
            name: w.name,
            error: runRow.error ?? "unknown",
          }),
        )
      }
    } catch (e) {
      toast.error(String(e))
    } finally {
      setBusyId(null)
    }
  }

  const handleDelete = (w: Workflow) => {
    toast(t("workflow.deleteConfirm", { name: w.name }), {
      action: {
        label: t("workflow.delete"),
        onClick: () =>
          remove(w.id)
            .then(() => toast.success(t("workflow.toastDeleted", { name: w.name })))
            .catch((e) => toast.error(String(e))),
      },
    })
  }

  const handleToggle = async (w: Workflow, enabled: boolean) => {
    try {
      await save({ ...w, is_enabled: enabled })
    } catch (e) {
      toast.error(String(e))
    }
  }

  const handleSave = async (w: Workflow) => {
    const isNew = workflows.every((x) => x.id !== w.id)
    try {
      await save(w)
      setEditing(null)
      toast.success(
        isNew
          ? t("workflow.toastCreated", { name: w.name })
          : t("workflow.toastSaved", { name: w.name }),
      )
    } catch (e) {
      toast.error(String(e))
    }
  }

  if (!isTauri()) {
    return (
      <div className="mx-auto max-w-3xl">
        <h1 className="text-2xl font-bold">{t("workflow.title")}</h1>
        <p className="mt-2 text-sm text-muted-foreground">{t("workflow.browserHint")}</p>
      </div>
    )
  }

  return (
    <div className="mx-auto max-w-4xl space-y-4">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold">{t("workflow.title")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("workflow.desc")}</p>
        </div>
        <Button onClick={() => setEditing(newWorkflow())}>
          <Plus />
          {t("workflow.new")}
        </Button>
      </div>

      {editing && (
        <WorkflowEditor
          initial={editing}
          onSave={handleSave}
          onCancel={() => setEditing(null)}
        />
      )}

      {loading ? (
        <div className="grid gap-4 md:grid-cols-2">
          <Skeleton className="h-32 rounded-xl" />
          <Skeleton className="h-32 rounded-xl" />
        </div>
      ) : workflows.length === 0 ? (
        <div className="rounded-xl border border-dashed p-10 text-center">
          <p className="font-medium">{t("workflow.empty")}</p>
          <p className="mt-1 text-sm text-muted-foreground">
            {t("workflow.emptyHint")}
          </p>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {workflows.map((w) => (
            <WorkflowCard
              key={w.id}
              workflow={w}
              latestRun={latestRuns[w.id] ?? null}
              onRun={handleRun}
              onEdit={(x) => setEditing(x)}
              onDelete={handleDelete}
              onToggle={handleToggle}
              busy={busyId === w.id}
            />
          ))}
        </div>
      )}
    </div>
  )
}