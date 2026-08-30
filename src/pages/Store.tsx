// 插件市场页：GitHub 目录源浏览 + 安装/更新（docs/PLUGINS.md §7）
// Store page: browse a GitHub dir source and install/update (docs/PLUGINS.md §7)
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Download, RefreshCw, Save } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardFooter, CardHeader } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { usePluginsStore } from "@/stores/plugins";
import { loadStoreRepo, saveStoreRepo, type StoreEntry } from "@/lib/store";
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
  const upToDate = installedVersion === entry.version;

  return (
    <Card>
      <CardHeader className="pb-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="truncate font-semibold">{entry.name}</h3>
              <Badge variant="secondary">v{entry.version}</Badge>
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
  const [repo, setRepo] = useState("");
  const [repoInput, setRepoInput] = useState("");
  const [entries, setEntries] = useState<StoreEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [busyDir, setBusyDir] = useState<string | null>(null);

  // 拉取市场列表（repo 变化触发；失败 toast，保留旧列表）
  // Fetch the listing (on repo change); failures toast and keep the old list
  const refresh = useCallback(
    async (source: string) => {
      if (!isTauri() || !source) return;
      setLoading(true);
      try {
        setEntries(await listStore(source));
      } catch (e) {
        toast.error(String(e));
      } finally {
        setLoading(false);
      }
    },
    [listStore]
  );

  // 初始化：已安装列表 + 持久化的源
  // Init: installed list + persisted source
  useEffect(() => {
    load();
    loadStoreRepo().then((saved) => {
      setRepo(saved);
      setRepoInput(saved);
    });
  }, [load]);

  // 源就绪或变更即拉取（初始 repo 为空串时跳过）
  // Fetch when the source is ready or changes (the initial empty repo is a no-op)
  useEffect(() => {
    if (repo) void refresh(repo);
  }, [repo, refresh]);

  // 保存源并刷新 / persist the source and refresh
  const handleSaveRepo = async () => {
    const next = repoInput.trim();
    if (!next) return;
    try {
      await saveStoreRepo(next);
      setRepo(next);
      toast.success(t("store.repoSaved"));
    } catch (e) {
      toast.error(String(e));
    }
  };

  const handleInstall = async (entry: StoreEntry) => {
    setBusyDir(entry.dir);
    const updating = plugins.some((p) => p.id === entry.id);
    try {
      await installFromStore(repo, entry.dir);
      toast.success(
        updating
          ? t("store.toastUpdated", { name: entry.name })
          : t("plugins.toastInstalled", { name: entry.name })
      );
      await refresh(repo);
    } catch (e) {
      toast.error(String(e));
    } finally {
      setBusyDir(null);
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
        <Button
          variant="outline"
          size="sm"
          disabled={loading || !repo}
          onClick={() => refresh(repo)}
        >
          <RefreshCw className={loading ? "animate-spin" : undefined} />
          {t("store.refresh")}
        </Button>
      </div>

      {/* 源配置：owner/name，持久化 settings.store.repo */}
      {/* Source config: owner/name, persisted to settings.store.repo */}
      <div className="mt-4 flex items-center gap-2">
        <Input
          value={repoInput}
          onChange={(e) => setRepoInput(e.target.value)}
          placeholder="owner/name"
          className="max-w-xs font-mono text-xs"
          aria-label={t("store.repo")}
        />
        <Button size="sm" variant="outline" disabled={!repoInput.trim() || repoInput.trim() === repo} onClick={handleSaveRepo}>
          <Save />
          {t("store.save")}
        </Button>
      </div>
      <p className="mt-1 text-xs text-muted-foreground/70">{t("store.repoHint")}</p>

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
                key={entry.dir}
                entry={entry}
                installedVersion={installedVersions.get(entry.id) ?? null}
                onInstall={handleInstall}
                busy={busyDir === entry.dir}
              />
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
