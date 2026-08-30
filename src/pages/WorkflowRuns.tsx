// 工作流运行历史页面（/workflow/:id/runs）
// Workflow run history page (/workflow/:id/runs)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import { RunHistoryPanel } from "@/components/workflow/RunHistoryPanel";
import type { Workflow } from "@/types";

export default function WorkflowRuns() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { id } = useParams<{ id: string }>();

  const { workflows, loading, load } = useWorkflowsStore();
  const [wf, setWf] = useState<Workflow | null>(null);

  useEffect(() => {
    if (workflows.length === 0) {
      load().catch((e) => toast.error(String(e)));
    }
  }, []);

  useEffect(() => {
    const found = workflows.find((w) => w.id === id) ?? null;
    setWf(found);
  }, [workflows, id]);

  if (loading || workflows.length === 0) {
    return (
      <div className="flex h-48 items-center justify-center text-muted-foreground">
        {t("workflow.loading")}
      </div>
    );
  }

  if (!wf) {
    return (
      <div className="rounded-xl border border-dashed p-10 text-center">
        <p className="font-medium text-destructive">{t("workflowRuns.notFound")}</p>
        <Button className="mt-4" variant="outline" onClick={() => navigate("/workflow")}>
          <ArrowLeft />
          {t("workflowRuns.backToList")}
        </Button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* 顶部导航 */}
      {/* Top navigation */}
      <div className="flex items-center gap-3">
        <Button variant="ghost" size="icon" onClick={() => navigate("/workflow")}>
          <ArrowLeft />
        </Button>
        <h2 className="flex-1 font-semibold">{wf.name}</h2>
        <Button
          variant="outline"
          size="sm"
          onClick={() => navigate(`/workflow/${id}/edit`)}
        >
          <Pencil className="h-3 w-3" />
          {t("workflowRuns.backToEditor")}
        </Button>
      </div>

      {/* 运行历史面板（整页模式） */}
      {/* Run history panel (full page mode) */}
      <RunHistoryPanel workflow={wf} mode="full" />
    </div>
  );
}