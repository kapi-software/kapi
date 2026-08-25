/**
 * @file stores/settings.ts
 * @description 设置 Zustand store：加载 / 更新 / 重置（对应 docs/plan.MD §8.1）
 * @author Kapi 开发团队 / Kapi Development Team
 * @created 2026-08-25
 * @updated 2026-08-25
 *
 * @changes
 * - 2026-08-25: Phase 1 初始实现
 */

import { create } from 'zustand'
import { settingsDb, initDb } from '@/lib/db'
import { isTauri } from '@/lib/tauri'
import {
  AppSettings,
  DEFAULT_SETTINGS,
  parseRawSettings,
} from '@/lib/settings'

interface SettingsStore {
  settings: AppSettings
  loading: boolean
  /** 初始化是否完成（浏览器环境无 Tauri 时也为 true，保持默认值） */
  ready: boolean
  loadSettings: () => Promise<void>
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>
  resetSettings: () => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: DEFAULT_SETTINGS,
  loading: false,
  ready: false,

  loadSettings: async () => {
    // 浏览器环境：无数据库，直接就绪（保持默认值）
    if (!isTauri()) {
      set({ ready: true })
      return
    }

    set({ loading: true })
    try {
      await initDb()
      const raw = await settingsDb.getAll()
      set({ settings: parseRawSettings(raw), loading: false, ready: true })
    } catch (error) {
      console.error('加载设置失败 / Failed to load settings:', error)
      set({ loading: false, ready: true })
    }
  },

  updateSetting: async (key, value) => {
    // 先写库再更新状态，失败时状态不被污染
    if (isTauri()) {
      await settingsDb.set(key, JSON.stringify(value))
    }
    set((state) => ({ settings: { ...state.settings, [key]: value } }))
  },

  resetSettings: async () => {
    if (isTauri()) {
      for (const [key, value] of Object.entries(DEFAULT_SETTINGS)) {
        await settingsDb.set(key, JSON.stringify(value))
      }
    }
    set({ settings: DEFAULT_SETTINGS })
  },
}))
