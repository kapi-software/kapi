// 顶栏搜索表单：全局搜索占位（TODO: 后续接入插件/工作流全局搜索）
// Header search form: global-search placeholder (TODO: wire to plugin/workflow search)
import { useTranslation } from "react-i18next";
import { Label } from "@/components/ui/label";
import { SidebarInput } from "@/components/ui/sidebar";
import { SearchIcon } from "lucide-react";

export function SearchForm({ ...props }: React.ComponentProps<"form">) {
  const { t } = useTranslation();

  return (
    // 占位表单：阻止回车提交
    // Placeholder form: prevent enter-key submission
    <form {...props} onSubmit={(e) => e.preventDefault()}>
      <div className="relative">
        <Label htmlFor="search" className="sr-only">
          {t("topbar.search")}
        </Label>
        <SidebarInput
          id="search"
          placeholder={t("topbar.searchPlaceholder")}
          className="h-8 pl-7"
        />
        <SearchIcon className="pointer-events-none absolute top-1/2 left-2 size-4 -translate-y-1/2 opacity-50 select-none" />
      </div>
    </form>
  );
}
