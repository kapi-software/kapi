// 工作流管理页：列表 + 启停 + 运行；新建走 Dialog（填名称）→ 跳编辑器；删除走 ConfirmDialog
// Workflow list: enable toggle + run; new opens a Dialog (name) then the editor;
// delete uses ConfirmDialog (no toast action prompts)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { Eye, History, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import { isTauri } from "@/lib/tauri";
import type { Workflow, WorkflowRun, WorkflowTrigger } from "@/types";
import { NewWorkflowDialog } from "@/components/workflow/NewWorkflowDialog";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";
import { CardDetailDialog } from "@/components/common/CardDetailDialog";
import { TriggerDialog } from "@/components/workflow/TriggerDialog";
import { TriggerListPanel } from "@/components/workflow/TriggerListPanel";

// 运行状态 → i18n key 映射
function statusKey(s: string): string {
  return `workflow.status.${s}` as const;
}

// 单行字段标签
function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[auto_1fr] items-baseline gap-3">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <span className="break-all">{children}</span>
    </div>
  );
}

// 单张工作流卡片：信息 + 启停 + 详情；操作按钮全在卡片底部
// One workflow card: info + enable + detail; all action buttons live at the bottom
function WorkflowCard({
  workflow,
  latestRun,
  onRun,
  onEdit,
  onHistory,
  onDelete,
  onToggle,
  onTriggerEdit,
  busy,
}: {
  workflow: Workflow;
  latestRun: WorkflowRun | null;
  onRun: (w: Workflow) => void;
  onEdit: (w: Workflow) => void;
  onHistory: (w: Workflow) => void;
  onDelete: (w: Workflow) => void;
  onToggle: (w: Workflow, enabled: boolean) => void;
  onTriggerEdit: (t: WorkflowTrigger) => void;
  busy: boolean;
}) {
  const { t } = useTranslation();
  const [detailOpen, setDetailOpen] = useState(false);

  const nodeCount = workflow.graph.nodes.length;
  const edgeCount = workflow.graph.edges.length;
  const bindingCount = workflow.graph.bindings.length;

  return (
    <>
      <Card
        className={`flex h-full flex-col gap-3 p-4 ${
          workflow.is_enabled ? undefined : "opacity-60"
        }`}
      >
        {/* 顶部：标题 + 状态徽章 + 详情按钮 */}
        {/* Top: title + status badges + detail button */}
        <div className="space-y-1.5">
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
            <Button
              variant="ghost"
              size="icon"
              className="ml-auto h-6 w-6"
              onClick={() => setDetailOpen(true)}
              aria-label={t("plugins.viewDetail")}
              title={t("plugins.viewDetail")}
            >
              <Eye className="size-3.5" />
            </Button>
          </div>
          {workflow.description && (
            <p className="line-clamp-2 text-xs text-muted-foreground">
              {workflow.description}
            </p>
          )}
          <p className="text-[10px] text-muted-foreground/70">
            {t("workflow.lastRun")}:{" "}
            {latestRun
              ? `${new Date(latestRun.started_at).toLocaleString()} · ${t(statusKey(latestRun.status))}`
              : t("workflow.neverRun")}
            {" · "}
            {t("workflow.nodes")}: {nodeCount} · {t("workflow.edges")}: {edgeCount} ·{" "}
            {t("workflow.bindings")}: {bindingCount}
          </p>
        </div>

        {/* 触发器面板 */}
        {/* Triggers panel */}
        <div className="rounded-md border border-dashed bg-muted/30 p-2">
          <TriggerListPanel workflowId={workflow.id} onEdit={onTriggerEdit} />
        </div>

        {/* 底部：启停 Switch + 全部操作按钮（运行/历史/编辑/删除） */}
        {/* Bottom: enable switch + all action buttons (run / history / edit / delete) */}
        <div className="mt-auto flex items-center justify-between gap-2 pt-1">
          <Switch
            checked={workflow.is_enabled}
            onCheckedChange={(v) => onToggle(workflow, v)}
            aria-label={t("workflow.enable")}
          />
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
              onClick={() => onHistory(workflow)}
              aria-label={t("workflow.history.title")}
              title={t("workflow.history.title")}
            >
              <History />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              onClick={() => onEdit(workflow)}
              aria-label={t("workflow.edit")}
              title={t("workflow.edit")}
            >
              <Pencil />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="text-destructive hover:text-destructive"
              onClick={() => onDelete(workflow)}
              aria-label={t("common.delete")}
              title={t("common.delete")}
            >
              <Trash2 />
            </Button>
          </div>
        </div>
      </Card>

      <CardDetailDialog
        open={detailOpen}
        onOpenChange={setDetailOpen}
        title={workflow.name}
        description={workflow.id}
      >
        <div className="space-y-3 text-sm">
          <Field label={t("workflow.fieldId")}>{workflow.id}</Field>
          <Field label={t("workflow.fieldName")}>{workflow.name}</Field>
          <Field label={t("workflow.fieldDescription")}>
            {workflow.description ?? "—"}
          </Field>
          <Field label={t("workflow.fieldEnabled")}>
            {workflow.is_enabled ? "✓" : "✗"}
          </Field>
          <Field label={t("workflow.fieldCreatedAt")}>
            {new Date(workflow.created_at).toLocaleString() || "—"}
          </Field>
          <Field label={t("workflow.fieldUpdatedAt")}>
            {workflow.updated_at || "—"}
          </Field>
          <Field label={t("workflow.fieldNodes")}>
            {workflow.graph.nodes.length}
          </Field>
          <Field label={t("workflow.fieldEdges")}>
            {workflow.graph.edges.length}
          </Field>
          <Field label={t("workflow.fieldBindings")}>
            {workflow.graph.bindings.length}
          </Field>
          {latestRun && (
            <>
              <Field label={t("workflow.fieldLastRun")}>
                {new Date(latestRun.started_at).toLocaleString()}
              </Field>
              <Field label={t("workflow.fieldLastRunStatus")}>
                {t(statusKey(latestRun.status))}
              </Field>
            </>
          )}
        </div>
      </CardDetailDialog>
    </>
  );
}

export default function Workflow() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { workflows, loading, load, save, remove, run, getRuns } = useWorkflowsStore();
  const [latestRuns, setLatestRuns] = useState<Record<string, WorkflowRun>>({});
  const [busyId, setBusyId] = useState<string | null>(null);
  const [newOpen, setNewOpen] = useState(false);
  const [pendingDelete, setPendingDelete] = useState<Workflow | null>(null);
  const [deleting, setDeleting] = useState(false);
  // 触发器 Dialog 状态
  // Trigger dialog state
  const [triggerDialogOpen, setTriggerDialogOpen] = useState(false);
  const [triggerTarget, setTriggerTarget] = useState<Workflow | null>(null);
  // 编辑中的 trigger；null = 新建，否则为编辑
  // The trigger being edited; null = new, otherwise edit
  const [editingTrigger, setEditingTrigger] = useState<WorkflowTrigger | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    load().catch((e) => toast.error(String(e)));
  }, [load]);

  useEffect(() => {
    if (!isTauri() || workflows.length === 0) return;
    let cancelled = false;
    Promise.all(
      workflows.map(async (w) => {
        try {
          const runs = await getRuns(w.id, 1);
          return [w.id, runs[0] ?? null] as const;
        } catch {
          return [w.id, null] as const;
        }
      }),
    ).then((entries) => {
      if (cancelled) return;
      const next: Record<string, WorkflowRun> = {};
      for (const [id, r] of entries) {
        if (r) next[id] = r;
      }
      setLatestRuns(next);
    });
    return () => {
      cancelled = true;
    };
  }, [workflows, getRuns]);

  const handleRun = async (w: Workflow) => {
    setBusyId(w.id);
    try {
      const runRow = await run(w.id);
      setLatestRuns((prev) => ({ ...prev, [w.id]: runRow }));
      const duration =
        runRow.finished_at && runRow.started_at
          ? Math.max(
              0,
              new Date(runRow.finished_at).getTime() -
                new Date(runRow.started_at).getTime(),
            )
          : 0;
      if (runRow.status === "success") {
        toast.success(t("workflow.toastRunSuccess", { name: w.name, duration }));
      } else {
        toast.error(
          t("workflow.toastRunFailed", {
            name: w.name,
            error: runRow.error ?? "unknown",
          }),
        );
      }
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const handleDelete = (w: Workflow) => {
    setPendingDelete(w);
  };

  const confirmDelete = async () => {
    if (!pendingDelete) return;
    setDeleting(true);
    try {
      await remove(pendingDelete.id);
      toast.success(t("workflow.toastDeleted", { name: pendingDelete.name }));
      setPendingDelete(null);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setDeleting(false);
    }
  };

  const handleToggle = async (w: Workflow, enabled: boolean) => {
    try {
      await save({ ...w, is_enabled: enabled });
    } catch (e) {
      toast.error(String(e));
    }
  };

  const handleEdit = (w: Workflow) => navigate(`/workflow/${w.id}/edit`);
  const handleHistory = (w: Workflow) => navigate(`/workflow/${w.id}/runs`);
  const handleTriggerEdit = (t: WorkflowTrigger) => {
    // 找到所属工作流（触发器列表是按 workflow 拉的，但 onEdit 只拿到 trigger）
    // Find owning workflow (the trigger list is loaded per workflow, but onEdit only sees the trigger)
    const wf = workflows.find((w) => w.id === t.workflow_id) ?? null;
    setTriggerTarget(wf);
    setEditingTrigger(t);
    setTriggerDialogOpen(true);
  };

  if (!isTauri()) {
    return (
      <div>
        <h1 className="text-2xl font-bold">{t("workflow.title")}</h1>
        <p className="mt-2 text-sm text-muted-foreground">{t("workflow.browserHint")}</p>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 工具栏：新建放左 / Toolbar: new on the left */}
      <div className="flex items-center gap-2">
        <Button onClick={() => setNewOpen(true)}>
          <Plus />
          {t("workflow.new")}
        </Button>
      </div>

      {loading ? (
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
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
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          {workflows.map((w) => (
            <WorkflowCard
              key={w.id}
              workflow={w}
              latestRun={latestRuns[w.id] ?? null}
              onRun={handleRun}
              onEdit={handleEdit}
              onHistory={handleHistory}
              onDelete={handleDelete}
              onToggle={handleToggle}
              onTriggerEdit={handleTriggerEdit}
              busy={busyId === w.id}
            />
          ))}
        </div>
      )}

      <NewWorkflowDialog open={newOpen} onOpenChange={setNewOpen} />

      {triggerTarget && (
        <TriggerDialog
          open={triggerDialogOpen}
          onOpenChange={setTriggerDialogOpen}
          workflowId={triggerTarget.id}
          trigger={editingTrigger}
        />
      )}

      <ConfirmDialog
        open={!!pendingDelete}
        onOpenChange={(o) => {
          if (!o) setPendingDelete(null);
        }}
        title={t("workflowRuns.deleteConfirmTitle")}
        description={
          pendingDelete
            ? t("workflowRuns.deleteConfirmDescription", { name: pendingDelete.name })
            : ""
        }
        confirmLabel={t("common.delete")}
        destructive
        onConfirm={confirmDelete}
        busy={deleting}
      />
    </div>
  );
}