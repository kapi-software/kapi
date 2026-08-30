// @kapi/plugin-sdk 单元测试：stub window 求值单文件源码（node 环境，无 DOM）
// @kapi/plugin-sdk unit tests: evaluate the single-file source against a stub window
import { describe, it, expect } from 'vitest'
// ?raw：以字符串引入 SDK 源码（vite/client 提供 *?raw 模块声明）
// ?raw: imports the SDK source as a string (vite/client declares the *?raw module)
import sdkSource from '../../src-tauri/assets/kapi-sdk.js?raw'

// 微任务冲刷：等待 Promise 链稳定 / microtask flush
async function flush() {
  await new Promise((r) => setTimeout(r, 0))
}

// 假 window：收集 message 监听与 parent.postMessage 调用（pagehide 等其它监听不参与）
// Fake window: collects message listeners and parent.postMessage calls (pagehide and
// other listeners stay out of the dispatch)
function loadSdk(parentOverride?: unknown) {
  const listeners: Array<(e: { data: unknown }) => void> = []
  const posted: Array<{ id: number; channel: string; payload: unknown }> = []
  const fakeWindow: Record<string, unknown> = {
    addEventListener: (type: string, fn: (e: { data: unknown }) => void) => {
      if (type === 'message') listeners.push(fn)
    },
  }
  fakeWindow.parent =
    parentOverride !== undefined ? parentOverride : { postMessage: (msg: unknown) => posted.push(msg as never) }

  new Function('window', sdkSource)(fakeWindow)
  const kapi = fakeWindow.kapi as Record<string, never> & {
    [k: string]: unknown
  }

  // 宿主回发 RPC 响应 / the host replies to an RPC
  const reply = (id: number, ok: boolean, data?: unknown, error?: string) => {
    listeners.forEach((fn) => fn({ data: { id, ok, data, error } }))
  }
  // 宿主推送事件 / the host pushes an event
  const push = (type: string, data: unknown, source = 'com.source') => {
    listeners.forEach((fn) => fn({ data: { kapiEvent: true, type, data, source } }))
  }
  const channels = () => posted.map((m) => m.channel)
  return { kapi, posted, reply, push, channels }
}

describe('@kapi/plugin-sdk / RPC 基础 / RPC basics', () => {
  it('调用应向 parent postMessage {id, channel, payload} 并按响应 resolve', async () => {
    const { kapi, posted, reply } = loadSdk()
    const promise = (kapi.storage as never as { get: (k: string) => Promise<unknown> }).get('counter')
    expect(posted).toEqual([{ id: 1, channel: 'kapi:storage.get', payload: { key: 'counter' } }])
    reply(1, true, { value: 5 })
    await expect(promise).resolves.toEqual({ value: 5 })
  })

  it('错误响应应 reject 出带 code 的 BridgeError', async () => {
    const { kapi, reply } = loadSdk()
    const promise = (kapi.storage as never as { get: (k: string) => Promise<unknown> }).get('x')
    reply(1, false, undefined, 'PermissionDenied: storage:read')
    await expect(promise).rejects.toMatchObject({
      code: 'PermissionDenied',
      message: 'PermissionDenied: storage:read',
    })
  })

  it('各命名空间通道名应与桥接表一致 / namespaces map to documented channels', async () => {
    const { kapi, reply, channels } = loadSdk()
    const any = kapi as never as Record<string, Record<string, (...a: unknown[]) => Promise<unknown>>>
    const calls: Array<Promise<unknown>> = [
      any.storage.set('k', 1),
      any.storage.remove('k'),
      any.clipboard.writeText('t'),
      any.log.info('m'),
      any.window.close(),
      any.plugin.invoke('reverse', { text: 'x' }),
      any.events.emit('tick', { n: 1 }),
    ]
    for (let i = 1; i <= calls.length; i++) reply(i, true, null)
    await Promise.all(calls)
    expect(channels()).toEqual([
      'kapi:storage.set',
      'kapi:storage.remove',
      'kapi:clipboard.write',
      'kapi:log.info',
      'kapi:window.close',
      'kapi:plugin.invoke',
      'kapi:events.emit',
    ])
  })

  it('无宿主（parent 即自身）应拒绝 BridgeUnavailable', async () => {
    // parent = window 本身 → 浏览器预览场景
    // parent === window itself → the browser-preview scenario
    const fake: Record<string, unknown> = {}
    const { kapi } = loadSdk(fake)
    await expect(
      (kapi.storage as never as { get: (k: string) => Promise<unknown> }).get('x')
    ).rejects.toMatchObject({ code: 'BridgeUnavailable' })
  })
})

describe('@kapi/plugin-sdk / 事件订阅 / event subscription', () => {
  it('on 应先订阅宿主，推送到达时分发回调，off 后退订', async () => {
    const { kapi, posted, reply, push, channels } = loadSdk()
    const received: Array<{ type: string; data: unknown; source: string }> = []
    const onPromise = (
      kapi.events as never as {
        on: (t: string, h: (ev: { type: string; data: unknown; source: string }) => void) => Promise<() => void>
      }
    ).on('counter.changed', (ev) => received.push(ev))
    // id 1 = kapi:events.on / id 1 is the kapi:events.on call
    expect(channels()).toEqual(['kapi:events.on'])
    reply(1, true, null)
    const off = await onPromise

    push('counter.changed', { count: 3 }, 'com.a')
    await flush()
    expect(received).toEqual([{ type: 'counter.changed', data: { count: 3 }, source: 'com.a' }])

    // 未订阅的类型不触发 / unsubscribed types never fire
    push('other.event', { x: 1 })
    await flush()
    expect(received).toHaveLength(1)

    off()
    await flush()
    expect(channels()).toEqual(['kapi:events.on', 'kapi:events.off'])
    expect(posted[1]?.payload).toEqual({ type: 'counter.changed' })
  })

  it('同类型多个回调共享一条宿主订阅，全部退订才发 off', async () => {
    const { kapi, reply, push, channels } = loadSdk()
    const events = (kapi.events as never as {
      on: (t: string, h: () => void) => Promise<() => void>
    })
    const hits: string[] = []
    const p1 = events.on('tick', () => hits.push('a'))
    reply(1, true, null)
    const off1 = await p1
    // 第二个回调：本地注册，不再向宿主订阅
    // Second callback: local registration only, no extra host subscription
    const p2 = events.on('tick', () => hits.push('b'))
    const off2 = await p2
    expect(channels()).toEqual(['kapi:events.on'])

    push('tick', 1)
    await flush()
    expect(hits).toEqual(['a', 'b'])

    off1()
    await flush()
    expect(channels()).toEqual(['kapi:events.on']) // 还剩一个回调 / one callback remains
    off2()
    await flush()
    expect(channels()).toEqual(['kapi:events.on', 'kapi:events.off'])
  })

  it('订阅被拒应回滚本地注册并透传错误 / a denied subscribe rolls back and rejects', async () => {
    const { kapi, reply, push } = loadSdk()
    const hits: number[] = []
    const p = (kapi.events as never as { on: (t: string, h: () => void) => Promise<() => void> }).on(
      'tick',
      () => hits.push(1)
    )
    reply(1, false, undefined, 'PermissionDenied: events:subscribe')
    await expect(p).rejects.toMatchObject({ code: 'PermissionDenied' })
    // 回滚后推送不触发任何回调 / after rollback a push triggers nothing
    push('tick', 1)
    await flush()
    expect(hits).toEqual([])
    // 失败后重订不受残留计数影响 / a retry starts clean
    const hits2: number[] = []
    const p2 = (kapi.events as never as { on: (t: string, h: () => void) => Promise<() => void> }).on(
      'tick',
      () => hits2.push(1)
    )
    reply(2, true, null)
    await p2
    push('tick', 2)
    await flush()
    expect(hits2).toEqual([1])
  })
})
