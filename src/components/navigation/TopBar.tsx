// 主面板顶栏（sidebar-16 布局）：全宽置顶，侧边栏切换 + 面包屑 + 窗口控制
// Panel top bar (sidebar-16 layout): full-width on top with sidebar toggle, breadcrumb and window controls
import { useTranslation } from "react-i18next";
import { NavLink, useLocation } from "react-router-dom";
import { Minus, PanelLeft, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useSidebar } from "@/components/ui/sidebar";
import { Button } from "@/components/ui/button";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { NAV_ITEMS, isNavItemActive } from "@/components/navigation/AppSidebar";

// 窗口控制按钮组（仅 Tauri 环境渲染）
// Window control buttons (rendered only inside Tauri)
// 阻止事件冒泡，避免触发外层 data-tauri-drag-region 的拖拽
// Stops propagation to avoid triggering the outer drag region
function WindowControls() {
  const { t } = useTranslation();

  if (!isTauri()) return null;
  const win = getCurrentWindow();

  const btn =
    "flex size-7 items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-accent hover:text-foreground";

  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <button
        className={btn}
        title={t("topbar.minimize")}
        onClick={(e) => {
          e.stopPropagation();
          win.minimize();
        }}
      >
        <Minus className="size-3.5" />
      </button>
      <button
        className={btn}
        title={t("topbar.maximize")}
        onClick={(e) => {
          e.stopPropagation();
          win.toggleMaximize();
        }}
      >
        <Square className="size-3" />
      </button>
      <button
        className={cn(btn, "hover:bg-destructive hover:text-white")}
        title={t("topbar.close")}
        onClick={(e) => {
          e.stopPropagation();
          win.close();
        }}
      >
        <X className="size-4" />
      </button>
    </div>
  );
}

// 面包屑：首页链接 + 当前页面；位于首页时只显示一项
// Breadcrumb: home link + current page; collapses to one item on the home page
function PageBreadcrumb() {
  const { t } = useTranslation();
  const { pathname } = useLocation();

  const active = NAV_ITEMS.find(({ to, end }) => isNavItemActive(pathname, to, end));

  return (
    <Breadcrumb className="hidden sm:block">
      <BreadcrumbList>
        {active?.to !== "/" && (
          <>
            <BreadcrumbItem>
              <BreadcrumbLink asChild>
                <NavLink to="/">{t("nav.home")}</NavLink>
              </BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator />
          </>
        )}
        <BreadcrumbItem>
          <BreadcrumbPage>{t(active?.labelKey ?? "nav.home")}</BreadcrumbPage>
        </BreadcrumbItem>
      </BreadcrumbList>
    </Breadcrumb>
  );
}

// 主面板顶栏：可拖拽（无边框窗口）+ 侧边栏切换 + 面包屑 + 窗口控制
// Draggable top bar (frameless) + sidebar toggle + breadcrumb + window controls
export function TopBar() {
  const { t } = useTranslation();
  const { toggleSidebar } = useSidebar();

  return (
    <header
      data-tauri-drag-region
      className="z-50 flex shrink-0 items-center border-b bg-background"
    >
      <div data-tauri-drag-region className="flex h-(--header-height) w-full items-center gap-2 px-4">
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={(e) => {
            e.stopPropagation();
            toggleSidebar();
          }}
        >
          <PanelLeft />
          <span className="sr-only">{t("topbar.toggleSidebar")}</span>
        </Button>
        {/* <Separator orientation="vertical" className="mr-2 data-vertical:h-4 data-vertical:self-auto" /> */}
        <PageBreadcrumb />
        <div className="ml-auto" data-tauri-drag-region />
        <WindowControls />
      </div>
    </header>
  );
}
