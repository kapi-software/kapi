// Tauri 桥接封装：环境检测、类型化 invoke、事件监听
// Tauri bridge helpers: environment detection, typed invoke, event listening
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

// 是否运行在 Tauri 环境
// 纯浏览器（vite dev 直开）环境下为 false，用于跳过数据库与窗口操作
// Whether running inside Tauri; false in a plain browser, used to skip DB/window calls
export function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

// 类型化 invoke 封装
// Typed invoke wrapper
// invokeTyped<Plugin[]>('get_installed_plugins')
export async function invokeTyped<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(command, args)
}

// 事件监听封装，返回取消监听函数
// Event listener wrapper returning an unlisten function
// const un = await onEvent<string>('plugin:navigate', (id) => navigate(`/plugin/${id}`))
export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return tauriListen<T>(event, (e) => handler(e.payload))
}
