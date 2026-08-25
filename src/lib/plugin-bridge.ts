// 插件桥接：PluginHost 侧 postMessage 协议处理（docs/PANEL.md §3）
// Plugin bridge: postMessage protocol handling on the PluginHost side
// 协议：请求 {id, channel, payload}（channel 以 kapi: 开头）；响应 {id, ok, data} | {id, ok, error}
// Protocol: request {id, channel, payload} (kapi: prefix); response {id, ok, data} | {id, ok, error}
// 权限检查全部在 Rust 侧（plugin_bridge 命令），此处只做来源校验与协议转发
// Permission checks live entirely in Rust (plugin_bridge); this side only
// validates the source and forwards the protocol

export interface BridgeRequest {
  id: number | string
  channel: string
  payload?: unknown
}

// 回发目标的最小结构：Window / 测试替身均满足
// Minimal shape of a postMessage target: satisfied by Window and test doubles
export interface BridgeTarget {
  postMessage(message: unknown, targetOrigin: string): void
}

// 事件最小结构：只依赖 source/origin/data（node 测试环境可构造）
// Minimal event shape: only source/origin/data (constructible in the node test env)
export type BridgeMessageEvent = Pick<MessageEvent, 'origin' | 'data'> & { source: unknown }

// 解析桥接请求：data 为对象、id 为 number|string、channel 为 kapi: 前缀字符串；否则 null
// Parse a bridge request: object data, number|string id, kapi:-prefixed channel; else null
export function parseBridgeRequest(data: unknown): BridgeRequest | null {
  if (typeof data !== 'object' || data === null) return null
  const { id, channel, payload } = data as Record<string, unknown>
  const validId = typeof id === 'number' || typeof id === 'string'
  if (!validId || typeof channel !== 'string' || !channel.startsWith('kapi:')) return null
  return { id, channel, payload: payload ?? null }
}

// 回发 targetOrigin：sandbox 无 allow-same-origin 的 iframe 是 opaque origin（字符串 "null"）→ '*'
// Response targetOrigin: sandboxed iframes (no allow-same-origin) report the opaque "null" → '*'
export function responseTargetOrigin(origin: string): string {
  return origin === '' || origin === 'null' ? '*' : origin
}

// 桥接 handler 工厂：注入依赖保持纯函数可测性（invoke 与目标窗口均由调用方提供）
// Bridge handler factory: deps are injected for testability (invoke / target window)
// 用法：const onMessage = createPluginBridgeHandler({pluginId, getTargetWindow, invoke})
// Usage: const onMessage = createPluginBridgeHandler({pluginId, getTargetWindow, invoke})
export function createPluginBridgeHandler(deps: {
  pluginId: string
  getTargetWindow: () => BridgeTarget | null
  invoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}): (event: BridgeMessageEvent) => void {
  return (event) => {
    // 只信任自家 iframe 的消息（e.source === contentWindow）
    // Trust only our own iframe (e.source === contentWindow)
    const target = deps.getTargetWindow()
    if (!target || event.source !== target) return
    const req = parseBridgeRequest(event.data)
    // 畸形请求静默丢弃：不回发，避免与恶意页面形成回声回路
    // Malformed requests are dropped silently: no echo loops with hostile pages
    if (!req) return

    const origin = responseTargetOrigin(event.origin)
    deps
      .invoke('plugin_bridge', {
        pluginId: deps.pluginId,
        channel: req.channel,
        payload: req.payload ?? null,
      })
      .then((data) => target.postMessage({ id: req.id, ok: true, data }, origin))
      .catch((err: unknown) =>
        target.postMessage(
          { id: req.id, ok: false, error: err instanceof Error ? err.message : String(err) },
          origin,
        ),
      )
  }
}
