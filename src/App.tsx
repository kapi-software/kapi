// 应用根组件：窗口分流、主题与语言应用、主面板布局与路由
// Root component: window routing, theme & language application, panel layout and routes
import { useEffect, useState, type CSSProperties } from "react";
import { BrowserRouter, Routes, Route, Outlet, useNavigate } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@/lib/tauri";
import { resolveThemeClass } from "@/lib/settings";
import type { AppSettings } from "@/lib/settings";
import { accentVars } from "@/lib/theme";
import { normalizeLanguage } from "@/i18n";
import i18n from "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { SidebarProvider } from "@/components/ui/sidebar";
import { AppSidebar } from "@/components/navigation/AppSidebar";
import { TopBar } from "@/components/navigation/TopBar";
import DockApp from "@/dock/Dock";
import Dashboard from "@/pages/Dashboard";
import Plugins from "@/pages/Plugins";
import Store from "@/pages/Store";
import Workflow from "@/pages/Workflow";
import Logs from "@/pages/Logs";
import Settings from "@/pages/Settings";
import PluginEmbedView from "@/pages/PluginEmbedView";
import PluginWindowShell from "@/pages/PluginWindowShell";
import { Toaster } from "@/components/ui/sonner";

// 主面板布局（shadcn sidebar-16）：全宽顶栏置顶 + 经典侧边栏 + 滚动内容区
// Panel layout (shadcn sidebar-16): full-width header on top, classic sidebar, scrolling content
function PanelLayout() {
  const navigate = useNavigate();

  // 托盘「设置」菜单 → Rust 发来 app:navigate，主窗口内跳转对应路由
  // Tray "Settings" menu → Rust emits app:navigate; the main window routes accordingly
  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<string>("app:navigate", (e) => {
      navigate(e.payload);
    });
    return () => {
      un.then((f) => f());
    };
  }, [navigate]);

  // launch_plugin（embedded 模式）→ Rust 发来 plugin:navigate，路由到内嵌视图
  // launch_plugin (embedded mode) → Rust emits plugin:navigate; route to the embed view
  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<string>("plugin:navigate", (e) => {
      navigate(`/plugin/${e.payload}`);
    });
    return () => {
      un.then((f) => f());
    };
  }, [navigate]);

  return (
    // sidebar-16：纵向排列（顶栏在上），侧边栏固定面板经 --header-height 下移
    // sidebar-16: column layout (header first); the sidebar's fixed panel shifts below via --header-height
    // 宽度覆盖：展开 16rem→12rem，图标态保持 3rem（仅此处声明，官方组件不改）
    // Width overrides: expanded 16rem→12rem, icon rail keeps 3rem (declared only here)
    <SidebarProvider
      className="flex h-svh flex-col"
      style={
        {
          // 顶栏高度 48px（官方 demo 为 32px，偏挤；上一版 56px 偏高）
          // Header height 48px (official demo 32px feels cramped; 56px felt tall)
          "--header-height": "calc(var(--spacing) * 12)",
          "--sidebar-width": "12rem",
          "--sidebar-width-icon": "3rem",
        } as CSSProperties
      }
    >
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <AppSidebar />
        <main className="min-w-0 flex-1 overflow-y-auto overscroll-contain p-4 md:p-6">
          <Outlet />
        </main>
      </div>
      <Toaster />
    </SidebarProvider>
  );
}

// 应用根组件：按窗口 label 分流
// Root component: routes by window label
export default function App() {
  const [entry, setEntry] = useState<"main" | "dock" | "plugin" | null>(null);
  const theme = useSettingsStore((s) => s.settings.theme);
  const language = useSettingsStore((s) => s.settings.language);
  const dockEnabled = useSettingsStore((s) => s.settings.dock_enabled);
  const dockPosition = useSettingsStore((s) => s.settings.dock_position);
  const dockHotzoneWidth = useSettingsStore((s) => s.settings.dock_hotzone_width);
  const dockExpandDelay = useSettingsStore((s) => s.settings.dock_expand_delay);
  const loadSettings = useSettingsStore((s) => s.loadSettings);

  // 启动时加载设置（含数据库连接）
  // Load settings on startup (includes the DB connection)
  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  // 跨窗口设置同步：任意窗口变更后广播，本窗口 store 补丁（docs/PANEL.md §4.1）
  // Cross-window settings sync: broadcasts on change; the local store patches itself
  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<{ key: string; value: unknown } | { settings: AppSettings }>(
      "settings:changed",
      (e) => {
        const p = e.payload;
        if ("settings" in p) {
          useSettingsStore.setState({ settings: p.settings });
          return;
        }
        const cur = useSettingsStore.getState().settings;
        // 类型防御：与当前值类型一致才采纳
        // Type guard: adopt only when the type matches the current value
        if (p.key in cur && typeof p.value === typeof cur[p.key as keyof AppSettings]) {
          useSettingsStore.setState({
            settings: { ...cur, [p.key]: p.value } as AppSettings,
          });
        }
      }
    );
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Dock 配置推送（docs/PANEL.md §4.1 实时生效）：主窗口设置页变更 → Rust 轮询服务
  // Dock config push (live application): settings changes in the panel reach the Rust polling service
  // 放在 App 根组件：主面板与 dock 窗口都会执行，不依赖某一窗口存活
  // Lives in the App root: both panel and dock windows run it, independent of either surviving
  useEffect(() => {
    if (!isTauri()) return;
    invoke("dock_set_config", {
      enabled: dockEnabled,
      position: dockPosition,
      hotzoneWidth: dockHotzoneWidth,
      expandDelayMs: dockExpandDelay,
    }).catch((e) => console.error("dock_set_config 失败 / failed:", e));
  }, [dockEnabled, dockPosition, dockHotzoneWidth, dockExpandDelay]);

  // 托盘菜单语言推送：跟随 settings.language 实时重建托盘文案
  // Tray language push: rebuild tray labels live with settings.language
  useEffect(() => {
    if (!isTauri()) return;
    invoke("tray_set_language", { language }).catch((e) =>
      console.error("tray_set_language 失败 / failed:", e)
    );
  }, [language]);

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
      // 插件独立窗口：label 形如 plugin-<id>（Rust 按 manifest.window 创建）
      // Plugin independent windows use labels like plugin-<id> (created by Rust)
      setEntry(
        label === "dock" ? "dock" : label.startsWith("plugin-") ? "plugin" : "main"
      );
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

  // 插件独立窗口：裸 PluginHost 壳（/plugin-window/:id）
  // Plugin independent window: a bare PluginHost shell (/plugin-window/:id)
  if (entry === "plugin") {
    return (
      <BrowserRouter>
        <Routes>
          <Route path="/plugin-window/:id" element={<PluginWindowShell />} />
          <Route path="*" element={<PluginWindowShell />} />
        </Routes>
      </BrowserRouter>
    );
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route element={<PanelLayout />}>
          <Route index element={<Dashboard />} />
          <Route path="/plugins" element={<Plugins />} />
          <Route path="/plugin/:id" element={<PluginEmbedView />} />
          <Route path="/store" element={<Store />} />
          <Route path="/workflow" element={<Workflow />} />
          <Route path="/logs" element={<Logs />} />
          <Route path="/settings" element={<Settings />} />
        </Route>
      </Routes>
    </BrowserRouter>
  );
}
