/**
 * @file zh-CN.ts
 * @description 简体中文语言包
 * Simplified Chinese locale pack
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 随 i18n 初始化创建
 */

export default {
  app: {
    name: 'Kapi',
  },
  nav: {
    home: '首页',
    plugins: '插件',
    store: '插件市场',
    workflow: '工作流',
    logs: '日志',
    settings: '设置',
  },
  topbar: {
    minimize: '最小化',
    maximize: '最大化 / 还原',
    close: '关闭',
  },
  dashboard: {
    title: '首页 / Dashboard',
    subtitle: 'Kapi 插件化桌面应用 —— Phase 1：项目初始化与数据库层',
    dbTitle: '数据库链路 / Database Chain',
    dbDesc: 'Rust 迁移（001 建表 + 002 默认设置）→ tauri-plugin-sql → 前端 db.ts',
    statusOk: '正常 · {{count}} 项设置',
    statusChecking: '检查中…',
    statusBrowser: '浏览器环境',
    statusFailed: '失败',
    browserHint:
      '当前在浏览器中运行，无 Tauri IPC。请使用 pnpm tauri dev 启动完整应用。',
    failedHint: '数据库连接失败，请查看控制台输出与 src-tauri/src/db.rs 迁移配置。',
    theme: '主题 / theme',
    language: '语言 / language',
    dockEnabled: 'Dock 启用 / dock_enabled',
    dockVisible: 'Dock 可见数 / dock_visible_items',
    sandboxStrict: '沙箱严格 / plugin_sandbox_strict',
    accentColor: '强调色 / accent_color',
    roadmapTitle: '路线图 / Roadmap',
    roadmapDesc: '按 docs/plan.MD §10.1 推进',
    phase1: 'Phase 1 · 项目初始化 + 数据库层（当前）',
    phase2: 'Phase 2 · 主面板框架 + 设置页',
    phase3: 'Phase 3 · Dock 侧边栏（仅唤醒）',
    phase4: 'Phase 4 · 插件系统（WASM）',
    phase57: 'Phase 5-7 · 市场 / 工作流 / 打磨',
  },
  settings: {
    title: '设置 / Settings',
    subtitle: '统一存储于 SQLite settings 表（plan §8）',
    themeTitle: '主题 / Theme',
    themeDesc: 'light / dark / system，实时切换 html.dark',
    themeLight: '浅色',
    themeDark: '深色',
    themeSystem: '跟随系统',
    dockTitle: 'Dock 侧边栏 / Dock Sidebar',
    dockDesc: '关闭后隐藏右缘触发器（实时生效需 Phase 3 的 dock_service 接入）',
    dockOn: '已启用',
    dockOff: '已关闭',
    languageTitle: '语言 / Language',
    languageDesc: '界面语言，持久化于 settings.language',
    reset: '恢复默认 / Reset',
  },
  logs: {
    title: '日志 / Logs',
    subtitle: 'system_logs 最近 {{count}} 条',
    loading: '加载中…',
    empty: '暂无日志 / No logs yet（应用运行中产生的事件会写入 system_logs）',
  },
  plugins: {
    title: '插件 / Plugins',
    desc: '已安装插件列表、启停、运行模式切换（embedded / independent / headless）与排序。待 Phase 4 插件系统实现。',
  },
  store: {
    title: '插件市场 / Store',
    desc: 'GitHub 来源浏览、安装 / 更新 / 卸载。待 Phase 5 实现。',
  },
  workflow: {
    title: '工作流 / Workflow',
    desc: '插件间数据联动编排（如：剪贴板监听 → 代码美化保存 → 截图生成）。待 Phase 6 实现（DAG 引擎 + React Flow 编辑器）。',
  },
}
