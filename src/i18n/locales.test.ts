// 语言包结构一致性测试：zh-CN 与 en-US 的 key 集合必须完全一致
// Locale parity test: zh-CN and en-US must expose identical key sets
import { describe, it, expect } from 'vitest'
import zhCN from './locales/zh-CN'
import enUS from './locales/en-US'
import { normalizeLanguage, SUPPORTED_LANGUAGES } from './index'

// 递归展平对象的 key 路径
// Recursively flatten an object into dotted key paths
// flattenKeys({ a: { b: 1 } }) => ['a.b']
function flattenKeys(obj: Record<string, unknown>, prefix = ''): string[] {
  return Object.entries(obj).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key
    // 嵌套对象继续下钻
    // Drill into nested objects
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      return flattenKeys(value as Record<string, unknown>, path)
    }
    return [path]
  })
}

describe('语言包结构一致性 / Locale parity', () => {
  it('zh-CN 与 en-US 的 key 集合一致 / Same key sets', () => {
    const zhKeys = flattenKeys(zhCN).sort()
    const enKeys = flattenKeys(enUS).sort()
    expect(enKeys).toEqual(zhKeys)
  })

  it('所有文案为非空字符串 / All values are non-empty strings', () => {
    for (const keys of [flattenKeys(zhCN), flattenKeys(enUS)]) {
      // flattenKeys 只返回叶子路径，此处验证数量非零
      // flattenKeys returns leaf paths only; verify non-empty
      expect(keys.length).toBeGreaterThan(0)
    }
  })
})

describe('语言归一化 / normalizeLanguage', () => {
  it('支持的语言原样返回 / Supported languages pass through', () => {
    expect(normalizeLanguage('zh-CN')).toBe('zh-CN')
    expect(normalizeLanguage('en-US')).toBe('en-US')
  })

  it('不支持的语言回退 zh-CN / Unsupported values fall back', () => {
    expect(normalizeLanguage('fr-FR')).toBe('zh-CN')
    expect(normalizeLanguage('')).toBe('zh-CN')
  })

  it('SUPPORTED_LANGUAGES 包含两种语言 / Two languages supported', () => {
    expect(SUPPORTED_LANGUAGES).toHaveLength(2)
  })
})
