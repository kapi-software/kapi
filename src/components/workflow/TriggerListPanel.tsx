// 工作流触发器列表面板
// Workflow trigger list panel
import { useEffect, useState } from "react";
import { Pencil, Plus, Trash2, Zap } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { useTriggersStore } from "@/stores/triggers";
import type { WorkflowTrigger, TriggerType } from "@/types";

const TRIGGER_LABELS: Record<TriggerType, string> = {
  manual: "手动",
  schedule: "定时",
  plugin_event: "插件事件",
  clipboard: "剪贴板",
  hotkey: "快捷键",
};

const TRIGGER_COLORS: Record<TriggerType, string> = {
  manual: "bg-gray-100 text-gray-700",
  schedule: "bg-blue-100 text-blue-700",
  plugin_event: "bg-purple-100 text-purple-700",
  clipboard: "bg-yellow-100 text-yellow-700",
  hotkey: "bg-green-100 text-green-700",
};

interface Props {
  workflowId: string;
  onEdit: (t: WorkflowTrigger) => void;
}

export function TriggerListPanel({ workflowId, onEdit }: Props) {
  const { triggers, load, remove } = useTriggersStore();
  const [pendingDelete, setPendingDelete] = useState<WorkflowTrigger | null>(null);
  const [deleting, setDeleting] = useState(false);

  useEffect(() => {
    load(workflowId).catch(console.error);
  }, [workflowId, load]);

  const handleDelete = async () => {
    if (!pendingDelete) return;
    setDeleting(true);
    try {
      await remove(pendingDelete.id);
      setPendingDelete(null);
    } catch (e) {
      console.error("Failed to delete trigger:", e);
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex w-full items-center justify-between">
        <div className="flex items-center gap-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
          <Zap className="size-3" />
          触发器
        </div>
        <Button
          size="sm"
          variant="ghost"
          className="h-6 px-2 text-xs"
          onClick={() => onEdit({
            id: "",
            workflow_id: workflowId,
            trigger_type: "schedule",
            config: { interval_seconds: 60 },
            is_enabled: true,
          })}
        >
          <Plus className="size-3" />
          新建
        </Button>
      </div>

      {triggers.length === 0 ? (
        <p className="text-[10px] text-muted-foreground/70">无触发器</p>
      ) : (
        <div className="w-full space-y-1">
          {triggers.map((t) => (
            <div
              key={t.id}
              className="flex w-full items-center gap-2 rounded border bg-background px-2 py-1"
            >
              <Badge
                className={`shrink-0 text-[9px] ${TRIGGER_COLORS[t.trigger_type] ?? ""}`}
                variant="outline"
              >
                {TRIGGER_LABELS[t.trigger_type] ?? t.trigger_type}
              </Badge>
              <span className="min-w-0 flex-1 truncate font-mono text-[10px] text-muted-foreground">
                {JSON.stringify(t.config)}
              </span>
              {!t.is_enabled && (
                <span className="shrink-0 text-[9px] text-muted-foreground">已停用</span>
              )}
              <Button
                size="icon"
                variant="ghost"
                className="h-5 w-5 shrink-0"
                onClick={() => onEdit(t)}
                title="编辑"
              >
                <Pencil className="size-3" />
              </Button>
              <Button
                size="icon"
                variant="ghost"
                className="h-5 w-5 shrink-0 text-destructive hover:text-destructive"
                onClick={() => setPendingDelete(t)}
                title="删除"
              >
                <Trash2 className="size-3" />
              </Button>
            </div>
          ))}
        </div>
      )}

      {/* 删除确认 */}
      {pendingDelete && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
          <div className="w-80 space-y-3 rounded-lg border bg-card p-4">
            <p className="text-sm font-medium">删除触发器？</p>
            <p className="text-xs text-muted-foreground">
              {TRIGGER_LABELS[pendingDelete.trigger_type]} - {JSON.stringify(pendingDelete.config)}
            </p>
            <div className="flex justify-end gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPendingDelete(null)}
              >
                取消
              </Button>
              <Button
                size="sm"
                variant="destructive"
                onClick={handleDelete}
                disabled={deleting}
              >
                {deleting ? "删除中..." : "删除"}
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
