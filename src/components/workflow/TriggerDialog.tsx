// 工作流触发器配置 Dialog
// Workflow trigger configuration dialog
import { useState, useEffect } from "react";
import { v4 as uuidv4 } from "uuid";
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
import { useTriggersStore } from "@/stores/triggers";
import { usePluginsStore } from "@/stores/plugins";
import type { WorkflowTrigger, TriggerType } from "@/types";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workflowId: string;
  trigger?: WorkflowTrigger | null;
}

const TRIGGER_LABELS: Record<TriggerType, string> = {
  manual: "手动",
  schedule: "定时",
  plugin_event: "插件事件",
  clipboard: "剪贴板",
  hotkey: "快捷键",
};

export function TriggerDialog({ open, onOpenChange, workflowId, trigger }: Props) {
  const { save } = useTriggersStore();
  const { plugins } = usePluginsStore();
  const [triggerType, setTriggerType] = useState<TriggerType>("schedule");
  const [isEnabled, setIsEnabled] = useState(true);
  // 各类型配置
  const [intervalSeconds, setIntervalSeconds] = useState(60);
  const [eventType, setEventType] = useState("");
  const [pattern, setPattern] = useState("");
  const [shortcut, setShortcut] = useState("CmdOrCtrl+Shift+K");
  const [saving, setSaving] = useState(false);

  // 初始化
  useEffect(() => {
    if (trigger) {
      setTriggerType(trigger.trigger_type);
      setIsEnabled(trigger.is_enabled);
      const cfg = trigger.config as Record<string, unknown>;
      if (trigger.trigger_type === "schedule") {
        setIntervalSeconds((cfg.interval_seconds as number) ?? 60);
      } else if (trigger.trigger_type === "plugin_event") {
        setEventType((cfg.event_type as string) ?? "");
      } else if (trigger.trigger_type === "clipboard") {
        setPattern((cfg.pattern as string) ?? "");
      } else if (trigger.trigger_type === "hotkey") {
        setShortcut((cfg.shortcut as string) ?? "CmdOrCtrl+Shift+K");
      }
    } else {
      setTriggerType("schedule");
      setIsEnabled(true);
      setIntervalSeconds(60);
      setEventType("");
      setPattern("");
      setShortcut("CmdOrCtrl+Shift+K");
    }
  }, [trigger, open]);

  // 收集所有可用事件类型
  const allEventTypes = plugins.flatMap((p) => p.manifest?.workflow?.events ?? []);

  const handleSave = async () => {
    setSaving(true);
    try {
      let config: Record<string, unknown> = {};
      if (triggerType === "schedule") {
        config = { interval_seconds: intervalSeconds };
      } else if (triggerType === "plugin_event") {
        config = { event_type: eventType };
      } else if (triggerType === "clipboard") {
        config = pattern ? { pattern } : {};
      } else if (triggerType === "hotkey") {
        config = { shortcut };
      }

      const newTrigger: WorkflowTrigger = {
        id: trigger?.id ?? `tr-${uuidv4().replace(/-/g, "")}`,
        workflow_id: workflowId,
        trigger_type: triggerType,
        config,
        is_enabled: isEnabled,
      };
      await save(newTrigger);
      onOpenChange(false);
    } catch (e) {
      console.error("Failed to save trigger:", e);
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{trigger ? "编辑触发器" : "新建触发器"}</DialogTitle>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {/* 触发器类型选择 */}
          <div className="space-y-1.5">
            <Label className="text-xs">类型</Label>
            <select
              className="h-8 w-full rounded-md border bg-background px-2 text-xs"
              value={triggerType}
              onChange={(e) => setTriggerType(e.target.value as TriggerType)}
            >
              <option value="schedule">{TRIGGER_LABELS.schedule}</option>
              <option value="plugin_event">{TRIGGER_LABELS.plugin_event}</option>
              <option value="clipboard">{TRIGGER_LABELS.clipboard}</option>
              <option value="hotkey">{TRIGGER_LABELS.hotkey}</option>
            </select>
          </div>

          {/* 类型特定配置 */}
          {triggerType === "schedule" && (
            <div className="space-y-1.5">
              <Label className="text-xs">间隔（秒）</Label>
              <Input
                type="number"
                min={1}
                className="h-8 text-xs"
                value={intervalSeconds}
                onChange={(e) => setIntervalSeconds(parseInt(e.target.value) || 60)}
              />
            </div>
          )}

          {triggerType === "plugin_event" && (
            <div className="space-y-1.5">
              <Label className="text-xs">事件类型</Label>
              {allEventTypes.length > 0 ? (
                <select
                  className="h-8 w-full rounded-md border bg-background px-2 text-xs"
                  value={eventType}
                  onChange={(e) => setEventType(e.target.value)}
                >
                  <option value="">选择事件...</option>
                  {allEventTypes.map((e) => (
                    <option key={e} value={e}>{e}</option>
                  ))}
                </select>
              ) : (
                <Input
                  className="h-8 text-xs"
                  value={eventType}
                  onChange={(e) => setEventType(e.target.value)}
                  placeholder="如 clipboard_changed"
                />
              )}
            </div>
          )}

          {triggerType === "clipboard" && (
            <div className="space-y-1.5">
              <Label className="text-xs">正则匹配（可选）</Label>
              <Input
                className="h-8 text-xs"
                value={pattern}
                onChange={(e) => setPattern(e.target.value)}
                placeholder="留空则任何变化都触发"
              />
            </div>
          )}

          {triggerType === "hotkey" && (
            <div className="space-y-1.5">
              <Label className="text-xs">快捷键</Label>
              <Input
                className="h-8 text-xs"
                value={shortcut}
                onChange={(e) => setShortcut(e.target.value)}
                placeholder="CmdOrCtrl+Shift+K"
              />
              <p className="text-[10px] text-muted-foreground">
                格式：CmdOrCtrl / Alt / Shift / Super + 字母/数字
              </p>
            </div>
          )}

          {/* 启用开关 */}
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="trigger-enabled"
              checked={isEnabled}
              onChange={(e) => setIsEnabled(e.target.checked)}
              className="size-3.5"
            />
            <Label htmlFor="trigger-enabled" className="text-xs">
              启用
            </Label>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button onClick={handleSave} disabled={saving}>
            {saving ? "保存中..." : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
