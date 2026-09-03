// 独立页面布局：无侧边栏，复用 TopBar（含返回按钮），全屏内容区
// Standalone page layout: no sidebar, reuse TopBar (with back button), full-screen content
import type { CSSProperties } from "react";
import { TopBar, HEADER_HEIGHT } from "@/components/navigation/TopBar";
import { Toaster } from "@/components/ui/sonner";

export function StandaloneLayout({ children }: { children: React.ReactNode }) {
  return (
    <div
      className="flex h-svh flex-col"
      // TopBar 用 h-(--header-height) 撑高；无 SidebarProvider 时需在此补上变量
      // TopBar sizes via h-(--header-height); supply the variable here without SidebarProvider
      style={{ "--header-height": HEADER_HEIGHT } as CSSProperties}
    >
      <TopBar />
      {/* 主区：flex-col，让子元素 h-full 可靠填满剩余高度，不自己滚动 */}
      {/* Main: flex-col so children use h-full reliably; scrolling is each child's responsibility */}
      <main className="flex min-h-0 flex-1 flex-col bg-muted/40 p-3 md:p-4">
        {children}
      </main>
      <Toaster position="top-right" offset={12} />
    </div>
  );
}
