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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { useTriggersStore } from "@/stores/triggers";
import { usePluginsStore } from "@/stores/plugins";
import type { WorkflowTrigger, TriggerType } from "@/types";
import {
  Combobox,
  ComboboxContent,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxEmpty,
} from "@/components/ui/combobox";

interface Props {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  workflowId: string;
  trigger?: WorkflowTrigger | null;
}

const TRIGGER_OPTIONS: { value: TriggerType; label: string }[] = [
  { value: "schedule", label: "定时" },
  { value: "plugin_event", label: "插件事件" },
  { value: "clipboard", label: "剪贴板" },
  { value: "hotkey", label: "快捷键" },
];

export function TriggerDialog({ open, onOpenChange, workflowId, trigger }: Props) {
  const { save } = useTriggersStore();
  const { plugins, getDistinctEventTypes } = usePluginsStore();
  // 合并：插件 manifest 声明 + 历史事件表已出现的事件
  // Merge: plugin manifest declarations + events seen in plugin_events history
  const [historyEvents, setHistoryEvents] = useState<string[]>([]);
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

  // 收集所有可用事件类型（去重）
  // manifest 声明 + 历史事件合并
  // Collect all available event types: merge manifest declarations + history
  const manifestEvents = Array.from(
    new Set(plugins.flatMap((p) => p.manifest?.workflow?.events ?? []))
  );
  const allEventTypes = Array.from(
    new Set([...manifestEvents, ...historyEvents])
  );
  // 标记每个事件来源（用于下拉框标签）
  // Tag each event with its source for the dropdown
  const eventSourceOf = (e: string): "manifest" | "history" | "both" => {
    const inManifest = manifestEvents.includes(e);
    const inHistory = historyEvents.includes(e);
    if (inManifest && inHistory) return "both";
    if (inManifest) return "manifest";
    return "history";
  };

  // 打开时加载历史事件
  // Load history events on open
  useEffect(() => {
    if (!open || triggerType !== "plugin_event") return;
    getDistinctEventTypes()
      .then(setHistoryEvents)
      .catch(() => setHistoryEvents([]));
  }, [open, triggerType, getDistinctEventTypes]);

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
            <Select value={triggerType} onValueChange={(v) => setTriggerType(v as TriggerType)}>
              <SelectTrigger className="w-full h-8 text-xs">
                <SelectValue placeholder="选择类型" />
              </SelectTrigger>
              <SelectContent>
                {TRIGGER_OPTIONS.map((opt) => (
                  <SelectItem key={opt.value} value={opt.value} className="text-xs">
                    {opt.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
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
              <div className="flex items-center justify-between">
                <Label className="text-xs">事件类型</Label>
                <span className="text-[10px] text-muted-foreground/70">
                  共 {allEventTypes.length} 个 ·{" "}
                  清单 {manifestEvents.length} · 历史 {historyEvents.length}
                </span>
              </div>
              {allEventTypes.length > 0 ? (
                <Combobox
                  value={eventType}
                  onValueChange={(v) => setEventType(v as string)}
                >
                  <ComboboxInput
                    placeholder="搜索事件类型..."
                    className="w-full"
                    showTrigger
                  />
                  <ComboboxContent>
                    <ComboboxList>
                      <ComboboxEmpty className="text-xs">未找到匹配事件</ComboboxEmpty>
                      {allEventTypes.map((e) => {
                        const src = eventSourceOf(e);
                        const suffix =
                          src === "both"
                            ? "（清单+历史）"
                            : src === "history"
                              ? "（仅历史）"
                              : "（清单）";
                        return (
                          <ComboboxItem key={e} value={e} className="text-xs">
                            <span className="font-mono">{e}</span>
                            <span className="ml-2 text-[10px] text-muted-foreground">
                              {suffix}
                            </span>
                          </ComboboxItem>
                        );
                      })}
                    </ComboboxList>
                  </ComboboxContent>
                </Combobox>
              ) : (
                <Input
                  className="h-8 text-xs"
                  value={eventType}
                  onChange={(e) => setEventType(e.target.value)}
                  placeholder="如 clipboard_changed"
                />
              )}
              <p className="text-[10px] text-muted-foreground/70">
                清单 = 插件 manifest 声明 · 历史 = plugin_events 表中实际出现过的事件
              </p>
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
          <div className="flex items-center gap-2 pt-1">
            <Switch
              id="trigger-enabled"
              checked={isEnabled}
              onCheckedChange={setIsEnabled}
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
