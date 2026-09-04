// validateGraph 单元测试 / validateGraph unit tests
import { describe, it, expect } from 'vitest'
import { validateGraph, hasFatalErrors } from '@/lib/workflow-graph'
import type { WorkflowGraph, WorkflowNode } from '@/types'

const node = (id: string): WorkflowNode => ({
  id,
  type: 'plugin',
  plugin_id: 'p',
  action: 'a',
  config: {},
  position: { x: 0, y: 0 },
})

const edge = (from: string, to: string): { from: string; to: string } => ({ from, to })

const graph = (nodes: WorkflowNode[], edges: { from: string; to: string }[]): WorkflowGraph => ({
  nodes,
  edges,
})

describe('validateGraph', () => {
  it('空图合法 / empty graph is valid', () => {
    expect(validateGraph(graph([], []))).toEqual([])
  })

  it('线性图合法 / linear graph is valid', () => {
    const r = validateGraph(graph([node('a'), node('b'), node('c')], [edge('a', 'b'), edge('b', 'c')]))
    expect(r).toEqual([])
  })

  it('节点 id 重复 → fatal / duplicate node ids → fatal', () => {
    const r = validateGraph(graph([node('a'), node('a')], []))
    expect(hasFatalErrors(r)).toBe(true)
    expect(r[0].code).toBe('duplicate_node_id')
  })

  it('悬空边 → fatal / dangling edge → fatal', () => {
    const r = validateGraph(graph([node('a')], [edge('a', 'ghost')]))
    expect(r.some((e) => e.code === 'dangling_edge')).toBe(true)
    expect(hasFatalErrors(r)).toBe(true)
  })

  it('自环 → fatal / self-loop → fatal', () => {
    const r = validateGraph(graph([node('a')], [edge('a', 'a')]))
    expect(r.some((e) => e.code === 'self_loop')).toBe(true)
    expect(hasFatalErrors(r)).toBe(true)
  })

  it('环 → fatal 并返回路径 / cycle → fatal with path', () => {
    const r = validateGraph(
      graph([node('a'), node('b'), node('c')], [edge('a', 'b'), edge('b', 'c'), edge('c', 'a')]),
    )
    const cycle = r.find((e) => e.code === 'cycle')!
    expect(cycle).toBeDefined()
    expect(cycle.node_ids![0]).toBe(cycle.node_ids![cycle.node_ids!.length - 1]) // 首尾相等
    expect(cycle.node_ids!.length).toBe(4) // a→b→c→a
  })

  it('重复边 → warning / duplicate edge → warning', () => {
    const r = validateGraph(graph([node('a'), node('b')], [edge('a', 'b'), edge('a', 'b')]))
    const w = r.find((e) => e.code === 'duplicate_edge')!
    expect(w.kind).toBe('warning')
    expect(hasFatalErrors(r)).toBe(false)
  })

  it('孤儿节点 → warning / orphan node → warning', () => {
    const r = validateGraph(
      graph([node('a'), node('b'), node('c')], [edge('a', 'b')]),
    )
    const w = r.find((e) => e.code === 'orphan_node')!
    expect(w.kind).toBe('warning')
    expect(w.node_ids).toContain('c')
  })

  it('单节点不报孤儿 / single node does not report orphan', () => {
    const r = validateGraph(graph([node('solo')], []))
    expect(r.some((e) => e.code === 'orphan_node')).toBe(false)
  })
})
