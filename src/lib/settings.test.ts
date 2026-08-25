// 设置解析纯函数单元测试
// Unit tests for the settings parsing pure functions
import { describe, it, expect } from 'vitest'
import {
  DEFAULT_SETTINGS,
  parseRawSettings,
  resolveThemeClass,
} from '@/lib/settings'

describe('设置解析 / parseRawSettings', () => {
  it('空输入返回完整默认值 / Empty input returns full defaults', () => {
    expect(parseRawSettings({})).toEqual(DEFAULT_SETTINGS)
  })

  it('合法 JSON 覆盖默认值 / Valid JSON overrides defaults', () => {
    const result = parseRawSettings({
      theme: '"dark"',
      dock_enabled: 'false',
      dock_hotzone_width: '18',
    })
    expect(result.theme).toBe('dark')
    expect(result.dock_enabled).toBe(false)
    expect(result.dock_hotzone_width).toBe(18)
    // 未提供的 key 保持默认
    // Untouched keys keep defaults
    expect(result.language).toBe(DEFAULT_SETTINGS.language)
  })

  it('非法 JSON 回退默认值 / Invalid JSON falls back to defaults', () => {
    const result = parseRawSettings({ theme: 'not-json{' })
    expect(result.theme).toBe(DEFAULT_SETTINGS.theme)
  })

  it('类型不符回退默认值 / Type-mismatched value falls back', () => {
    // 布尔设置收到字符串 'true'（JSON 编码错误）→ 不采纳
    // A boolean setting receiving the string 'true' is rejected
    const result = parseRawSettings({ dock_enabled: '"true"' })
    expect(result.dock_enabled).toBe(DEFAULT_SETTINGS.dock_enabled)
  })

  it('未知 key 被忽略 / Unknown keys are ignored', () => {
    const result = parseRawSettings({ evil_key: '1', another: '"x"' })
    expect(result).toEqual(DEFAULT_SETTINGS)
  })
})

describe('主题解析 / resolveThemeClass', () => {
  it('显式深色 / Explicit dark', () => {
    expect(resolveThemeClass('dark')).toBe('dark')
  })

  it('显式浅色 / Explicit light', () => {
    expect(resolveThemeClass('light')).toBeNull()
  })

  it('跟随系统 / System follows OS preference', () => {
    expect(resolveThemeClass('system', true)).toBe('dark')
    expect(resolveThemeClass('system', false)).toBeNull()
  })
})
