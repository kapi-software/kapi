/**
 * @file settings.ts
 * @description 设置纯逻辑：默认值、原始值解析、主题解析（无副作用，可单元测试）
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始实现
 */

/**
 * 应用设置类型 / App settings type
 * 与 migrations/002_defaults.sql 的种子项一一对应（plan §8.2）
 */
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

/** 主题模式 / Theme mode */
export type ThemeMode = 'light' | 'dark' | 'system'

/** 默认设置（与 002_defaults.sql 保持一致）/ Defaults matching the SQL seed */
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

/**
 * 解析数据库原始设置 / Parse raw settings from the settings table
 *
 * 规则：仅接受已知 key；值按 JSON 解析；解析失败或类型不符时回退默认值。
 * 纯函数，供 store 与单元测试使用。
 *
 * @param raw - settings 表的 key → JSON 字符串映射 / Raw key → JSON string map
 * @param defaults - 回退默认值（默认 DEFAULT_SETTINGS）/ Fallback defaults
 * @returns 合并后的完整设置 / Merged complete settings
 *
 * @example
 * parseRawSettings({ theme: '"dark"', unknown_key: '1' })
 * // => { ...DEFAULT_SETTINGS, theme: 'dark' }   // 未知 key 被忽略
 */
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
      // 类型守卫：与默认值类型一致才采纳 / Adopt only if type matches default
      if (typeof parsed === typeof defaults[key]) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ;(result as any)[key] = parsed
      }
    } catch {
      // 非法 JSON：保留默认值 / Invalid JSON: keep default
    }
  }

  return result
}

/**
 * 解析主题为 html class / Resolve theme to an html class
 *
 * system 模式下的明暗由操作系统决定，此处仅返回 'dark' 或 null（跟随浅色），
 * 具体的 matchMedia 监听由副作用层（React hook）处理。
 *
 * @param mode - 主题模式 / Theme mode
 * @param prefersDark - system 模式下系统是否偏好深色 / OS preference (for 'system')
 * @returns 'dark' 或 null（浅色）/ 'dark' or null (light)
 *
 * @example
 * resolveThemeClass('dark')            // => 'dark'
 * resolveThemeClass('system', true)    // => 'dark'
 * resolveThemeClass('system', false)   // => null
 */
export function resolveThemeClass(mode: ThemeMode, prefersDark = false): 'dark' | null {
  if (mode === 'dark') return 'dark'
  if (mode === 'light') return null
  return prefersDark ? 'dark' : null
}
