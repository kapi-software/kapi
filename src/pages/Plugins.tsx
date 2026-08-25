/**
 * @file Plugins.tsx
 * @description 插件管理页：Phase 4 插件系统落地前的占位
 * Plugins page: placeholder until the Phase 4 plugin system lands
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 占位页
 * - 2026-08-25: 接入 i18n
 */

import { useTranslation } from "react-i18next";

export default function Plugins() {
  const { t } = useTranslation();

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="text-2xl font-bold">{t("plugins.title")}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t("plugins.desc")}</p>
    </div>
  );
}
