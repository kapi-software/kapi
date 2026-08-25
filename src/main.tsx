// 前端入口：i18n → React 挂载 → 全局样式
// Frontend entry: i18n → React mount → global styles
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
