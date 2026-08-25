// 插件桥接单元测试：协议解析 / 来源校验 / 回发形状（docs/PANEL.md §3）
// Plugin bridge unit tests: protocol parsing / source validation / response shapes
import { describe, it, expect, vi } from 'vitest'
import {
  parseBridgeRequest,
  responseTargetOrigin,
  createPluginBridgeHandler,
  type BridgeTarget,
  type BridgeMessageEvent,
} from '@/lib/plugin-bridge'

// 假 iframe 窗口：记录 postMessage 调用 / fake iframe window recording postMessage calls
function fakeTarget() {
  const posted: Array<{ message: unknown; origin: string }> = []
  const target: BridgeTarget = {
    postMessage: (message, origin) => posted.push({ message, origin }),
  }
  return { target, posted }
}

// 假消息事件 / fake message event
function fakeEvent(source: unknown, data: unknown, origin = 'null'): BridgeMessageEvent {
  return { source, origin, data }
}

describe('parseBridgeRequest / 桥接请求解析', () => {
  it('应解析合法请求并保留 id 与 channel / should parse a valid request', () => {
    const req = parseBridgeRequest({ id: 1, channel: 'kapi:storage.get', payload: { key: 'n' } })
    expect(req).toEqual({ id: 1, channel: 'kapi:storage.get', payload: { key: 'n' } })
  })

  it('应把缺省 payload 归一为 null / should normalize a missing payload to null', () => {
    const req = parseBridgeRequest({ id: 'abc', channel: 'kapi:window.close' })
    expect(req).toEqual({ id: 'abc', channel: 'kapi:window.close', payload: null })
  })

  it('应拒绝非 kapi 前缀 / 非法 id / 非对象数据 / should reject bad prefix, id or data', () => {
    expect(parseBridgeRequest({ id: 1, channel: 'storage.get' })).toBeNull()
    expect(parseBridgeRequest({ id: true, channel: 'kapi:storage.get' })).toBeNull()
    expect(parseBridgeRequest('kapi:storage.get')).toBeNull()
    expect(parseBridgeRequest(null)).toBeNull()
  })
})

describe('responseTargetOrigin / 回发源选择', () => {
  it('opaque 与空 origin 应回退 * / opaque and empty origins fall back to "*"', () => {
    expect(responseTargetOrigin('null')).toBe('*')
    expect(responseTargetOrigin('')).toBe('*')
  })

  it('正常 origin 应原样返回 / a normal origin passes through unchanged', () => {
    expect(responseTargetOrigin('http://kapi-plugin.localhost')).toBe('http://kapi-plugin.localhost')
  })
})

describe('createPluginBridgeHandler / 桥接 handler', () => {
  it('来源窗口不匹配时不得调用 invoke / must not invoke for foreign sources', async () => {
    const invoke = vi.fn().mockResolvedValue(null)
    const { target } = fakeTarget()
    const onMessage = createPluginBridgeHandler({
      pluginId: 'com.example.demo',
      getTargetWindow: () => target,
      invoke,
    })
    // 陌生 source / an unfamiliar source
    onMessage(fakeEvent({}, { id: 1, channel: 'kapi:storage.get' }))
    expect(invoke).not.toHaveBeenCalled()
  })

  it('成功时应按 id 回发 ok:true 与 data / should post ok:true with data by id', async () => {
    const invoke = vi.fn().mockResolvedValue({ value: 42 })
    const { target, posted } = fakeTarget()
    const onMessage = createPluginBridgeHandler({
      pluginId: 'com.example.demo',
      getTargetWindow: () => target,
      invoke,
    })
    onMessage(fakeEvent(target, { id: 7, channel: 'kapi:storage.get', payload: { key: 'n' } }))

    await vi.waitFor(() => expect(posted).toHaveLength(1))
    expect(invoke).toHaveBeenCalledWith('plugin_bridge', {
      pluginId: 'com.example.demo',
      channel: 'kapi:storage.get',
      payload: { key: 'n' },
    })
    // opaque origin 回发用 '*' / the opaque origin sends back to '*'
    expect(posted[0]).toEqual({
      message: { id: 7, ok: true, data: { value: 42 } },
      origin: '*',
    })
  })

  it('invoke 失败应回发 ok:false 与错误串 / should post ok:false with the error', async () => {
    const invoke = vi.fn().mockRejectedValue(new Error('PermissionDenied: storage:read'))
    const { target, posted } = fakeTarget()
    const onMessage = createPluginBridgeHandler({
      pluginId: 'com.example.demo',
      getTargetWindow: () => target,
      invoke,
    })
    onMessage(fakeEvent(target, { id: 'x1', channel: 'kapi:storage.get' }))

    await vi.waitFor(() => expect(posted).toHaveLength(1))
    expect(posted[0].message).toEqual({
      id: 'x1',
      ok: false,
      error: 'PermissionDenied: storage:read',
    })
  })

  it('畸形请求不得触发 invoke / malformed requests never invoke', async () => {
    const invoke = vi.fn()
    const { target } = fakeTarget()
    const onMessage = createPluginBridgeHandler({
      pluginId: 'com.example.demo',
      getTargetWindow: () => target,
      invoke,
    })
    // 来源匹配但协议不合法 / source matches but the protocol is invalid
    onMessage(fakeEvent(target, { id: 1, channel: 'evil:channel' }))
    onMessage(fakeEvent(target, 'just a string'))
    expect(invoke).not.toHaveBeenCalled()
  })
})
