// 数据库只读访问层：插件 / 设置 / 日志 / 事件历史
// Database read-only access layer: plugins / settings / logs / event history
// 表结构见 docs/DATABASE.md；迁移已在 Rust 侧完成（src-tauri/src/db.rs）
// 所有写操作一律走 Rust 命令（写者矩阵见 docs/DATABASE.md §1.1），此处只做 SELECT
import Database from '@tauri-apps/plugin-sql'
import type {
  Plugin,
  PluginRow,
  SystemLog,
  PluginEvent,
} from '@/types'
import { supportedModes } from '@/lib/plugin-shapes'

let db: Database

// 连接数据库；重复调用复用已有连接
// Connect to the database; repeated calls reuse the existing connection
export async function initDb(): Promise<Database> {
  if (db) return db
  db = await Database.load('sqlite:kapi.db')
  return db
}

// 获取当前连接；未初始化时抛错
// Get the current connection; throws if not initialized
export function getDb(): Database {
  if (!db) throw new Error('数据库未初始化，请先调用 initDb() / Database not initialized')
  return db
}

// ============================================================
// 插件查询 / Plugin queries（写操作走 plugin_* Rust 命令）
// ============================================================

// plugins 表行 → Plugin（解析 manifest / window_config JSON，派生声明形态）
// plugins row → Plugin (parses manifest / window_config JSON, derives declared shapes)
function parsePlugin(row: PluginRow): Plugin {
  const manifest = JSON.parse(row.manifest)
  return {
    ...row,
    manifest,
    supported_modes: supportedModes(manifest, row.web_path != null, row.wasm_path != null),
    window_config: row.window_config ? JSON.parse(row.window_config) : null,
  }
}

export const pluginDb = {
  // 全部已安装插件（按 sort_order）
  // All installed plugins ordered by sort_order
  async getAll(): Promise<Plugin[]> {
    const rows = await getDb().select<PluginRow[]>(
      'SELECT * FROM plugins WHERE is_installed = 1 ORDER BY sort_order'
    )
    return rows.map(parsePlugin)
  },

  // 按 id 查询（插件独立窗口壳读取自身配置）
  // Get by id (the plugin window shell reads its own config)
  async getById(id: string): Promise<Plugin | null> {
    const rows = await getDb().select<PluginRow[]>('SELECT * FROM plugins WHERE id = $1', [id])
    return rows[0] ? parsePlugin(rows[0]) : null
  },
}

// ============================================================
// 设置查询 / Settings queries（写入走 settings_set Rust 命令）
// 统一表，见 docs/PANEL.md
// ============================================================

export const settingsDb = {
  async get(key: string): Promise<string | null> {
    const rows = await getDb().select<Array<{ value: string }>>(
      'SELECT value FROM settings WHERE key = $1',
      [key]
    )
    return rows[0]?.value ?? null
  },

  async getAll(): Promise<Record<string, string>> {
    const rows = await getDb().select<Array<{ key: string; value: string }>>(
      'SELECT key, value FROM settings'
    )
    return Object.fromEntries(rows.map((r) => [r.key, r.value]))
  },
}

// ============================================================
// 日志查询 / System log queries（写入在 Rust bridge/log.rs）
// ============================================================

export const logDb = {
  async getRecent(limit = 100): Promise<SystemLog[]> {
    return getDb().select<SystemLog[]>(
      'SELECT * FROM system_logs ORDER BY id DESC LIMIT $1',
      [limit]
    )
  },
}

// ============================================================
// 插件事件查询 / Plugin event queries（写入在 Rust bridge 侧落库）
// events.emit 落库的事件总线历史（docs/DATABASE.md plugin_events）
// ============================================================

export const eventDb = {
  async getRecent(limit = 200): Promise<PluginEvent[]> {
    return getDb().select<PluginEvent[]>(
      'SELECT * FROM plugin_events ORDER BY id DESC LIMIT $1',
      [limit]
    )
  },
}
