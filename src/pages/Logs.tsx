// 日志页：system_logs 查看，Phase 2 完善过滤与自动刷新
// Logs page: system_logs viewer; filtering and auto-refresh land in Phase 2
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { isTauri } from "@/lib/tauri";
import { logDb, initDb } from "@/lib/db";
import type { SystemLog } from "@/types";

export default function Logs() {
  const { t } = useTranslation();
  const [logs, setLogs] = useState<SystemLog[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setLoaded(true);
      return;
    }
    initDb()
      .then(() => logDb.getRecent(50))
      .then((rows) => {
        setLogs(rows);
        setLoaded(true);
      })
      .catch((e) => {
        console.error("日志加载失败 / Failed to load logs:", e);
        setLoaded(true);
      });
  }, []);

  return (
    <div className="mx-auto max-w-3xl space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t("logs.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("logs.subtitle", { count: 50 })}</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>system_logs</CardTitle>
        </CardHeader>
        <CardContent>
          {!loaded && <p className="text-sm text-muted-foreground">{t("logs.loading")}</p>}
          {loaded && logs.length === 0 && (
            <p className="text-sm text-muted-foreground">{t("logs.empty")}</p>
          )}
          <ul className="space-y-1 font-mono text-xs">
            {logs.map((log) => (
              <li key={log.id} className="flex items-start gap-2">
                <Badge
                  variant={
                    log.level === "error"
                      ? "destructive"
                      : log.level === "warn"
                        ? "secondary"
                        : "outline"
                  }
                  className="mt-0.5 shrink-0"
                >
                  {log.level}
                </Badge>
                <span className="text-muted-foreground">{log.created_at}</span>
                <span className="min-w-0 break-all">{log.message}</span>
              </li>
            ))}
          </ul>
        </CardContent>
      </Card>
    </div>
  );
}
