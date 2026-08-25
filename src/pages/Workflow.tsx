// 工作流页：Phase 6 DAG 引擎与编辑器落地前的占位
// Workflow page: placeholder until the Phase 6 DAG engine and editor land
import { useTranslation } from "react-i18next";

export default function Workflow() {
  const { t } = useTranslation();

  return (
    <div className="mx-auto max-w-3xl">
      <h1 className="text-2xl font-bold">{t("workflow.title")}</h1>
      <p className="mt-2 text-sm text-muted-foreground">{t("workflow.desc")}</p>
    </div>
  );
}
