// 日志页：system_logs 查看，支持级别过滤与自动刷新（Phase 2 范围，docs/PANEL.md）
// Logs page: system_logs viewer with level filtering and auto-refresh (Phase 2, docs/PANEL.md)
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { isTauri } from "@/lib/tauri";
import { logDb, initDb } from "@/lib/db";
import type { SystemLog, LogLevel } from "@/types";

// 单次拉取条数（过滤在内存中进行，避免频繁查询）
// Rows fetched per load (filtering happens in memory to avoid frequent queries)
const FETCH_LIMIT = 200;

// 自动刷新间隔
// Auto-refresh interval
const REFRESH_INTERVAL_MS = 5000;

// 级别过滤选项（'all' 表示不过滤）
// Level filter options ('all' disables filtering)
const LEVEL_FILTERS: Array<LogLevel | 'all'> = ['all', 'debug', 'info', 'warn', 'error'];

// 级别 → Badge 变体
// Level → Badge variant
function levelVariant(level: LogLevel): "destructive" | "secondary" | "outline" {
  if (level === "error") return "destructive";
  if (level === "warn") return "secondary";
  return "outline";
}

export default function Logs() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<SystemLog[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [levelFilter, setLevelFilter] = useState<LogLevel | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(false);

  // 拉取最近日志；refreshing 标记仅用于手动刷新按钮的加载态
  // Fetch recent logs; the refreshing flag only drives the manual button state
  const refresh = useCallback(async (manual = false) => {
    if (!isTauri()) {
      setLoaded(true);
      return;
    }
    if (manual) setRefreshing(true);
    try {
      await initDb();
      const rows = await logDb.getRecent(FETCH_LIMIT);
      setLogs(rows);
    } catch (e) {
      console.error("日志加载失败 / Failed to load logs:", e);
    } finally {
      setLoaded(true);
      if (manual) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // 自动刷新：开关控制定时器，避免重复注册
  // Auto refresh: the switch owns the timer so it is never registered twice
  useEffect(() => {
    if (!autoRefresh) return;
    const timer = setInterval(() => refresh(), REFRESH_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [autoRefresh, refresh]);

  // 级别过滤在内存中进行
  // Level filtering happens in memory
  const filtered = useMemo(
    () => (levelFilter === "all" ? logs : logs.filter((l) => l.level === levelFilter)),
    [logs, levelFilter]
  );

  return (
    <div className="mx-auto max-w-3xl space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t("logs.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("logs.subtitle", { count: FETCH_LIMIT })}</p>
      </div>

      <Card>
        <CardHeader>
          {/* 工具行：级别过滤 + 自动刷新 + 手动刷新 */}
          {/* Toolbar: level filter + auto refresh + manual refresh */}
          <CardTitle className="flex flex-wrap items-center gap-2">
            {LEVEL_FILTERS.map((level) => (
              <Button
                key={level}
                size="sm"
                variant={levelFilter === level ? "default" : "outline"}
                className="h-7 px-2.5 font-mono text-xs lowercase"
                onClick={() => setLevelFilter(level)}
              >
                {level === "all" ? t("logs.levelAll") : level}
              </Button>
            ))}
            <div className="ml-auto flex items-center gap-2">
              <span className="text-xs text-muted-foreground">{t("logs.autoRefresh")}</span>
              <Switch
                size="sm"
                checked={autoRefresh}
                onCheckedChange={setAutoRefresh}
              />
              <Button
                size="sm"
                variant="outline"
                className="h-7 gap-1.5 px-2.5"
                disabled={refreshing}
                onClick={() => refresh(true)}
              >
                <RefreshCw className={cn("size-3.5", refreshing && "animate-spin")} />
                {t("logs.refresh")}
              </Button>
            </div>
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!loaded && <p className="text-sm text-muted-foreground">{t("logs.loading")}</p>}
          {loaded && filtered.length === 0 && (
            <p className="text-sm text-muted-foreground">
              {logs.length === 0 ? t("logs.empty") : t("logs.emptyFiltered")}
            </p>
          )}
          {filtered.length > 0 && (
            <>
              <p className="mb-2 text-xs text-muted-foreground">
                {t("logs.shownCount", { shown: filtered.length, total: logs.length })}
              </p>
              <ul className="space-y-1 font-mono text-xs">
                {filtered.map((log) => (
                  <li key={log.id} className="flex items-start gap-2">
                    <Badge variant={levelVariant(log.level)} className="mt-0.5 shrink-0 lowercase">
                      {log.level}
                    </Badge>
                    <span className="shrink-0 text-muted-foreground">{log.created_at}</span>
                    <span className="min-w-0 break-all">{log.message}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
