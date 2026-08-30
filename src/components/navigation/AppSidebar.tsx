// 应用侧边栏：分组导航 + 底部 Header (Dropdown) 切换日志/设置
// App sidebar: grouped nav + bottom Header (Dropdown) for Logs/Settings
import { useTranslation } from "react-i18next";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import {
  ChevronUp,
  Home,
  Layers,
  PackageOpen,
  Puzzle,
  ScrollText,
  Settings as SettingsIcon,
  Workflow as WorkflowIcon,
  type LucideIcon,
} from "lucide-react";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

// 主导航分组（日志/设置从底部 Dropdown 跳转，不在主菜单内）
// Main nav groups (Logs/Settings live in the footer dropdown)
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
];

// 展平的导航项（用于面包屑/路由查找）
// Flattened nav items (for breadcrumb/route lookup)
export const NAV_ITEMS = NAV_GROUPS.flatMap((g) => g.items);

// 判断路由是否激活：end 项精确匹配，其余前缀匹配子路由
// Route active check: exact for end items, prefix match for child routes
export function isNavItemActive(pathname: string, to: string, end: boolean): boolean {
  if (end) return pathname === to
  return pathname === to || pathname.startsWith(`${to}/`)
}

export function AppSidebar() {
  const { t } = useTranslation();
  const { pathname } = useLocation();
  const navigate = useNavigate();

  return (
    <Sidebar
      collapsible="icon"
      className="top-(--header-height) h-[calc(100svh-var(--header-height))]!"
    >
      <SidebarContent>
        {NAV_GROUPS.map(({ groupKey, items }) => (
          <SidebarGroup key={groupKey}>
            <SidebarGroupLabel>{t(groupKey)}</SidebarGroupLabel>
            <SidebarGroupContent>
              <SidebarMenu>
                {items.map(({ to, labelKey, icon: Icon, end }) => (
                  <SidebarMenuItem key={to}>
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

      {/* 底部：点击 Header 触发 Dropdown 跳转到日志/设置 */}
      {/* Bottom: click the header to open a Dropdown for Logs/Settings */}
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <SidebarMenuButton
                  size="lg"
                  tooltip={t("sidebar.more")}
                  className="data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
                >
                  <div className="flex aspect-square size-8 items-center justify-center rounded-lg bg-sidebar-primary text-sidebar-primary-foreground">
                    <Layers className="size-4" />
                  </div>
                  <div className="flex flex-col gap-0.5 leading-none">
                    <span className="font-semibold">{t("app.name")}</span>
                    <span className="text-xs text-muted-foreground">
                      {t("app.tagline")}
                    </span>
                  </div>
                  <ChevronUp className="ml-auto size-4" />
                </SidebarMenuButton>
              </DropdownMenuTrigger>
              <DropdownMenuContent
                className="w-(--radix-dropdown-menu-trigger-width)"
                align="start"
              >
                <DropdownMenuItem onSelect={() => navigate("/logs")}>
                  <ScrollText className="size-4" />
                  {t("nav.logs")}
                </DropdownMenuItem>
                <DropdownMenuItem onSelect={() => navigate("/settings")}>
                  <SettingsIcon className="size-4" />
                  {t("nav.settings")}
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  );
}