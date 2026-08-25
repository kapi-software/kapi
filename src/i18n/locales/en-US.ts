/**
 * @file en-US.ts
 * @description English (US) locale pack
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
    home: 'Home',
    plugins: 'Plugins',
    store: 'Store',
    workflow: 'Workflow',
    logs: 'Logs',
    settings: 'Settings',
  },
  topbar: {
    minimize: 'Minimize',
    maximize: 'Maximize / Restore',
    close: 'Close',
  },
  dashboard: {
    title: 'Dashboard',
    subtitle: 'Kapi plugin-oriented desktop app — Phase 1: project init & database layer',
    dbTitle: 'Database Chain',
    dbDesc: 'Rust migrations (001 tables + 002 defaults) → tauri-plugin-sql → frontend db.ts',
    statusOk: 'OK · {{count}} settings',
    statusChecking: 'Checking…',
    statusBrowser: 'Browser env',
    statusFailed: 'Failed',
    browserHint:
      'Running in a browser without Tauri IPC. Launch the full app with pnpm tauri dev.',
    failedHint: 'Database connection failed. Check the console output and src-tauri/src/db.rs.',
    theme: 'theme',
    language: 'language',
    dockEnabled: 'dock_enabled',
    dockVisible: 'dock_visible_items',
    sandboxStrict: 'plugin_sandbox_strict',
    accentColor: 'accent_color',
    roadmapTitle: 'Roadmap',
    roadmapDesc: 'Follows docs/plan.MD §10.1',
    phase1: 'Phase 1 · Project init + database layer (current)',
    phase2: 'Phase 2 · Panel framework + settings page',
    phase3: 'Phase 3 · Dock sidebar (launcher only)',
    phase4: 'Phase 4 · Plugin system (WASM)',
    phase57: 'Phase 5-7 · Store / workflow / polish',
  },
  settings: {
    title: 'Settings',
    subtitle: 'Persisted in the SQLite settings table (plan §8)',
    themeTitle: 'Theme',
    themeDesc: 'light / dark / system, toggles html.dark instantly',
    themeLight: 'Light',
    themeDark: 'Dark',
    themeSystem: 'System',
    dockTitle: 'Dock Sidebar',
    dockDesc: 'Hides the right-edge trigger when off (live toggle lands in Phase 3 dock_service)',
    dockOn: 'Enabled',
    dockOff: 'Disabled',
    languageTitle: 'Language',
    languageDesc: 'UI language, persisted as settings.language',
    reset: 'Reset',
  },
  logs: {
    title: 'Logs',
    subtitle: 'Latest {{count}} entries from system_logs',
    loading: 'Loading…',
    empty: 'No logs yet — events will be written to system_logs while the app runs',
  },
  plugins: {
    title: 'Plugins',
    desc: 'Installed plugin list, enable/disable, window mode (embedded / independent / headless) and ordering. Lands in Phase 4.',
  },
  store: {
    title: 'Store',
    desc: 'Browse, install, update and uninstall plugins from GitHub. Lands in Phase 5.',
  },
  workflow: {
    title: 'Workflow',
    desc: 'Data-link orchestration between plugins (e.g. clipboard watch → beautify & save → screenshot). Lands in Phase 6 (DAG engine + React Flow editor).',
  },
}
