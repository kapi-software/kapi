// 插件市场类型与源配置（Rust 侧 store.rs 的前端对应物）
// Store types and source config (the frontend counterpart of Rust store.rs)
import { initDb, settingsDb } from '@/lib/db'

// 市场源里的单个插件条目（store_list 返回形状）
// One plugin entry in the store source (the store_list return shape)
export interface StoreEntry {
  dir: string
  id: string
  name: string
  version: string
  author: string | null
  description: string | null
  category: string | null
}

// 缺省市场源：仓库顶层每个目录 = 一个插件（含 manifest.json）
// Default store source: every top-level repo dir is one plugin (with manifest.json)
export const DEFAULT_STORE_REPO = 'kapi-software/kapi-plugins'

// 读取持久化的市场源（settings.store.repo；缺省 DEFAULT_STORE_REPO）
// Read the persisted store source (settings.store.repo; defaults to DEFAULT_STORE_REPO)
export async function loadStoreRepo(): Promise<string> {
  try {
    await initDb()
    const saved = await settingsDb.get('store.repo')
    return saved && saved.trim() ? saved.trim() : DEFAULT_STORE_REPO
  } catch {
    return DEFAULT_STORE_REPO
  }
}

// 持久化市场源 / Persist the store source
export async function saveStoreRepo(repo: string): Promise<void> {
  await initDb()
  await settingsDb.set('store.repo', repo.trim())
}
