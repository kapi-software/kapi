// P1-2 连接校验单元测试
// P1-2: connection validation unit tests
import { describe, expect, it } from 'vitest'
import type { Plugin, PluginManifest, PluginWorkflowSpec } from '@/types'
import { validateConnection, type ValidationResult } from './workflow-connection'

// ============================================================
// 类型守卫 helper
// Type guards for narrowing ValidationResult
// ============================================================

function assertOk(r: ValidationResult): asserts r is { ok: true; autoMap: Record<string, string> } {
  expect(r.ok).toBe(true)
}

function assertFail(r: ValidationResult): asserts r is { ok: false; reason: string } {
  expect(r.ok).toBe(false)
}

// ============================================================
// Fixtures
// ============================================================

function makePlugin(id: string, spec: Partial<PluginWorkflowSpec> = {}): Plugin {
  return {
    id,
    name: id,
    version: '1.0.0',
    author: null,
    description: null,
    icon: null,
    category: null,
    manifest: { id, name: id, version: '1.0.0', workflow: spec } as PluginManifest,
    install_path: '',
    wasm_path: null,
    web_path: null,
    window_mode: 'embedded',
    supported_modes: ['embedded'],
    window_config: null,
    is_enabled: true,
    is_installed: true,
    sort_order: 0,
    installed_at: '',
    updated_at: '',
  }
}

const CLIPBOARD_PLUGIN: Plugin = makePlugin('clipboard-plugin', {
  actions: [{
    name: 'get',
    inputs: {},
    outputs: {
      content: { type: 'string', label: '内容' },
      length: { type: 'number', label: '长度' },
    },
  }],
})

const TEXT_PLUGIN: Plugin = makePlugin('text-plugin', {
  actions: [{
    name: 'uppercase',
    inputs: { text: { type: 'string', label: '文本' } },
    outputs: { text: { type: 'string', label: '结果' } },
  }],
})

const HTTP_PLUGIN: Plugin = makePlugin('http-plugin', {
  actions: [{
    name: 'fetch',
    inputs: { url: { type: 'string', label: 'URL' } },
    outputs: {
      status: { type: 'number', label: '状态码' },
      body: { type: 'string', label: '响应体' },
    },
  }],
})

const ALL_PLUGINS = [CLIPBOARD_PLUGIN, TEXT_PLUGIN, HTTP_PLUGIN]

// ============================================================
// Tests
// ============================================================

describe('P1-2: 连接校验', () => {
  it('自环 → 拒绝', () => {
    const r = validateConnection(
      { source: 'n1', target: 'n1' },
      { id: 'n1', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      { id: 'n1', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      ALL_PLUGINS,
    )
    assertFail(r)
    expect(r.reason).toContain('不能连接自身')
  })

  it('transform 节点 → 始终允许', () => {
    const r = validateConnection(
      { source: 't1', target: 'n2' },
      { id: 't1', type: 'transform' },
      { id: 'n2', type: 'plugin', plugin_id: 'http-plugin', action: 'fetch' },
      ALL_PLUGINS,
    )
    assertOk(r)
    expect(r.autoMap).toEqual({})
  })

  it('v2 同名字段类型匹配 → 允许 + autoMap', () => {
    // clipboard.get outputs content/length, text.uppercase inputs text
    // 无同名字段，autoMap 为空
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      { id: 'n1', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      { id: 'n2', type: 'plugin', plugin_id: 'text-plugin', action: 'uppercase' },
      ALL_PLUGINS,
    )
    assertOk(r)
    expect(r.autoMap).toEqual({})
  })

  it('v2 同名字段类型不匹配 → 拒绝', () => {
    // 构造真正同名字段但类型不匹配：
    // plugin-a 输出 content:number, plugin-b 输入 content:string → 类型不匹配
    const mixedPluginA: Plugin = makePlugin('mixed-a', {
      actions: [{
        name: 'op',
        inputs: {},
        outputs: { content: { type: 'number', label: '内容' } },
      }],
    })
    const mixedPluginB: Plugin = makePlugin('mixed-b', {
      actions: [{
        name: 'op',
        inputs: { content: { type: 'string', label: '内容' } },
        outputs: {},
      }],
    })
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      { id: 'n1', type: 'plugin', plugin_id: 'mixed-a', action: 'op' },
      { id: 'n2', type: 'plugin', plugin_id: 'mixed-b', action: 'op' },
      [mixedPluginA, mixedPluginB],
    )
    assertFail(r)
    expect(r.reason).toContain('content')
  })

  it('同名字段类型匹配 → autoMap 含映射', () => {
    // text-plugin uppercase outputs text:string
    // http-plugin fetch inputs url:string（无同名）→ 但有 status:number 也无同名
    // 测试需要两端同名字段：text-plugin uppercase outputs.text:string
    // 构造一个 inputs 含 text:string 的下游
    const downstreamWithText: Plugin = makePlugin('downstream', {
      actions: [{
        name: 'accept',
        inputs: { text: { type: 'string', label: '文本' } },
        outputs: {},
      }],
    })
    const r = validateConnection(
      { source: 't1', target: 'd1' },
      { id: 't1', type: 'plugin', plugin_id: 'text-plugin', action: 'uppercase' },
      { id: 'd1', type: 'plugin', plugin_id: 'downstream', action: 'accept' },
      [...ALL_PLUGINS, downstreamWithText],
    )
    assertOk(r)
    expect(r.autoMap).toEqual({ text: 'text' })
  })

  it('上游无 outputs → 允许', () => {
    const sourceOnly: Plugin = makePlugin('source-only', {
      actions: [{
        name: 'op',
        inputs: { seed: { type: 'string', label: 'seed' } },
        outputs: {},
      }],
    })
    const r = validateConnection(
      { source: 's1', target: 't1' },
      { id: 's1', type: 'plugin', plugin_id: 'source-only', action: 'op' },
      { id: 't1', type: 'plugin', plugin_id: 'text-plugin', action: 'uppercase' },
      [...ALL_PLUGINS, sourceOnly],
    )
    assertOk(r)
    expect(r.autoMap).toEqual({})
  })

  it('下游无 inputs → 允许', () => {
    const sinkOnly: Plugin = makePlugin('sink-only', {
      actions: [{
        name: 'sink',
        inputs: {},
        outputs: { result: { type: 'string', label: '结果' } },
      }],
    })
    const r = validateConnection(
      { source: 'c1', target: 's1' },
      { id: 'c1', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      { id: 's1', type: 'plugin', plugin_id: 'sink-only', action: 'sink' },
      [...ALL_PLUGINS, sinkOnly],
    )
    assertOk(r)
    expect(r.autoMap).toEqual({})
  })

  it('不存在的 plugin_id → 允许（运行时插件可能未加载）', () => {
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      { id: 'n1', type: 'plugin', plugin_id: 'nonexistent', action: 'foo' },
      { id: 'n2', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      ALL_PLUGINS,
    )
    assertOk(r)
  })

  it('不存在的 action → 允许', () => {
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      { id: 'n1', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'nonexistent' },
      { id: 'n2', type: 'plugin', plugin_id: 'clipboard-plugin', action: 'get' },
      ALL_PLUGINS,
    )
    assertOk(r)
  })

  it('undefined sourceNode → 允许（兜底）', () => {
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      undefined,
      { id: 'n2', type: 'plugin', plugin_id: 'text-plugin', action: 'uppercase' },
      ALL_PLUGINS,
    )
    assertOk(r)
  })

  it('undefined targetNode → 允许（兜底）', () => {
    const r = validateConnection(
      { source: 'n1', target: 'n2' },
      { id: 'n1', type: 'plugin', plugin_id: 'text-plugin', action: 'uppercase' },
      undefined,
      ALL_PLUGINS,
    )
    assertOk(r)
  })

  it('两端都是 transform → 允许（不检查类型）', () => {
    const r = validateConnection(
      { source: 't1', target: 't2' },
      { id: 't1', type: 'transform' },
      { id: 't2', type: 'transform' },
      ALL_PLUGINS,
    )
    assertOk(r)
  })
})
