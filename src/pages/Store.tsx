// 插件市场页：索引源浏览 + 安装/更新（docs/PLUGINS.md §7）
// Store page: browse the index source and install/update (docs/PLUGINS.md §7)
// 列表缓存优先（settings.store.index），仅手动刷新回源 index.json
// Cache-first listing (settings.store.index); only a manual refresh hits index.json
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Download, RefreshCw } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardFooter, CardHeader } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { usePluginsStore } from "@/stores/plugins";
import type { StoreEntry } from "@/lib/store";
import { isTauri } from "@/lib/tauri";

// 单个市场卡片：元信息 + 安装/更新/已安装态
// One store card: metadata plus install/update/installed state
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

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
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
            </div>
            <p className="mt-1 line-clamp-2 min-h-8 text-xs text-muted-foreground">
              {entry.description ?? t("plugins.noDesc")}
            </p>
            <p className="mt-1 truncate text-[10px] text-muted-foreground/70">
              {entry.id}
              {entry.author ? ` · ${entry.author}` : ""}
            </p>
          </div>
        </div>
      </CardHeader>
      <CardFooter className="pt-0">
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
      </CardFooter>
    </Card>
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
    <div className="mx-auto max-w-4xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold">{t("store.title")}</h1>
          <p className="mt-1 text-sm text-muted-foreground">{t("store.desc")}</p>
        </div>
        <Button variant="outline" size="sm" disabled={loading} onClick={() => refresh(true)}>
          <RefreshCw className={loading ? "animate-spin" : undefined} />
          {t("store.refresh")}
        </Button>
      </div>

      <div className="mt-6">
        {loading && entries === null ? (
          <div className="grid gap-4 md:grid-cols-2">
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
          <div className="grid gap-4 md:grid-cols-2">
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
    </div>
  );
}
