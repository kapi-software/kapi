// C3-续：把 trigger / binding config 渲染成人类可读摘要
// C3-续: render trigger / binding config as human-readable summary
// 避免在 UI 上直接展示 JSON.stringify(config)（提案 C3）
import type { TriggerConfig, DataBinding } from '@/types'

/**
 * 触发器 config 摘要
 * Trigger config summary
 * schedule: "cron: 0 9 * * *"
 * plugin_event: "event: clipboard_changed"
 * clipboard: "pattern: ^https://" 或 "(任意变化)"
 * hotkey: "shortcut: CmdOrCtrl+Shift+K"
 */
export function summarizeTriggerConfig(config: TriggerConfig | Record<string, unknown>): string {
  const cfg = config as Record<string, unknown>
  if ('cron' in cfg && typeof cfg.cron === 'string') {
    return `cron: ${cfg.cron}`
  }
  if ('event_type' in cfg && typeof cfg.event_type === 'string') {
    return `event: ${cfg.event_type}`
  }
  if ('pattern' in cfg) {
    return cfg.pattern ? `pattern: ${cfg.pattern}` : '(任意变化)'
  }
  if ('shortcut' in cfg && typeof cfg.shortcut === 'string') {
    return `shortcut: ${cfg.shortcut}`
  }
  return JSON.stringify(config)
}

/**
 * 把 cron 5 字段表达式翻译成人类语言（简单翻译）
 * Translate 5-field cron to a short human description
 * "0 9 * * *" → "每天 09:00"
 * "* /5 * * * *" → "每 5 分钟"
 * "0 8 * * 1" → "每周一 08:00"
 * 其他情况回退到原始表达式
 */
export function describeCron(expr: string): string {
  const parts = expr.trim().split(/\s+/)
  if (parts.length !== 5) return expr
  const [min, hour, , , dow] = parts

  // 每 N 分钟
  // every N minutes
  if (hour === '*' && min.length >= 3 && min[0] === '*' && min[1] === '/' && /\d/.test(min[2])) {
    const n = min.slice(2)
    return `每 ${n} 分钟`
  }
  // 每天 H:M
  if (hour !== '*' && !hour.includes(',') && !hour.includes('/') && !hour.includes('-')
    && min !== '*' && !min.includes(',') && !min.includes('/') && !min.includes('-')
    && dow === '*') {
    return `每天 ${hour.padStart(2, '0')}:${min.padStart(2, '0')}`
  }
  // 每周 DOW H:M
  if (dow !== '*' && !dow.includes(',') && !dow.includes('/') && !dow.includes('-')
    && hour !== '*' && !hour.includes(',') && !hour.includes('/') && !hour.includes('-')
    && min !== '*' && !min.includes(',') && !min.includes('/') && !min.includes('-')) {
    const dayNames = ['周日', '周一', '周二', '周三', '周四', '周五', '周六']
    const dayIdx = Number.parseInt(dow, 10)
    const dayName = dayNames[dayIdx] ?? dow
    return `每${dayName} ${hour.padStart(2, '0')}:${min.padStart(2, '0')}`
  }
  return expr
}

/**
 * DataBinding 摘要："A.text → B.content"
 * DataBinding summary: "A.text → B.content"
 */
export function summarizeBinding(b: DataBinding): string {
  const out = b.output || '*'
  return `${b.from}.${out} → ${b.to}.${b.input}`
}
