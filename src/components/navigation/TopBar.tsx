// 主面板顶栏（sidebar-16 布局）：侧边栏切换 + 返回 + 窗口控制
// Panel top bar (sidebar-16 layout): sidebar toggle + back button + window controls
// 子页面（编辑器/历史）通过 PageBack 组件在菜单按钮旁显示「返回」入口
// Sub-pages (editor / history) show a "Back" affordance next to the sidebar toggle
// 侧边栏切换按钮仅在 SidebarProvider 内显示（StandaloneLayout 不显示）
// The sidebar toggle only shows inside SidebarProvider (hidden in StandaloneLayout)
import { useContext } from "react";
import { useTranslation } from "react-i18next";
import { useLocation, useNavigate } from "react-router-dom";
import { ArrowLeft, Minus, PanelLeft, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { SidebarContext } from "@/components/ui/sidebar";

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

// 子页面返回按钮：根据当前路径推断回退目标
// Sub-page back button: derives a back route from the current path
function PageBack() {
  const { t } = useTranslation();
  const { pathname } = useLocation();
  const navigate = useNavigate();

  // 子页面：编辑器/历史/插件详情/日志/设置 都显示返回
  // Only show on sub-pages (editor / history / plugin detail / logs / settings)
  const isSub =
    (pathname.startsWith("/workflow/") && pathname !== "/workflow") ||
    pathname.startsWith("/plugin/") ||
    pathname === "/logs" ||
    pathname === "/settings";
  if (!isSub) return null;

  // 根据路径推断回退目标 / Derive a back target from the path
  const backTarget =
    pathname === "/logs" || pathname === "/settings"
      ? "/" // 日志/设置回到首页（侧边栏外，没有更具体的列表）
      : pathname.startsWith("/plugin/")
        ? "/plugins"
        : "/workflow"; // 工作流子页面回到 /workflow

  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-7 w-7"
      onClick={(e) => {
        e.stopPropagation();
        navigate(backTarget);
      }}
      aria-label={t("common.back")}
      title={t("common.back")}
    >
      <ArrowLeft className="size-4" />
    </Button>
  );
}

// 侧边栏切换按钮：仅在 SidebarProvider 内渲染
// Sidebar toggle: rendered only inside SidebarProvider
function SidebarToggle() {
  const { t } = useTranslation();
  const ctx = useContext(SidebarContext);
  if (!ctx) return null;
  return (
    <Button
      variant="ghost"
      size="icon"
      className="h-7 w-7"
      onClick={(e) => {
        e.stopPropagation();
        ctx.toggleSidebar();
      }}
    >
      <PanelLeft />
      <span className="sr-only">{t("topbar.toggleSidebar")}</span>
    </Button>
  );
}

// 主面板顶栏：可拖拽（无边框窗口）+ 侧边栏切换（按上下文）+ 返回 + 窗口控制
// Draggable top bar (frameless) + sidebar toggle (context-aware) + back + window controls
export function TopBar() {
  return (
    <header
      data-tauri-drag-region
      className="z-50 flex shrink-0 items-center border-b bg-background"
    >
      <div
        data-tauri-drag-region
        className="flex h-(--header-height) w-full items-center gap-2 px-4"
      >
        <SidebarToggle />
        <PageBack />
        <div className="ml-auto" data-tauri-drag-region />
        <WindowControls />
      </div>
    </header>
  );
}