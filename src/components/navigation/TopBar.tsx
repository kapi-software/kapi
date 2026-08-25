/**
 * @file TopBar.tsx
 * @description 主面板顶栏：拖拽区（无边框窗口）+ 窗口控制按钮
 * Panel top bar: drag region (frameless window) + window control buttons
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始实现（主窗口 decorations: false 的自绘标题栏）
 * - 2026-08-25: 接入 i18n
 */

import { Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import { isTauri } from "@/lib/tauri";

/**
 * 窗口控制按钮组（仅 Tauri 环境渲染）
 * Window control buttons (rendered only inside Tauri)
 *
 * 阻止事件冒泡，避免触发外层 data-tauri-drag-region 的拖拽
 * Stops event propagation to avoid triggering the outer drag region
 */
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
        className={`${btn} hover:bg-destructive hover:text-white`}
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

/** 主面板顶栏：整条可拖拽 + 窗口控制 / Draggable top bar with window controls */
export function TopBar() {
  return (
    <header
      data-tauri-drag-region
      className="flex h-10 shrink-0 select-none items-center border-b px-3"
    >
      <span data-tauri-drag-region className="text-sm font-semibold tracking-wide">
        Kapi
      </span>
      <WindowControls />
    </header>
  );
}
