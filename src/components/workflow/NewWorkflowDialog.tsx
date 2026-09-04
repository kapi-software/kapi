// 新建工作流 Dialog：选模板 → 填名称 → 跳转编辑器
// New workflow dialog: pick template → fill name → navigate to editor
// 实现 P6 三步向导：第一步选模板（每张卡片含名称+描述），第二步填名称/描述
// P6 three-step wizard: step 1 pick template, step 2 fill name/description
import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { v4 as uuidv4 } from "uuid";
import { Check, ChevronRight, Sparkles } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card } from "@/components/ui/card";
import { WORKFLOW_TEMPLATES, getTemplate, type WorkflowTemplate } from "@/lib/workflow-templates";
import type { WorkflowGraph } from "@/types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function NewWorkflowDialog({ open, onOpenChange }: Props) {
  const { t } = useTranslation();
  const navigate = useNavigate();
  // 步骤：1=选模板，2=填名称
  // Step 1 pick template, 2 fill name
  const [step, setStep] = useState<1 | 2>(1);
  const [templateId, setTemplateId] = useState<string | null>("blank");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);

  // 选中的模板（用于预填 graph + 校验）
  const selectedTemplate = useMemo<WorkflowTemplate | null>(
    () => getTemplate(templateId),
    [templateId],
  );

  const reset = () => {
    setStep(1);
    setTemplateId("blank");
    setName("");
    setDescription("");
  };

  const handleCreate = () => {
    if (!name.trim()) return;
    setSaving(true);
    // 模板 graph 通过 search params 传给编辑器
    // Template graph travels to the editor via search params
    const id = `wf-${uuidv4().replace(/-/g, "")}`;
    const graphJson = JSON.stringify(selectedTemplate?.graph ?? { nodes: [], edges: [] } as WorkflowGraph);
    const params = new URLSearchParams({
      name: name.trim(),
      description: description.trim(),
      template: templateId ?? "blank",
      graph: graphJson,
    });
    navigate(`/workflow/${id}/edit?${params.toString()}`);
    reset();
    setSaving(false);
    onOpenChange(false);
  };

  const handleOpenChange = (next: boolean) => {
    if (!next) reset();
    onOpenChange(next);
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Sparkles className="size-4 text-primary" />
            {step === 1 ? t("workflow.new.step1Title", "选择模板") : t("workflow.new.step2Title", "填名称")}
          </DialogTitle>
        </DialogHeader>

        {step === 1 ? (
          // 步骤 1：选模板 / Step 1: pick template
          <div className="space-y-3 py-2">
            <p className="text-xs text-muted-foreground">
              {t("workflow.new.step1Hint", "模板会预填画布和触发器，进入编辑器后仍可自由修改")}
            </p>
            <div className="grid gap-2 sm:grid-cols-2">
              {WORKFLOW_TEMPLATES.map((tpl) => {
                const selected = templateId === tpl.id;
                return (
                  <Card
                    key={tpl.id}
                    className={`cursor-pointer p-3 transition-colors ${
                      selected
                        ? "border-primary ring-1 ring-primary/40 bg-primary/5"
                        : "hover:border-primary/40"
                    }`}
                    onClick={() => setTemplateId(tpl.id)}
                    role="button"
                    tabIndex={0}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        setTemplateId(tpl.id);
                      }
                    }}
                  >
                    <div className="flex items-start justify-between gap-1">
                      <h4 className="text-sm font-medium">
                        {tpl.i18n?.nameKey ? t(tpl.i18n.nameKey, tpl.name) : tpl.name}
                      </h4>
                      {selected && <Check className="size-4 shrink-0 text-primary" />}
                    </div>
                    <p className="mt-1 line-clamp-2 text-[11px] text-muted-foreground">
                      {tpl.i18n?.descriptionKey
                        ? t(tpl.i18n.descriptionKey, tpl.description)
                        : tpl.description}
                    </p>
                    <p className="mt-1 text-[10px] text-muted-foreground/70">
                      {tpl.requires.length === 0
                        ? t("workflow.new.noPluginRequired", "无需额外插件")
                        : `${tpl.requires.length} 个插件`}
                    </p>
                  </Card>
                );
              })}
            </div>
          </div>
        ) : (
          // 步骤 2：填名称 / Step 2: fill name
          <div className="space-y-4 py-2">
            <div className="rounded-md border bg-muted/40 p-2 text-xs">
              <span className="text-muted-foreground">模板：</span>
              <span className="ml-1 font-medium">
                {selectedTemplate?.i18n?.nameKey
                  ? t(selectedTemplate.i18n.nameKey, selectedTemplate.name)
                  : selectedTemplate?.name}
              </span>
              <Button
                variant="link"
                size="sm"
                className="ml-2 h-5 px-1 text-[10px]"
                onClick={() => setStep(1)}
              >
                {t("common.back", "返回")}
              </Button>
            </div>
            <div className="space-y-2">
              <Label htmlFor="wf-name">
                {t("workflowEditor.name")} <span className="text-destructive">*</span>
              </Label>
              <Input
                id="wf-name"
                placeholder={t("workflowEditor.namePlaceholder")}
                value={name}
                onChange={(e) => setName(e.target.value)}
                maxLength={20}
                autoFocus
                onKeyDown={(e) => {
                  if (e.key === "Enter" && name.trim()) handleCreate();
                }}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="wf-desc">{t("workflowEditor.description")}</Label>
              <Input
                id="wf-desc"
                placeholder={t("workflowEditor.descriptionPlaceholder")}
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>
          </div>
        )}

        <DialogFooter>
          {step === 1 ? (
            <>
              <Button variant="outline" onClick={() => handleOpenChange(false)}>
                {t("common.cancel")}
              </Button>
              <Button onClick={() => setStep(2)} disabled={!templateId}>
                {t("common.next", "下一步")}
                <ChevronRight className="ml-1 size-3.5" />
              </Button>
            </>
          ) : (
            <>
              <Button variant="outline" onClick={() => setStep(1)}>
                {t("common.back", "返回")}
              </Button>
              <Button onClick={handleCreate} disabled={!name.trim() || saving}>
                {saving ? t("workflow.saving") : t("workflow.new")}
              </Button>
            </>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
