// 内置模板单元测试
// Built-in templates unit tests
import { describe, it, expect } from 'vitest'
import { WORKFLOW_TEMPLATES, getTemplate } from '@/lib/workflow-templates'
import type { WorkflowGraph } from '@/types'

describe('WORKFLOW_TEMPLATES', () => {
  it('至少有 1 个模板（至少空白模板）', () => {
    expect(WORKFLOW_TEMPLATES.length).toBeGreaterThan(0)
  })

  it('每个模板都有 id + name + graph', () => {
    for (const t of WORKFLOW_TEMPLATES) {
      expect(typeof t.id).toBe('string')
      expect(t.id.length).toBeGreaterThan(0)
      expect(typeof t.name).toBe('string')
      expect(t.name.length).toBeGreaterThan(0)
      expect(t.graph).toBeDefined()
    }
  })

  it('模板 id 唯一', () => {
    const ids = WORKFLOW_TEMPLATES.map((t) => t.id)
    expect(new Set(ids).size).toBe(ids.length)
  })

  it('所有模板的 graph 都是合法 WorkflowGraph 形状', () => {
    for (const t of WORKFLOW_TEMPLATES) {
      const g: WorkflowGraph = t.graph
      expect(Array.isArray(g.nodes)).toBe(true)
      expect(Array.isArray(g.edges)).toBe(true)
      // 节点 id 唯一
      const ids = g.nodes.map((n) => n.id)
      expect(new Set(ids).size).toBe(ids.length)
      // 边端点存在
      const idSet = new Set(ids)
      for (const e of g.edges) {
        expect(idSet.has(e.from)).toBe(true)
        expect(idSet.has(e.to)).toBe(true)
      }
    }
  })

  it('至少含 blank 模板', () => {
    expect(getTemplate('blank')).not.toBeNull()
  })

  it('模板引用插件占位 plugin_id 为空字符串（用户进编辑器后填）', () => {
    for (const t of WORKFLOW_TEMPLATES) {
      if (t.id === 'blank') continue
      const hasPlaceholder = t.graph.nodes.some(
        (n) => n.type === 'plugin' && n.plugin_id === '',
      )
      expect(hasPlaceholder).toBe(true)
    }
  })
})

describe('getTemplate', () => {
  it('已知 id → 返回模板', () => {
    const blank = getTemplate('blank')
    expect(blank).not.toBeNull()
    expect(blank!.id).toBe('blank')
  })

  it('未知 id → null', () => {
    expect(getTemplate('not-a-template')).toBeNull()
  })

  it('null / undefined → null', () => {
    expect(getTemplate(null)).toBeNull()
    expect(getTemplate(undefined)).toBeNull()
    expect(getTemplate('')).toBeNull()
  })
})
