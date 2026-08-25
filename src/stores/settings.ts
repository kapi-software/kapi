// 设置 Zustand store：加载 / 更新 / 重置 / 跨窗口同步
// Settings Zustand store: load / update / reset / cross-window sync
// 设计见 docs/PANEL.md；多窗口（主面板 / dock）各自持有 store 实例，
// 变更经 settings:changed 事件广播，各窗口监听后补丁本地状态
// Design in docs/PANEL.md; each window (panel / dock) holds its own store,
// changes broadcast via the settings:changed event and patch every window
import { create } from 'zustand'
import { emit } from '@tauri-apps/api/event'
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
  // 初始化是否完成（浏览器环境无 Tauri 时也为 true，保持默认值）
  // Whether initialization finished (true in browsers too, keeping defaults)
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
    // Browser env: no DB, ready immediately with defaults
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
    // Persist first, then update state so failures don't dirty the UI
    if (isTauri()) {
      await settingsDb.set(key, JSON.stringify(value))
      // 广播给所有窗口（含自身，重复应用幂等）
      // Broadcast to every window (including self; re-applying is idempotent)
      await emit('settings:changed', { key, value })
    }
    set((state) => ({ settings: { ...state.settings, [key]: value } }))
  },

  resetSettings: async () => {
    if (isTauri()) {
      for (const [key, value] of Object.entries(DEFAULT_SETTINGS)) {
        await settingsDb.set(key, JSON.stringify(value))
      }
      // 重置广播完整快照（逐 key 广播 15 次没有意义）
      // Reset broadcasts a full snapshot (15 per-key events would be wasteful)
      await emit('settings:changed', { settings: DEFAULT_SETTINGS })
    }
    set({ settings: DEFAULT_SETTINGS })
  },
}))
