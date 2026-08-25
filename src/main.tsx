/**
 * @file main.tsx
 * @description 前端入口：i18n → React 挂载 → 全局样式
 * Frontend entry: i18n → React mount → global styles
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始化（替换 Tauri 模板）
 * - 2026-08-25: 挂载 i18next
 */

// i18n 必须先于 App 初始化（App 内使用 useTranslation）
// i18n must initialize before App (App uses useTranslation)
import '@/i18n'
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
