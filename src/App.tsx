/**
 * @file App.tsx
 * @description 应用根组件：窗口分流、主题与语言应用、主面板布局与路由
 * Root component: window routing, theme & language application, panel layout and routes
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 主面板骨架（左侧导航 + 右侧内容区）
 * - 2026-08-25: 接入 i18n（导航文案 t() 化 + 语言同步 effect）
 */

import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route, Outlet, NavLink } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  Home,
  Puzzle,
  PackageOpen,
  Workflow as WorkflowIcon,
  ScrollText,
  Settings as SettingsIcon,
} from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauri } from "@/lib/tauri";
import { resolveThemeClass } from "@/lib/settings";
import { normalizeLanguage } from "@/i18n";
import i18n from "@/i18n";
import { useSettingsStore } from "@/stores/settings";
import { TopBar } from "@/components/navigation/TopBar";
import Dashboard from "@/pages/Dashboard";
import Plugins from "@/pages/Plugins";
import Store from "@/pages/Store";
import Workflow from "@/pages/Workflow";
import Logs from "@/pages/Logs";
import Settings from "@/pages/Settings";

/** 导航路由键 → i18n key 映射 / Nav route keys mapped to i18n keys（plan §4.1） */
const NAV_ITEMS = [
  { to: "/", labelKey: "nav.home", icon: Home, end: true },
  { to: "/plugins", labelKey: "nav.plugins", icon: Puzzle },
  { to: "/store", labelKey: "nav.store", icon: PackageOpen },
  { to: "/workflow", labelKey: "nav.workflow", icon: WorkflowIcon },
  { to: "/logs", labelKey: "nav.logs", icon: ScrollText },
  { to: "/settings", labelKey: "nav.settings", icon: SettingsIcon },
];

/**
 * 主面板布局：顶栏 + 左侧导航 + 内容区
 * Panel layout: top bar + left nav + content area
 *
 * Phase 2 将替换为 shadcn/ui Sidebar 组件
 * Will be replaced with the shadcn/ui Sidebar component in Phase 2
 */
function PanelLayout() {
  const { t } = useTranslation();

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-14 flex-col items-center gap-1 border-r py-3 md:w-52 md:items-stretch md:px-3">
          {NAV_ITEMS.map(({ to, labelKey, icon: Icon, end }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              className={({ isActive }) =>
                `flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
                  isActive
                    ? "bg-accent text-accent-foreground font-medium"
                    : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                }`
              }
            >
              <Icon className="size-4 shrink-0" />
              <span className="hidden md:inline">{t(labelKey)}</span>
            </NavLink>
          ))}
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}

/** 应用根组件：按窗口 label 分流 / Root component routing by window label */
export default function App() {
  const [entry, setEntry] = useState<"main" | "dock" | null>(null);
  const theme = useSettingsStore((s) => s.settings.theme);
  const language = useSettingsStore((s) => s.settings.language);
  const loadSettings = useSettingsStore((s) => s.loadSettings);

  // 启动时加载设置（含数据库连接）
  // Load settings on startup (includes DB connection)
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

  // 窗口分流：dock 窗口渲染 Dock UI（Phase 3 实现），其余渲染主面板
  // Window routing: dock window renders the Dock UI (Phase 3), others the panel
  useEffect(() => {
    if (!isTauri()) {
      setEntry("main");
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

  // Dock 窗口：透明占位，Phase 3 按 plan §5 实现弧形 UI
  // Dock window: transparent placeholder; arc UI lands in Phase 3 (plan §5)
  if (entry === "dock") {
    return <div className="h-screen w-screen bg-transparent" />;
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
