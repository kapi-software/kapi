/**
 * @file tauri.ts
 * @description Tauri 桥接封装：环境检测、类型化 invoke、事件监听
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始封装
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'
import type { UnlistenFn } from '@tauri-apps/api/event'

/**
 * 是否运行在 Tauri 环境 / Whether running inside Tauri
 *
 * 纯浏览器（vite dev 直开）环境下为 false，用于跳过数据库与窗口操作，
 * 保证页面在浏览器中也能渲染占位内容。
 */
export function isTauri(): boolean {
  return '__TAURI_INTERNALS__' in window
}

/**
 * 类型化 invoke 封装 / Typed invoke wrapper
 *
 * @param command - Tauri 命令名 / Command name
 * @param args - 命令参数 / Command arguments
 * @returns 命令返回值 / Command result
 *
 * @example
 * const plugins = await invokeTyped<Plugin[]>('get_installed_plugins')
 */
export async function invokeTyped<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return tauriInvoke<T>(command, args)
}

/**
 * 事件监听封装 / Event listener wrapper
 *
 * @param event - 事件名（如 'plugin:navigate'）/ Event name
 * @param handler - 处理函数 / Handler
 * @returns 取消监听函数 / Unlisten function
 *
 * @example
 * const unlisten = await onEvent<string>('plugin:navigate', (id) => navigate(`/plugin/${id}`))
 */
export async function onEvent<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
  return tauriListen<T>(event, (e) => handler(e.payload))
}
