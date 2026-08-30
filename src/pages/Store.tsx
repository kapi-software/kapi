// 插件市场页：索引源浏览 + 安装/更新（docs/PLUGINS.md §7）
// Store page: browse the index source and install/update (docs/PLUGINS.md §7)
// 列表缓存优先（settings.store.index），仅手动刷新回源 index.json
// Cache-first listing (settings.store.index); only a manual refresh hits index.json
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Download, Eye, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";
import { usePluginsStore } from "@/stores/plugins";
import type { StoreEntry } from "@/lib/store";
import { isTauri } from "@/lib/tauri";
import { CardDetailDialog } from "@/components/common/CardDetailDialog";

// 单个市场卡片：元信息 + 安装/更新/已安装态 + 查看详情
// One store card: metadata plus install/update/installed state + detail dialog
function StoreCard({
  entry,
  installedVersion,
  onInstall,
  busy,
}: {
  entry: StoreEntry;
  installedVersion: string | null;
  onInstall: (entry: StoreEntry) => void;
  busy: boolean;
}) {
  const { t } = useTranslation();
  const upToDate = installedVersion === (entry.version ?? "?");
  const [detailOpen, setDetailOpen] = useState(false);

  return (
    <>
      <Card className="flex h-full flex-col gap-3 p-4">
        {/* 顶部：标题 + 徽章 / Top: title + badges */}
        <div className="space-y-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate font-semibold">{entry.name ?? entry.id}</h3>
            {entry.version && <Badge variant="secondary">v{entry.version}</Badge>}
            {entry.category && <Badge variant="outline">{entry.category}</Badge>}
            {installedVersion !== null && (
              <Badge variant={upToDate ? "secondary" : "default"}>
                {upToDate
                  ? t("store.installed")
                  : t("store.updatesAvailable", { version: installedVersion })}
              </Badge>
            )}
            {/* 详情按钮：过长字段可在此查看完整内容 */}
            {/* Detail button: shows full content for overlong fields */}
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
            {entry.description ?? t("plugins.noDesc")}
          </p>
          <p className="truncate text-[10px] text-muted-foreground/70">
            {entry.id}
            {entry.author ? ` · ${entry.author}` : ""}
          </p>
        </div>

        {/* 底部：操作按钮 / Bottom: action button */}
        <div className="mt-auto pt-1">
          <Button
            size="sm"
            variant={installedVersion === null ? "default" : "outline"}
            disabled={busy || upToDate}
            onClick={() => onInstall(entry)}
          >
            <Download />
            {busy
              ? t("store.installing")
              : installedVersion === null
                ? t("store.install")
                : t("store.update")}
          </Button>
        </div>
      </Card>

      <CardDetailDialog
        open={detailOpen}
        onOpenChange={setDetailOpen}
        title={entry.name ?? entry.id}
        description={entry.id}
      >
        <div className="space-y-3 text-sm">
          <Field label={t("plugins.fieldId")}>{entry.id}</Field>
          <Field label={t("plugins.fieldName")}>{entry.name ?? "—"}</Field>
          <Field label={t("plugins.fieldVersion")}>v{entry.version ?? "—"}</Field>
          <Field label={t("plugins.fieldCategory")}>{entry.category ?? "—"}</Field>
          <Field label={t("plugins.fieldAuthor")}>{entry.author ?? "—"}</Field>
          <Field label={t("plugins.fieldDescription")}>
            {entry.description ?? t("plugins.noDesc")}
          </Field>
          <Field label={t("plugins.fieldRepo")}>{entry.repo}</Field>
          <Field label={t("plugins.fieldDir")}>{entry.dir ?? "—"}</Field>
        </div>
      </CardDetailDialog>
    </>
  );
}

// 单行字段标签 / One row of label + value
function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[auto_1fr] items-baseline gap-3">
      <span className="text-xs font-medium text-muted-foreground">{label}</span>
      <span className="break-all">{children}</span>
    </div>
  );
}

export default function Store() {
  const { t } = useTranslation();
  const { plugins, load, listStore, installFromStore } = usePluginsStore();
  const [entries, setEntries] = useState<StoreEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [busyId, setBusyId] = useState<string | null>(null);

  // 拉取列表：manual=true 强制回源并更新缓存，否则缓存优先
  // Fetch the listing: manual=true refetches and updates the cache, else cache-first
  const refresh = useCallback(
    async (manual = false) => {
      if (!isTauri()) return;
      setLoading(true);
      try {
        setEntries(await listStore(manual));
      } catch (e) {
        toast.error(String(e));
      } finally {
        setLoading(false);
      }
    },
    [listStore]
  );

  // 挂载：已安装列表 + 缓存的市场列表
  // Mount: the installed list plus the cached store listing
  useEffect(() => {
    load();
    refresh();
  }, [load, refresh]);

  const handleInstall = async (entry: StoreEntry) => {
    setBusyId(entry.id);
    const updating = plugins.some((p) => p.id === entry.id);
    try {
      await installFromStore(entry.repo, entry.dir ?? null);
      toast.success(
        updating
          ? t("store.toastUpdated", { name: entry.name ?? entry.id })
          : t("plugins.toastInstalled", { name: entry.name ?? entry.id })
      );
      await refresh();
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusyId(null);
    }
  };

  const installedVersions = new Map(plugins.map((p) => [p.id, p.version]));

  return (
    <div className="space-y-4">
      {/* 工具栏：按钮放左 / Toolbar: actions on the left */}
      <div className="flex items-center gap-2">
        <Button variant="outline" size="sm" disabled={loading} onClick={() => refresh(true)}>
          <RefreshCw className={cn(loading && "animate-spin")} />
          {t("store.refresh")}
        </Button>
      </div>

      {loading && entries === null ? (
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          <Skeleton className="h-36 rounded-xl" />
          <Skeleton className="h-36 rounded-xl" />
        </div>
      ) : !isTauri() ? (
        <div className="rounded-xl border border-dashed p-10 text-center">
          <p className="font-medium">{t("store.browserTitle")}</p>
          <p className="mt-1 text-sm text-muted-foreground">{t("plugins.browserHint")}</p>
        </div>
      ) : entries !== null && entries.length === 0 ? (
        <div className="rounded-xl border border-dashed p-10 text-center">
          <p className="font-medium">{t("store.empty")}</p>
          <p className="mt-1 text-sm text-muted-foreground">{t("store.emptyHint")}</p>
        </div>
      ) : entries !== null ? (
        <div className="grid gap-4 sm:grid-cols-1 md:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          {entries.map((entry) => (
            <StoreCard
              key={entry.id}
              entry={entry}
              installedVersion={installedVersions.get(entry.id) ?? null}
              onInstall={handleInstall}
              busy={busyId === entry.id}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}