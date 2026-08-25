// 应用根组件：窗口分流、主题与语言应用、主面板布局与路由
// Root component: window routing, theme & language application, panel layout and routes
import { useEffect, useState, type CSSProperties } from "react";
import { BrowserRouter, Routes, Route, Outlet } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";
import { resolveThemeClass } from "@/lib/settings";
import { accentVars } from "@/lib/theme";
import { normalizeLanguage } from "@/i18n";
import i18n from "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/navigation/AppSidebar";
import { TopBar } from "@/components/navigation/TopBar";
import DockApp from "@/dock/Dock";
import Dashboard from "@/pages/Dashboard";
import Plugins from "@/pages/Plugins";
import Store from "@/pages/Store";
import Workflow from "@/pages/Workflow";
import Logs from "@/pages/Logs";
import Settings from "@/pages/Settings";

// 主面板布局：官方 Sidebar（inset 变体）+ SidebarInset 内容区（shadcn sidebar-07）
// Panel layout: official Sidebar (inset variant) + SidebarInset content (shadcn sidebar-07)
function PanelLayout() {
  return (
    // 覆盖官方默认宽度：展开 16rem→12rem，图标态保持 3rem（仅此处声明，官方组件不改）
    // Override the official width: expanded 16rem→12rem, icon rail keeps the 3rem default (declared only here, vendored file untouched)
    <SidebarProvider
      style={
        {
          "--sidebar-width": "12rem",
          "--sidebar-width-icon": "3rem",
        } as CSSProperties
      }
    >
      <AppSidebar />
      {/* inset 变体在 md+ 有 m-2 外边距：高度须扣除 1rem，否则窗口级出现常驻滚动条 */}
      {/* The inset variant adds m-2 margins on md+: subtract 1rem or the window keeps a permanent scrollbar */}
      <SidebarInset className="h-svh max-h-svh md:h-[calc(100svh-1rem)] md:max-h-[calc(100svh-1rem)]">
        <TopBar />
        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain p-4 md:p-6">
          <Outlet />
        </div>
      </SidebarInset>
    </SidebarProvider>
  );
}

// 应用根组件：按窗口 label 分流
// Root component: routes by window label
export default function App() {
  const [entry, setEntry] = useState<"main" | "dock" | null>(null);
  const theme = useSettingsStore((s) => s.settings.theme);
  const language = useSettingsStore((s) => s.settings.language);
  const loadSettings = useSettingsStore((s) => s.loadSettings);

  // 启动时加载设置（含数据库连接）
  // Load settings on startup (includes the DB connection)
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // 主题应用：light/dark/system → html.dark class（shadcn 约定）
  // Theme application: light/dark/system → html.dark class (shadcn convention)
  useEffect(() => {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const cls = resolveThemeClass(theme, mq.matches);
      document.documentElement.classList.toggle("dark", cls === "dark");
    };
    apply();
    mq.addEventListener("change", apply);
    return () => mq.removeEventListener("change", apply);
  }, [theme]);

  // 语言应用：settings.language → i18n.changeLanguage + html lang
  // Language application: settings.language → i18n.changeLanguage + html lang
  useEffect(() => {
    const lang = normalizeLanguage(language);
    i18n.changeLanguage(lang);
    document.documentElement.lang = lang;
  }, [language]);

  // 强调色应用：settings.accent_color → root 级 CSS 变量（docs/PANEL.md §4.1）
  // Accent application: settings.accent_color → root-level CSS variables (docs/PANEL.md §4.1)
  const accentColor = useSettingsStore((s) => s.settings.accent_color);
  useEffect(() => {
    for (const [name, value] of Object.entries(accentVars(accentColor))) {
      document.documentElement.style.setProperty(name, value);
    }
  }, [accentColor]);

  // 窗口分流：dock 窗口渲染 Dock UI（Phase 3），其余渲染主面板
  // Window routing: the dock window renders the Dock UI (Phase 3), others the panel
  // 浏览器开发预览：?window=dock 直接预览 Dock 窗口 UI
  // Browser dev preview: ?window=dock previews the dock window UI
  useEffect(() => {
    if (!isTauri()) {
      const query = new URLSearchParams(window.location.search).get("window");
      setEntry(query === "dock" ? "dock" : "main");
      return;
    }
    // getCurrentWindow() 在 Tauri v2 中为同步调用
    // getCurrentWindow() is synchronous in Tauri v2
    try {
      const label = getCurrentWindow().label;
      setEntry(label === "dock" ? "dock" : "main");
    } catch {
      setEntry("main");
    }
  }, []);

  if (entry === null) return null;

  // Dock 窗口：弧形插件栏（docs/DOCK.md）
  // Dock window: arc-shaped plugin bar (docs/DOCK.md)
  if (entry === "dock") {
    return <DockApp />;
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<PanelLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="/plugins" element={<Plugins />} />
          <Route path="/store" element={<Store />} />
          <Route path="/workflow" element={<Workflow />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/settings" element={<Settings />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
