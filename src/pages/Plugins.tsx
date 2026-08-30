// 插件管理页：已安装列表 + 本地导入 + 启停 / 模式切换 / 排序 / 卸载（docs/PLUGINS.md §6）
// Plugins page: installed list + local import + enable/mode/order/uninstall (docs/PLUGINS.md §6)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { ChevronDown, ChevronUp, Eye, FolderOpen, Play, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { usePluginsStore } from "@/stores/plugins";
import { pluginWindowLabel, usePluginWindows } from "@/hooks/use-plugin-windows";
import { isTauri } from "@/lib/tauri";
import type { Plugin, WindowMode } from "@/types";
import { CardDetailDialog } from "@/components/common/CardDetailDialog";
import { ConfirmDialog } from "@/components/common/ConfirmDialog";

// 运行模式选项（值 → i18n key）
// Window mode options (value -> i18n key)
const MODE_OPTIONS: Array<{ value: WindowMode; labelKey: string }> = [
  { value: "embedded", labelKey: "plugins.modeEmbedded" },
  { value: "independent", labelKey: "plugins.modeIndependent" },
  { value: "headless", labelKey: "plugins.modeHeadless" },
];

// 单行字段标签
// One row of label + value
function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[auto_1fr] items-baseline gap-3">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <span className="break-all">{children}</span>
    </div>
  );
}

// 单个插件卡片：信息 + 控件（模式 / 启停 / 启动 / 排序 / 卸载 / 详情）
// One plugin card: info + controls (mode / enable / launch / reorder / uninstall / detail)
function PluginCard({
  plugin,
  index,
  total,
  modeLocked,
}: {
  plugin: Plugin;
  index: number;
  total: number;
  // 独立窗口运行中：锁定模式切换（切换会导致悬挂窗口 / 静默失效）
  // Independent window open: lock the mode switch (switching would orphan/break the window)
  modeLocked: boolean;
}) {
  const { t } = useTranslation();
  const { setEnabled, setWindowMode, move, uninstall } = usePluginsStore();
  const [detailOpen, setDetailOpen] = useState(false);
  // 卸载确认 Dialog 状态 / Uninstall-confirm dialog state
  const [pendingUninstall, setPendingUninstall] = useState(false);
  const [uninstalling, setUninstalling] = useState(false);

  // 模式选项 = manifest 声明的形态（Rust 裁决 UnsupportedMode，这里只渲染可选项）；
  // 当前值若已过期（重装后不再支持）仍保留展示，用户可改选声明形态
  // Options = the manifest-declared shapes (Rust rules UnsupportedMode; we render only
  // these) — a stale current value stays visible so the user can pick a declared one
  const modeOptions = MODE_OPTIONS.filter((o) => plugin.supported_modes.includes(o.value));
  const staleCurrent = !modeOptions.some((o) => o.value === plugin.window_mode);

  const handleLaunch = async () => {
    try {
      await invoke("launch_plugin", { pluginId: plugin.id });
    } catch (e) {
      toast.error(String(e));
    }
  };

  // 卸载确认：Dialog 替代旧版 toast action 流程
  // Uninstall confirm: Dialog replaces the legacy toast action flow
  const requestUninstall = () => setPendingUninstall(true);
  const confirmUninstall = async () => {
    setUninstalling(true);
    try {
      await uninstall(plugin.id);
      toast.success(t("plugins.toastUninstalled", { name: plugin.name }));
      setPendingUninstall(false);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setUninstalling(false);
    }
  };

  return (
    <>
      <Card
        className={`flex h-full flex-col gap-3 p-4 ${
          plugin.is_enabled ? undefined : "opacity-60"
        }`}
      >
        {/* 顶部：标题 + 徽章 + 详情按钮 / Top: title + badges + detail button */}
        <div className="space-y-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate font-semibold">{plugin.name}</h3>
            <Badge variant="secondary">v{plugin.version}</Badge>
            {plugin.category && <Badge variant="outline">{plugin.category}</Badge>}
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
          <p className="line-clamp-2 min-h-8 text-xs text-muted-foreground">
            {plugin.description ?? t("plugins.noDesc")}
          </p>
          <p className="truncate text-[10px] text-muted-foreground/70">
            {plugin.id}
            {plugin.author ? ` · ${plugin.author}` : ""}
          </p>
        </div>

        {/* 底部：启停 Switch + 全部操作按钮 / Bottom: enable switch + all action buttons */}
        <div className="mt-auto flex items-center justify-between gap-2 pt-1">
          <Switch
            checked={plugin.is_enabled}
            onCheckedChange={(v) =>
              setEnabled(plugin.id, v).catch((e) => toast.error(String(e)))
            }
            aria-label={t("plugins.enabled")}
          />
          <div className="flex items-center gap-1">
            <Select
              value={plugin.window_mode}
              disabled={modeLocked}
              onValueChange={(v) =>
                setWindowMode(plugin.id, v as WindowMode).catch((e) =>
                  toast.error(String(e))
                )
              }
            >
              <SelectTrigger size="sm" className="w-28" aria-label={t("plugins.mode")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {staleCurrent && (
                  <SelectItem value={plugin.window_mode} disabled>
                    {t("plugins.modeUnsupported")}
                  </SelectItem>
                )}
                {modeOptions.map(({ value, labelKey }) => (
                  <SelectItem key={value} value={value}>
                    {t(labelKey)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
            {modeLocked && (
              <span
                className="text-[10px] text-muted-foreground/80"
                title={t("plugins.modeLocked")}
              >
                🔒
              </span>
            )}
            <Button size="sm" variant="outline" onClick={handleLaunch}>
              <Play />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={index === 0}
              onClick={() => move(plugin.id, -1)}
              aria-label={t("plugins.moveUp")}
            >
              <ChevronUp />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              disabled={index === total - 1}
              onClick={() => move(plugin.id, 1)}
              aria-label={t("plugins.moveDown")}
            >
              <ChevronDown />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="text-destructive hover:text-destructive"
              onClick={requestUninstall}
              aria-label={t("plugins.uninstall")}
            >
              <Trash2 />
            </Button>
          </div>
        </div>
      </Card>

      <CardDetailDialog
        open={detailOpen}
        onOpenChange={setDetailOpen}
        title={plugin.name}
        description={plugin.id}
      >
        <div className="space-y-3 text-sm">
          <Field label={t("plugins.fieldId")}>{plugin.id}</Field>
          <Field label={t("plugins.fieldName")}>{plugin.name}</Field>
          <Field label={t("plugins.fieldVersion")}>v{plugin.version}</Field>
          <Field label={t("plugins.fieldAuthor")}>{plugin.author ?? "—"}</Field>
          <Field label={t("plugins.fieldCategory")}>{plugin.category ?? "—"}</Field>
          <Field label={t("plugins.fieldDescription")}>
            {plugin.description ?? t("plugins.noDesc")}
          </Field>
          <Field label={t("plugins.fieldWindowMode")}>{plugin.window_mode}</Field>
          <Field label={t("plugins.fieldEnabled")}>
            {plugin.is_enabled ? "✓" : "✗"}
          </Field>
          <Field label={t("plugins.fieldSupportedModes")}>
            {plugin.supported_modes.join(", ")}
          </Field>
        </div>
      </CardDetailDialog>

      {/* 卸载确认 Dialog / Uninstall confirm dialog */}
      <ConfirmDialog
        open={pendingUninstall}
        onOpenChange={setPendingUninstall}
        title={t("plugins.uninstallConfirmTitle")}
        description={t("plugins.uninstallConfirmDesc", { name: plugin.name })}
        confirmLabel={t("plugins.uninstall")}
        destructive
        busy={uninstalling}
        onConfirm={confirmUninstall}
      />
    </>
  );
}

export default function Plugins() {
  const { t } = useTranslation();
  const { plugins, loading, load, installFromDir } = usePluginsStore();
  const pluginWindows = usePluginWindows();
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    load();
  }, [load]);

  // 本地导入：选目录 → Rust 校验 + 复制 + 入库
  // Local import: pick a dir; Rust validates, copies and inserts
  const handleImport = async () => {
    if (!isTauri()) {
      toast.error(t("plugins.browserHint"));
      return;
    }
    const dir = await open({ directory: true, multiple: false });
    if (!dir || typeof dir !== "string") return;
    setImporting(true);
    try {
      const plugin = await installFromDir(dir);
      toast.success(t("plugins.toastInstalled", { name: plugin.name }));
    } catch (e) {
      toast.error(String(e));
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* 工具栏：本地导入放左 */}
      {/* Toolbar: import action on the left */}
      <div className="flex items-center gap-2">
        <Button onClick={handleImport} disabled={importing}>
          <FolderOpen />
          {importing ? t("plugins.importing") : t("plugins.import")}
        </Button>
      </div>

      {loading ? (
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          <Skeleton className="h-40 rounded-xl" />
          <Skeleton className="h-40 rounded-xl" />
        </div>
      ) : plugins.length === 0 ? (
        <div className="rounded-xl border border-dashed p-10 text-center">
          <p className="font-medium">{t("plugins.empty")}</p>
          <p className="mt-1 text-sm text-muted-foreground">
            {t("plugins.emptyHint")}
          </p>
        </div>
      ) : (
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          {plugins.map((p, i) => (
            <PluginCard
              key={p.id}
              plugin={p}
              index={i}
              total={plugins.length}
              modeLocked={pluginWindows.has(pluginWindowLabel(p.id))}
            />
          ))}
        </div>
      )}
    </div>
  );
}