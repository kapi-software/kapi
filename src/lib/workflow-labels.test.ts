// 人类可读摘要测试
// Human-readable summary tests
import { describe, it, expect } from 'vitest'
import { summarizeTriggerConfig, describeCron } from './workflow-labels'
import type { TriggerConfig } from '@/types'

describe('summarizeTriggerConfig', () => {
  it('schedule: 显示 cron 表达式', () => {
    expect(summarizeTriggerConfig({ cron: '0 9 * * *' } as TriggerConfig)).toBe(
      'cron: 0 9 * * *',
    )
  })

  it('plugin_event: 显示 event_type', () => {
    expect(summarizeTriggerConfig({ event_type: 'clipboard_changed' } as TriggerConfig)).toBe(
      'event: clipboard_changed',
    )
  })

  it('clipboard: 显示 pattern 或"任意变化"', () => {
    expect(summarizeTriggerConfig({ pattern: '^https://' } as TriggerConfig)).toBe(
      'pattern: ^https://',
    )
    expect(summarizeTriggerConfig({} as TriggerConfig)).toBe('(任意变化)')
  })

  it('hotkey: 显示 shortcut', () => {
    expect(
      summarizeTriggerConfig({ shortcut: 'CmdOrCtrl+Shift+K' } as TriggerConfig),
    ).toBe('shortcut: CmdOrCtrl+Shift+K')
  })

  it('未知 config: 退化为 JSON.stringify', () => {
    expect(summarizeTriggerConfig({ foo: 'bar' } as unknown as TriggerConfig)).toBe(
      JSON.stringify({ foo: 'bar' }),
    )
  })
})

describe('describeCron', () => {
  it('每天 HH:MM', () => {
    expect(describeCron('0 9 * * *')).toBe('每天 09:00')
    expect(describeCron('30 18 * * *')).toBe('每天 18:30')
  })

  it('每 N 分钟', () => {
    expect(describeCron('*/5 * * * *')).toBe('每 5 分钟')
    expect(describeCron('*/15 * * * *')).toBe('每 15 分钟')
  })

  it('每周 N HH:MM', () => {
    expect(describeCron('0 8 * * 1')).toBe('每周一 08:00')
    expect(describeCron('30 17 * * 5')).toBe('每周五 17:30')
  })

  it('非标准表达式：回退到原表达式', () => {
    expect(describeCron('0,30 9-17 * * 1-5')).toBe('0,30 9-17 * * 1-5')
  })

  it('字段数不对：回退到原表达式', () => {
    expect(describeCron('0 9 * *')).toBe('0 9 * *')
  })
})

