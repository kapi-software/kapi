/**
 * @file Dashboard.tsx
 * @description 首页仪表盘：Phase 1 展示数据库链路验证（迁移 → settings 表 → store）
 * Dashboard: Phase 1 database chain verification (migrations → settings table → store)
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 数据库链路验证页
 * - 2026-08-25: 接入 i18n
 */

import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardContent, CardHeader, CardTitle, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { isTauri } from "@/lib/tauri";
import { settingsDb, initDb } from "@/lib/db";
import { useSettingsStore } from "@/stores/settings";

/** 数据库链路状态 / DB chain status */
type DbStatus = "checking" | "ok" | "failed" | "browser";

export default function Dashboard() {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const ready = useSettingsStore((s) => s.ready);
  const [status, setStatus] = useState<DbStatus>("checking");
  const [rowCount, setRowCount] = useState(0);

  // 直接查询 settings 表行数：证明 Rust 迁移 + 前端连接全链路可用
  // Count settings rows directly to prove the full chain works
  useEffect(() => {
    if (!ready) return;
    if (!isTauri()) {
      setStatus("browser");
      return;
    }
    initDb()
      .then(() => settingsDb.getAll())
      .then((rows) => {
        setRowCount(Object.keys(rows).length);
        setStatus("ok");
      })
      .catch((e) => {
        console.error("数据库链路检查失败 / DB chain check failed:", e);
        setStatus("failed");
      });
  }, [ready]);

  return (
    <div className="mx-auto max-w-3xl space-y-4">
      <div>
        <h1 className="text-2xl font-bold">{t("dashboard.title")}</h1>
        <p className="text-sm text-muted-foreground">{t("dashboard.subtitle")}</p>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            {t("dashboard.dbTitle")}
            {status === "ok" && (
              <Badge variant="default">{t("dashboard.statusOk", { count: rowCount })}</Badge>
            )}
            {status === "checking" && <Badge variant="secondary">{t("dashboard.statusChecking")}</Badge>}
            {status === "browser" && <Badge variant="outline">{t("dashboard.statusBrowser")}</Badge>}
            {status === "failed" && <Badge variant="destructive">{t("dashboard.statusFailed")}</Badge>}
          </CardTitle>
          <CardDescription>{t("dashboard.dbDesc")}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-2 text-sm">
          {status === "browser" && (
            <p className="text-muted-foreground">{t("dashboard.browserHint")}</p>
          )}
          {status === "failed" && (
            <p className="text-destructive">{t("dashboard.failedHint")}</p>
          )}
          {status === "ok" && (
            <ul className="grid grid-cols-2 gap-x-4 gap-y-1 text-muted-foreground">
              <li>
                {t("dashboard.theme")}: <span className="text-foreground">{settings.theme}</span>
              </li>
              <li>
                {t("dashboard.language")}:{" "}
                <span className="text-foreground">{settings.language}</span>
              </li>
              <li>
                {t("dashboard.dockEnabled")}:{" "}
                <span className="text-foreground">{String(settings.dock_enabled)}</span>
              </li>
              <li>
                {t("dashboard.dockVisible")}:{" "}
                <span className="text-foreground">{settings.dock_visible_items}</span>
              </li>
              <li>
                {t("dashboard.sandboxStrict")}:{" "}
                <span className="text-foreground">{String(settings.plugin_sandbox_strict)}</span>
              </li>
              <li>
                {t("dashboard.accentColor")}:{" "}
                <span className="text-foreground">{settings.accent_color}</span>
              </li>
            </ul>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>{t("dashboard.roadmapTitle")}</CardTitle>
          <CardDescription>{t("dashboard.roadmapDesc")}</CardDescription>
        </CardHeader>
        <CardContent>
          <ol className="list-inside list-decimal space-y-1 text-sm text-muted-foreground">
            <li>{t("dashboard.phase1")}</li>
            <li className="opacity-60">{t("dashboard.phase2")}</li>
            <li className="opacity-60">{t("dashboard.phase3")}</li>
            <li className="opacity-60">{t("dashboard.phase4")}</li>
            <li className="opacity-60">{t("dashboard.phase57")}</li>
          </ol>
        </CardContent>
      </Card>
    </div>
  );
}
