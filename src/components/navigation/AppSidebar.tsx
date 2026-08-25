// 应用侧边栏：官方 Sidebar 组件（inset + 图标折叠模式）+ 分组导航
// App sidebar: official Sidebar component (inset + icon collapse) with grouped nav
// 分组定义见 docs/PANEL.md §1；面包屑复用本文件的 NAV_GROUPS
// Group definitions follow docs/PANEL.md §1; the breadcrumb reuses NAV_GROUPS below
import { useTranslation } from "react-i18next";
import { NavLink, useLocation } from "react-router-dom";
import {
  Home,
  Puzzle,
  PackageOpen,
  Workflow as WorkflowIcon,
  ScrollText,
  Settings as SettingsIcon,
  Layers,
  type LucideIcon,
} from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";

// 导航分组与路由项（概览 / 插件 / 自动化 / 系统）
// Nav groups and route items (overview / plugins / automation / system)
export const NAV_GROUPS: Array<{
  groupKey: string;
  items: Array<{ to: string; labelKey: string; icon: LucideIcon; end: boolean }>;
}> = [
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

// 展平的导航项（面包屑查找用）
// Flattened nav items (for breadcrumb lookup)
export const NAV_ITEMS = NAV_GROUPS.flatMap((g) => g.items);

// 判断路由是否激活：end 项精确匹配，其余前缀匹配子路由
// Route active check: exact for end items, prefix match for child routes
// isNavItemActive('/plugins', '/plugins', true) => false; isNavItemActive('/plugins/x', '/plugins', false) => true
export function isNavItemActive(pathname: string, to: string, end: boolean): boolean {
  if (end) return pathname === to
  return pathname === to || pathname.startsWith(`${to}/`)
}

export function AppSidebar() {
  const { t } = useTranslation();
  const { pathname } = useLocation();

  return (
    <Sidebar collapsible="icon" variant="inset">
      {/* 应用名：点击回首页；图标折叠模式下自动缩为 Logo */}
      {/* App name: click returns home; collapses to the logo in icon mode */}
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <NavLink to="/">
                <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                  <Layers className="size-4" />
                </div>
                <div className="grid flex-1 text-left text-sm leading-tight">
                  <span className="truncate font-semibold">{t("app.name")}</span>
                  <span className="truncate text-xs">{t("app.tagline")}</span>
                </div>
              </NavLink>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        {NAV_GROUPS.map(({ groupKey, items }) => (
          <SidebarGroup key={groupKey}>
            <SidebarGroupLabel>{t(groupKey)}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {items.map(({ to, labelKey, icon: Icon, end }) => (
                  <SidebarMenuItem key={to}>
                    {/* tooltip 仅供图标折叠态显示完整名称 */}
                    {/* The tooltip only shows the full label in icon-collapsed mode */}
                    <SidebarMenuButton
                      isActive={isNavItemActive(pathname, to, end)}
                      tooltip={t(labelKey)}
                      asChild
                    >
                      <NavLink to={to} end={end}>
                        <Icon />
                        <span>{t(labelKey)}</span>
                      </NavLink>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                ))}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        ))}
      </SidebarContent>

      {/* 右缘隐形轨道：悬停可收起/展开，与顶栏触发按钮等效 */}
      {/* Invisible edge rail: hover to toggle, equivalent to the header trigger */}
      {/* <SidebarRail /> */}
    </Sidebar>
  );
}
