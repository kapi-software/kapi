// 主面板顶栏：侧边栏切换按钮 + 面包屑（左）+ 窗口控制按钮（右），整条可拖拽
// Panel top bar: sidebar trigger + breadcrumb (left) + window controls (right), fully draggable
// 布局对应 shadcn sidebar-07：Trigger 与 Breadcrumb 位于 SidebarInset 顶栏
// Layout follows shadcn sidebar-07: trigger and breadcrumb live in the SidebarInset header
import { useTranslation } from "react-i18next";
import { NavLink, useLocation } from "react-router-dom";
import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { SidebarTrigger } from "@/components/ui/sidebar";
import { Separator } from "@/components/ui/separator";
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
    <div className="ml-auto flex items-center gap-0.5">
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
    <Breadcrumb>
      <BreadcrumbList>
        {active?.to !== "/" && (
          <>
            <BreadcrumbItem className="hidden md:block">
              <BreadcrumbLink asChild>
                <NavLink to="/">{t("nav.home")}</NavLink>
              </BreadcrumbLink>
            </BreadcrumbItem>
            <BreadcrumbSeparator className="hidden md:block" />
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
// Draggable top bar (frameless window) + sidebar toggle + breadcrumb + window controls
export function TopBar() {
  return (
    <header
      data-tauri-drag-region
      className="flex h-14 shrink-0 select-none items-center gap-2 border-b px-4"
    >
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-1 h-4" />
      <PageBreadcrumb />
      <WindowControls />
    </header>
  );
}
