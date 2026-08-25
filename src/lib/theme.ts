// 强调色纯逻辑：hex 校验、亮度计算、CSS 变量集生成（无副作用，可单元测试）
// Accent color pure logic: hex validation, luminance, CSS variable set (side-effect free, testable)
// 设计见 docs/PANEL.md §4.1：accent_color → 切换 CSS 变量，实时生效

// 预设强调色（设置页色板）
// Preset accent colors (settings palette)
export const ACCENT_PRESETS = [
  '#007AFF', // 蓝 / Blue（默认）
  '#7C3AED', // 紫 / Violet
  '#E11D48', // 玫红 / Rose
  '#F59E0B', // 琥珀 / Amber
  '#10B981', // 翠绿 / Emerald
  '#64748B', // 石板灰 / Slate
] as const

// 默认强调色（与 002_defaults.sql / DEFAULT_SETTINGS 保持一致）
// Default accent (kept in sync with 002_defaults.sql / DEFAULT_SETTINGS)
export const DEFAULT_ACCENT = '#007AFF'

// 校验十六进制颜色：支持 #RGB 与 #RRGGBB（大小写不限）
// Validate a hex color: #RGB or #RRGGBB, case-insensitive
// isValidHexColor('#007AFF') => true; isValidHexColor('007aff') => false; isValidHexColor('#12345') => false
export function isValidHexColor(input: string): boolean {
  return /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(input)
}

// 归一化十六进制颜色：#RGB → #RRGGBB，统一小写；非法输入返回 null
// Normalize a hex color: #RGB → #RRGGBB, lowercased; null on invalid input
// normalizeHexColor('#0AF') => '#00aaff'; normalizeHexColor('bad') => null
export function normalizeHexColor(input: string): string | null {
  if (!isValidHexColor(input)) return null
  let hex = input.slice(1).toLowerCase()
  if (hex.length === 3) hex = hex
    .split('')
    .map((ch) => ch + ch)
    .join('')
  return `#${hex}`
}

// 计算 ITU-R BT.709 相对亮度（0–1），输入为已归一化的 #rrggbb
// ITU-R BT.709 relative luminance (0–1) from a normalized #rrggbb
function relativeLuminance(hex: string): number {
  const r = parseInt(hex.slice(1, 3), 16) / 255
  const g = parseInt(hex.slice(3, 5), 16) / 255
  const b = parseInt(hex.slice(5, 7), 16) / 255
  return 0.2126 * r + 0.7152 * g + 0.0722 * b
}

// 根据亮度选择前景色：亮底配深字、暗底配白字（WCAG 对比度经验阈值 0.55）
// Pick a foreground by luminance: dark text on bright colors, white on dark ones
// pickForeground('#007AFF') => '#ffffff'; pickForeground('#F59E0B') => '#0a0a0a'
export function pickForeground(hex: string): string {
  return relativeLuminance(hex) > 0.55 ? '#0a0a0a' : '#ffffff'
}

// 生成强调色相关的 CSS 变量集；非法输入回退默认色
// Build the accent CSS variable set; falls back to the default accent on invalid input
// accentVars('#7C3AED') => { '--primary': '#7c3aed', '--primary-foreground': '#ffffff', '--ring': '#7c3aed', '--sidebar-primary': '#7c3aed', '--sidebar-ring': '#7c3aed' }
export function accentVars(input: string): Record<string, string> {
  const hex = normalizeHexColor(input) ?? DEFAULT_ACCENT
  return {
    '--primary': hex,
    '--primary-foreground': pickForeground(hex),
    '--ring': hex,
    '--sidebar-primary': hex,
    '--sidebar-ring': hex,
  }
}
