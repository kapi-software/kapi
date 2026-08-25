// 强调色纯函数单元测试
// Unit tests for the accent color pure functions
import { describe, it, expect } from 'vitest'
import {
  ACCENT_PRESETS,
  DEFAULT_ACCENT,
  isValidHexColor,
  normalizeHexColor,
  pickForeground,
  accentVars,
} from '@/lib/theme'

describe('hex 校验 / isValidHexColor', () => {
  it('接受 #RGB 与 #RRGGBB / Accepts #RGB and #RRGGBB', () => {
    expect(isValidHexColor('#007AFF')).toBe(true)
    expect(isValidHexColor('#0AF')).toBe(true)
    expect(isValidHexColor('#0af')).toBe(true)
  })

  it('拒绝非法格式 / Rejects invalid formats', () => {
    expect(isValidHexColor('007AFF')).toBe(false) // 缺少 # / missing #
    expect(isValidHexColor('#12345')).toBe(false) // 5 位 / 5 digits
    expect(isValidHexColor('#GGGGGG')).toBe(false) // 非 hex 字符 / non-hex chars
    expect(isValidHexColor('')).toBe(false)
  })
})

describe('hex 归一化 / normalizeHexColor', () => {
  it('展开 3 位缩写并转小写 / Expands 3-digit shorthand, lowercases', () => {
    expect(normalizeHexColor('#0AF')).toBe('#00aaff')
    expect(normalizeHexColor('#7C3AED')).toBe('#7c3aed')
  })

  it('非法输入返回 null / Returns null on invalid input', () => {
    expect(normalizeHexColor('nope')).toBeNull()
    expect(normalizeHexColor('#12')).toBeNull()
  })
})

describe('前景色选择 / pickForeground', () => {
  it('暗底返回白色 / Dark colors get white text', () => {
    // 默认蓝 #007AFF 亮度约 0.21
    // Default blue #007AFF has luminance ≈ 0.21
    expect(pickForeground('#007AFF')).toBe('#ffffff')
    expect(pickForeground('#000000')).toBe('#ffffff')
  })

  it('亮底返回深色 / Bright colors get dark text', () => {
    expect(pickForeground('#F59E0B')).toBe('#0a0a0a')
    expect(pickForeground('#FFFFFF')).toBe('#0a0a0a')
  })
})

describe('CSS 变量集 / accentVars', () => {
  it('生成完整变量集 / Produces the full variable set', () => {
    const vars = accentVars('#7C3AED')
    expect(vars).toEqual({
      '--primary': '#7c3aed',
      '--primary-foreground': '#ffffff',
      '--ring': '#7c3aed',
      '--sidebar-primary': '#7c3aed',
      '--sidebar-ring': '#7c3aed',
    })
  })

  it('非法输入回退默认色 / Falls back to the default accent', () => {
    expect(accentVars('garbage')['--primary']).toBe(DEFAULT_ACCENT)
  })

  it('全部预设色生成合法变量 / All presets produce valid vars', () => {
    // 预设色不能回退到默认值（否则色板选择失效）
    // Presets must not silently fall back to the default
    for (const preset of ACCENT_PRESETS) {
      expect(accentVars(preset)['--primary']).toBe(preset.toLowerCase())
    }
  })
})
