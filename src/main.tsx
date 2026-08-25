// 前端入口：i18n → React 挂载 → 全局样式
// Frontend entry: i18n → React mount → global styles
// i18n 必须先于 App 初始化（App 内使用 useTranslation）
// i18n must initialize before App (App uses useTranslation)
import '@/i18n'
import React from "react";
import ReactDOM from "react-dom/client";
import { TooltipProvider } from "@/components/ui/tooltip";
import App from "./App";
import "./index.css";

// TooltipProvider 必须包住 Sidebar（图标折叠态的菜单提示依赖它）
// TooltipProvider must wrap the Sidebar (icon-collapsed menu hints depend on it)
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TooltipProvider>
      <App />
    </TooltipProvider>
  </React.StrictMode>,
);
