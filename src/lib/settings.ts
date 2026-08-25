// 设置纯逻辑：默认值、原始值解析、主题解析（无副作用，可单元测试）
// Settings pure logic: defaults, raw parsing, theme resolution (side-effect free, testable)
// 设置项清单见 docs/PANEL.md

// 主题模式
// Theme mode
export type ThemeMode = 'light' | 'dark' | 'system'

// 应用设置类型，与 migrations/002_defaults.sql 种子项一一对应
// App settings type, mirroring the seeds in 002_defaults.sql
export interface AppSettings {
  // 通用 / General
  language: string
  auto_start: boolean
  check_update: boolean
  // 主题 / Theme
  theme: ThemeMode
  accent_color: string
  // Dock
  dock_enabled: boolean
  dock_hotzone_width: number
  dock_animation_speed: 'slow' | 'medium' | 'fast'
  dock_expand_delay: number
  dock_auto_hide_delay: number
  dock_visible_items: number
  dock_position: 'right' | 'left'
  // 插件 / Plugin
  plugin_auto_update: boolean
  plugin_sandbox_strict: boolean
  plugin_log_level: 'debug' | 'info' | 'warn' | 'error'
}

// 默认设置（与 002_defaults.sql 保持一致）
// Defaults matching 002_defaults.sql
export const DEFAULT_SETTINGS: AppSettings = {
  language: 'zh-CN',
  auto_start: false,
  check_update: true,
  theme: 'system',
  accent_color: '#007AFF',
  dock_enabled: true,
  dock_hotzone_width: 12,
  dock_animation_speed: 'medium',
  dock_expand_delay: 0,
  dock_auto_hide_delay: 3000,
  dock_visible_items: 9,
  dock_position: 'right',
  plugin_auto_update: false,
  plugin_sandbox_strict: true,
  plugin_log_level: 'info',
}

// 解析数据库原始设置：
// 仅接受已知 key；值按 JSON 解析；解析失败或类型不符时回退默认值
// Parse raw settings from the settings table:
// known keys only, values parsed as JSON, falling back to defaults on failure or type mismatch
// parseRawSettings({ theme: '"dark"', unknown: '1' }) => { ...DEFAULT_SETTINGS, theme: 'dark' }
export function parseRawSettings(
  raw: Record<string, string>,
  defaults: AppSettings = DEFAULT_SETTINGS
): AppSettings {
  const result: AppSettings = { ...defaults }

  for (const key of Object.keys(defaults) as Array<keyof AppSettings>) {
    const value = raw[key]
    if (value === undefined) continue

    try {
      const parsed = JSON.parse(value) as unknown
      // 类型守卫：与默认值类型一致才采纳
      // Adopt only if the type matches the default
      if (typeof parsed === typeof defaults[key]) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ;(result as any)[key] = parsed
      }
    } catch {
      // 非法 JSON：保留默认值
      // Invalid JSON: keep the default
    }
  }

  return result
}

// 解析主题为 html class：
// system 模式下的明暗由操作系统决定（prefersDark），浅色返回 null
// Resolve the theme to an html class; 'dark' or null (light)
// resolveThemeClass('dark') => 'dark'; resolveThemeClass('system', true) => 'dark'
export function resolveThemeClass(mode: ThemeMode, prefersDark = false): 'dark' | null {
  if (mode === 'dark') return 'dark'
  if (mode === 'light') return null
  return prefersDark ? 'dark' : null
}
