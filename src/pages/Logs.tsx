// 日志页：system_logs 与 plugin_events 双视图，级别/来源过滤与自动刷新（docs/PANEL.md）
// Logs page: system_logs and plugin_events views with filtering and auto-refresh
// D3: UTC ISO → 本地可读时间 / UTC ISO → local readable time
function fmt(s: string | undefined | null): string {
  if (!s) return "—";
  try {
    return new Date(s).toLocaleString(undefined, {
      year: "numeric", month: "2-digit", day: "2-digit",
      hour: "2-digit", minute: "2-digit", second: "2-digit",
    });
  } catch { return s; }
}

import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, ScrollText, Zap } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { isTauri } from "@/lib/tauri";
import { logDb, eventDb, initDb } from "@/lib/db";
import type { SystemLog, PluginEvent, LogLevel } from "@/types";

// 单次拉取条数（过滤在内存中进行，避免频繁查询）
// Rows fetched per load (filtering happens in memory to avoid frequent queries)
const FETCH_LIMIT = 200;

// 自动刷新间隔
// Auto-refresh interval
const REFRESH_INTERVAL_MS = 5000;

// 视图：系统日志 / 插件事件
// Views: system logs / plugin events
type ViewMode = "logs" | "events";

// 级别过滤选项（'all' 表示不过滤）
// Level filter options ('all' disables filtering)
const LEVEL_FILTERS: Array<LogLevel | "all"> = ["all", "debug", "info", "warn", "error"];

// 级别 → Badge 变体
// Level → Badge variant
function levelVariant(level: LogLevel): "destructive" | "secondary" | "outline" {
  if (level === "error") return "destructive";
  if (level === "warn") return "secondary";
  return "outline";
}

// 事件 data JSON 展示截断（title 里保留全量，悬停可看）
// Truncate the event data JSON for display (title keeps the full text on hover)
function shortenData(data: string | null): string {
  if (!data) return "";
  return data.length > 120 ? `${data.slice(0, 120)}…` : data;
}

export default function Logs() {
  const { t } = useTranslation();
  const [view, setView] = useState<ViewMode>("logs");
  const [logs, setLogs] = useState<SystemLog[]>([]);
  const [events, setEvents] = useState<PluginEvent[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [levelFilter, setLevelFilter] = useState<LogLevel | "all">("all");
  const [autoRefresh, setAutoRefresh] = useState(false);

  // 拉取当前视图数据；refreshing 标记仅用于手动刷新按钮的加载态
  // Fetch the active view; the refreshing flag only drives the manual button state
  const refresh = useCallback(
    async (manual = false) => {
      if (!isTauri()) {
        setLoaded(true);
        return;
      }
      if (manual) setRefreshing(true);
      try {
        await initDb();
        if (view === "logs") {
          setLogs(await logDb.getRecent(FETCH_LIMIT));
        } else {
          setEvents(await eventDb.getRecent(FETCH_LIMIT));
        }
      } catch (e) {
        console.error("日志加载失败 / Failed to load logs:", e);
      } finally {
        setLoaded(true);
        if (manual) setRefreshing(false);
      }
    },
    [view]
  );

  // 视图切换即重拉（loaded 复位显示加载态）
  // Switching views refetches (loaded resets for the loading state)
  useEffect(() => {
    setLoaded(false);
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
    <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto">
      <Card className="gap-3 p-4">
        {/* 工具行：视图切换 + （日志视图）级别过滤 + 自动刷新 + 手动刷新 */}
        {/* Toolbar: view switch + (logs view) level filter + auto/manual refresh */}
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex items-center gap-1">
            <Button
              size="sm"
              variant={view === "logs" ? "default" : "outline"}
              className="h-7 gap-1.5 px-2.5 text-xs"
              onClick={() => setView("logs")}
            >
              <ScrollText className="size-3.5" />
              {t("logs.viewLogs")}
            </Button>
            <Button
              size="sm"
              variant={view === "events" ? "default" : "outline"}
              className="h-7 gap-1.5 px-2.5 text-xs"
              onClick={() => setView("events")}
            >
              <Zap className="size-3.5" />
              {t("logs.viewEvents")}
            </Button>
          </div>
          {view === "logs" &&
            LEVEL_FILTERS.map((level) => (
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
            <Switch size="sm" checked={autoRefresh} onCheckedChange={setAutoRefresh} />
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
        </div>

        {!loaded && <p className="text-sm text-muted-foreground">{t("logs.loading")}</p>}

        {/* 系统日志视图 / system-log view */}
        {loaded && view === "logs" && (
          <>
            {filtered.length === 0 && (
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
                      <span className="shrink-0 text-muted-foreground">{fmt(log.created_at)}</span>
                      <span className="min-w-0 break-all">{log.message}</span>
                    </li>
                  ))}
                </ul>
              </>
            )}
          </>
        )}

        {/* 插件事件视图 / plugin-event view */}
        {loaded && view === "events" && (
          <>
            {events.length === 0 && (
              <p className="text-sm text-muted-foreground">{t("logs.eventsEmpty")}</p>
            )}
            {events.length > 0 && (
              <ul className="space-y-1 font-mono text-xs">
                {events.map((ev) => (
                  <li key={ev.id} className="flex items-start gap-2">
                    <Badge variant="outline" className="mt-0.5 shrink-0">
                      <Zap className="mr-1 size-3" />
                      {ev.event_type}
                    </Badge>
                    <span className="shrink-0 text-muted-foreground">{fmt(ev.created_at)}</span>
                    <span className="shrink-0 text-muted-foreground/80">
                      {ev.source_plugin_id ?? t("logs.eventSourceUnknown")}
                    </span>
                    {ev.data && (
                      <span className="min-w-0 break-all" title={ev.data}>
                        {shortenData(ev.data)}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </>
        )}
      </Card>
    </div>
  );
}