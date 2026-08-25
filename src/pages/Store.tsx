/**
 * @file Store.tsx
 * @description 插件市场页：Phase 5 GitHub 集成落地前的占位
 * Store page: placeholder until the Phase 5 GitHub integration lands
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 占位页
 * - 2026-08-25: 接入 i18n
 */

import { useTranslation } from "react-i18next";

export default function Store() {
  const { t } = useTranslation();

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="text-2xl font-bold">{t("store.title")}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t("store.desc")}</p>
    </div>
  );
}
