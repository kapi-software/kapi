// 插件市场页：Phase 5 GitHub 集成落地前的占位
// Store page: placeholder until the Phase 5 GitHub integration lands
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
