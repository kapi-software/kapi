// 插件管理页：已安装列表 + 本地导入 + 启停 / 模式切换 / 排序 / 卸载（docs/PLUGINS.md §6）
// Plugins page: installed list + local import + enable/mode/order/uninstall (docs/PLUGINS.md §6)
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { ChevronDown, ChevronUp, FolderOpen, Play, Trash2 } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardFooter, CardHeader } from "@/components/ui/card";
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

// 运行模式选项（值 → i18n key）
// Window mode options (value -> i18n key)
const MODE_OPTIONS: Array<{ value: WindowMode; labelKey: string }> = [
  { value: "embedded", labelKey: "plugins.modeEmbedded" },
  { value: "independent", labelKey: "plugins.modeIndependent" },
  { value: "headless", labelKey: "plugins.modeHeadless" },
];

// 单个插件卡片：信息 + 控件（模式 / 启停 / 启动 / 排序 / 卸载）
// One plugin card: info + controls (mode / enable / launch / order / uninstall)
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

  const handleLaunch = async () => {
    try {
      await invoke("launch_plugin", { pluginId: plugin.id });
    } catch (e) {
      toast.error(String(e));
    }
  };

  // 两步确认：toast 中再点一次才执行卸载
  // Two-step confirm: click once more in the toast to actually uninstall
  const confirmUninstall = () => {
    toast(t("plugins.uninstallConfirm", { name: plugin.name }), {
      action: {
        label: t("plugins.uninstall"),
        onClick: () => {
          uninstall(plugin.id).catch((e) => toast.error(String(e)));
        },
      },
    });
  };

  return (
    <Card className={plugin.is_enabled ? undefined : "opacity-60"}>
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="truncate font-semibold">{plugin.name}</h3>
              <Badge variant="secondary">v{plugin.version}</Badge>
              {plugin.category && (
                <Badge variant="outline">{plugin.category}</Badge>
              )}
            </div>
            <p className="mt-1 line-clamp-2 min-h-8 text-xs text-muted-foreground">
              {plugin.description ?? t("plugins.noDesc")}
            </p>
            <p className="mt-1 truncate text-[10px] text-muted-foreground/70">
              {plugin.id}
              {plugin.author ? ` · ${plugin.author}` : ""}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Switch
              checked={plugin.is_enabled}
              onCheckedChange={(v) =>
                setEnabled(plugin.id, v).catch((e) => toast.error(String(e)))
              }
              aria-label={t("plugins.enabled")}
            />
          </div>
        </div>
      </CardHeader>
      <CardFooter className="flex items-center justify-between gap-2 pt-0">
        <div className="flex items-center gap-2">
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
              {MODE_OPTIONS.map(({ value, labelKey }) => (
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
            {t("plugins.launch")}
          </Button>
        </div>
        <div className="flex items-center gap-1">
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
            onClick={confirmUninstall}
            aria-label={t("plugins.uninstall")}
          >
            <Trash2 />
          </Button>
        </div>
      </CardFooter>
    </Card>
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
    <div className="mx-auto max-w-4xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold">{t("plugins.title")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("plugins.desc")}</p>
        </div>
        <Button onClick={handleImport} disabled={importing}>
          <FolderOpen />
          {importing ? t("plugins.importing") : t("plugins.import")}
        </Button>
      </div>

      <div className="mt-6">
        {loading ? (
          <div className="grid gap-4 md:grid-cols-2">
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
          <div className="grid gap-4 md:grid-cols-2">
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
    </div>
  );
}
