// 应用根组件：窗口分流、主题与语言应用、主面板布局与路由
// Root component: window routing, theme & language application, panel layout and routes
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
import { accentVars } from "@/lib/theme";
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

// 导航分组与路由项（docs/PANEL.md §1：概览 / 插件 / 自动化 / 系统）
// Nav groups and route items (docs/PANEL.md §1: overview / plugins / automation / system)
const NAV_GROUPS = [
  {
    groupKey: "nav.groupOverview",
    items: [{ to: "/", labelKey: "nav.home", icon: Home, end: true }],
  },
  {
    groupKey: "nav.groupPlugins",
    items: [
      { to: "/plugins", labelKey: "nav.plugins", icon: Puzzle, end: false },
      { to: "/store", labelKey: "nav.store", icon: PackageOpen, end: false },
    ],
  },
  {
    groupKey: "nav.groupAutomation",
    items: [{ to: "/workflow", labelKey: "nav.workflow", icon: WorkflowIcon, end: false }],
  },
  {
    groupKey: "nav.groupSystem",
    items: [
      { to: "/logs", labelKey: "nav.logs", icon: ScrollText, end: false },
      { to: "/settings", labelKey: "nav.settings", icon: SettingsIcon, end: false },
    ],
  },
];

// 主面板布局：顶栏 + 左侧导航 + 内容区
// Panel layout: top bar + left nav + content area
// Phase 2 将替换为 shadcn/ui Sidebar 组件
// To be replaced with the shadcn/ui Sidebar component in Phase 2
function PanelLayout() {
  const { t } = useTranslation();

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TopBar />
      <div className="flex min-h-0 flex-1">
        <nav className="flex w-14 flex-col gap-3 overflow-y-auto border-r py-3 md:w-52 md:px-3">
          {NAV_GROUPS.map(({ groupKey, items }) => (
            <div key={groupKey} className="flex flex-col items-center gap-1 md:items-stretch">
              {/* 分组标签仅宽栏显示；窄栏（w-14）为纯图标模式 */}
              {/* Group labels show on the wide sidebar only; the narrow rail is icon-only */}
              <span className="hidden px-3 pb-0.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground/70 md:inline">
                {t(groupKey)}
              </span>
              {items.map(({ to, labelKey, icon: Icon, end }) => (
                <NavLink
                  key={to}
                  to={to}
                  end={end}
                  title={t(labelKey)}
                  className={({ isActive }) =>
                    `flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors ${
                      isActive
                        ? "bg-accent font-medium text-accent-foreground"
                        : "text-muted-foreground hover:bg-accent/50 hover:text-foreground"
                    }`
                  }
                >
                  <Icon className="size-4 shrink-0" />
                  <span className="hidden md:inline">{t(labelKey)}</span>
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
        <main className="min-w-0 flex-1 overflow-y-auto p-6">
          <Outlet />
        </main>
      </div>
    </div>
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

  // 窗口分流：dock 窗口渲染 Dock UI（Phase 3 实现），其余渲染主面板
  // Window routing: the dock window renders the Dock UI (Phase 3), others the panel
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

  // Dock 窗口：透明占位，Phase 3 按 docs/DOCK.md 实现弧形 UI
  // Dock window: transparent placeholder; arc UI lands in Phase 3 (docs/DOCK.md)
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
