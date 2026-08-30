// 数据库访问层：插件 / 插件数据 / 工作流 / 设置 / 日志
// Database access layer: plugins / plugin data / workflows / settings / logs
// 表结构见 docs/DATABASE.md；迁移已在 Rust 侧完成（src-tauri/src/db.rs），此处仅读写
import Database from '@tauri-apps/plugin-sql'
import type {
  Plugin,
  PluginRow,
  SystemLog,
  LogLevel,
  Workflow,
  WorkflowRow,
  WorkflowRun,
  WorkflowStepLog,
  WindowMode,
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
// 插件操作 / Plugin operations
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

  // 按 id 查询
  // Get by id
  async getById(id: string): Promise<Plugin | null> {
    const rows = await getDb().select<PluginRow[]>('SELECT * FROM plugins WHERE id = $1', [id])
    return rows[0] ? parsePlugin(rows[0]) : null
  },

  // 保存（INSERT OR REPLACE）
  // Save (INSERT OR REPLACE)
  async save(plugin: Plugin): Promise<void> {
    await getDb().execute(
      `INSERT OR REPLACE INTO plugins
       (id, name, version, author, description, icon, category, manifest,
        install_path, wasm_path, web_path, window_mode, window_config,
        is_enabled, sort_order, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,CURRENT_TIMESTAMP)`,
      [
        plugin.id,
        plugin.name,
        plugin.version,
        plugin.author,
        plugin.description,
        plugin.icon,
        plugin.category,
        JSON.stringify(plugin.manifest),
        plugin.install_path,
        plugin.wasm_path,
        plugin.web_path,
        plugin.window_mode,
        plugin.window_config ? JSON.stringify(plugin.window_config) : null,
        plugin.is_enabled ? 1 : 0,
        plugin.sort_order,
      ]
    )
  },

  // 删除（外键级联清 plugin_data）
  // Delete (foreign key cascades plugin_data)
  async delete(id: string): Promise<void> {
    await getDb().execute('DELETE FROM plugins WHERE id = $1', [id])
  },

  // 切换运行模式
  // Switch window mode
  async updateWindowMode(id: string, mode: WindowMode): Promise<void> {
    await getDb().execute(
      'UPDATE plugins SET window_mode = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2',
      [mode, id]
    )
  },

  // 启用 / 禁用（禁用后 Dock 与侧边栏隐藏，docs/PLUGINS.md §6）
  // Enable / disable (disabled plugins hide from the Dock and sidebar)
  async updateEnabled(id: string, enabled: boolean): Promise<void> {
    await getDb().execute(
      'UPDATE plugins SET is_enabled = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2',
      [enabled ? 1 : 0, id]
    )
  },

  // 批量更新排序（单事务）
  // Batch update sort order in one transaction
  async updateSortOrder(orderedIds: string[]): Promise<void> {
    await getDb().execute('BEGIN')
    try {
      for (let i = 0; i < orderedIds.length; i++) {
        await getDb().execute('UPDATE plugins SET sort_order = $1 WHERE id = $2', [
          i,
          orderedIds[i],
        ])
      }
      await getDb().execute('COMMIT')
    } catch (e) {
      await getDb().execute('ROLLBACK')
      throw e
    }
  },
}

// ============================================================
// 插件数据操作 / Plugin-scoped KV operations
// 命名空间隔离由 Rust 权限层强制，此处为宿主侧管理入口
// ============================================================

export const pluginDataDb = {
  async get(pluginId: string, key: string): Promise<string | null> {
    const rows = await getDb().select<Array<{ value: string }>>(
      'SELECT value FROM plugin_data WHERE plugin_id = $1 AND key = $2',
      [pluginId, key]
    )
    return rows[0]?.value ?? null
  },

  async set(pluginId: string, key: string, value: string): Promise<void> {
    await getDb().execute(
      `INSERT OR REPLACE INTO plugin_data (plugin_id, key, value, updated_at)
       VALUES ($1, $2, $3, CURRENT_TIMESTAMP)`,
      [pluginId, key, value]
    )
  },

  async getAll(pluginId: string): Promise<Record<string, string>> {
    const rows = await getDb().select<Array<{ key: string; value: string }>>(
      'SELECT key, value FROM plugin_data WHERE plugin_id = $1',
      [pluginId]
    )
    return Object.fromEntries(rows.map((r) => [r.key, r.value]))
  },

  async delete(pluginId: string, key: string): Promise<void> {
    await getDb().execute('DELETE FROM plugin_data WHERE plugin_id = $1 AND key = $2', [
      pluginId,
      key,
    ])
  },
}

// ============================================================
// 工作流操作 / Workflow operations
// ============================================================

// workflows 表行 → Workflow（解析 graph JSON）
// workflows row → Workflow (parses graph JSON)
function parseWorkflow(row: WorkflowRow): Workflow {
  return { ...row, graph: JSON.parse(row.graph) }
}

export const workflowDb = {
  async getAll(): Promise<Workflow[]> {
    const rows = await getDb().select<WorkflowRow[]>(
      'SELECT * FROM workflows ORDER BY updated_at DESC'
    )
    return rows.map(parseWorkflow)
  },

  async getById(id: string): Promise<Workflow | null> {
    const rows = await getDb().select<WorkflowRow[]>('SELECT * FROM workflows WHERE id = $1', [id])
    return rows[0] ? parseWorkflow(rows[0]) : null
  },

  async save(w: Workflow): Promise<void> {
    await getDb().execute(
      `INSERT OR REPLACE INTO workflows (id, name, description, graph, is_enabled, updated_at)
       VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)`,
      [w.id, w.name, w.description, JSON.stringify(w.graph), w.is_enabled ? 1 : 0]
    )
  },

  async delete(id: string): Promise<void> {
    // 外键级联清 runs
    // Foreign key cascades runs
    await getDb().execute('DELETE FROM workflows WHERE id = $1', [id])
  },

  // 查询执行历史（含步骤日志）
  // Runs with step logs
  async getRuns(workflowId: string, limit = 20): Promise<WorkflowRun[]> {
    const runs = await getDb().select<WorkflowRun[]>(
      `SELECT * FROM workflow_runs WHERE workflow_id = $1
       ORDER BY started_at DESC LIMIT $2`,
      [workflowId, limit]
    )
    if (runs.length === 0) return runs

    // 步骤日志按 run_id 批量拉取
    // Batch fetch step logs by run_id
    const ids = runs.map((r) => r.id)
    const placeholders = ids.map((_, i) => `$${i + 1}`).join(',')
    const steps = await getDb().select<WorkflowStepLog[]>(
      `SELECT * FROM workflow_step_logs WHERE run_id IN (${placeholders}) ORDER BY id`,
      ids
    )
    return runs.map((r) => ({ ...r, steps: steps.filter((s) => s.run_id === r.id) }))
  },
}

// ============================================================
// 设置操作 / Settings operations（统一表，见 docs/PANEL.md）
// ============================================================

export const settingsDb = {
  async get(key: string): Promise<string | null> {
    const rows = await getDb().select<Array<{ value: string }>>(
      'SELECT value FROM settings WHERE key = $1',
      [key]
    )
    return rows[0]?.value ?? null
  },

  async set(key: string, value: string): Promise<void> {
    await getDb().execute(
      `INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, CURRENT_TIMESTAMP)
       ON CONFLICT(key) DO UPDATE SET value = $2, updated_at = CURRENT_TIMESTAMP`,
      [key, value]
    )
  },

  async getAll(): Promise<Record<string, string>> {
    const rows = await getDb().select<Array<{ key: string; value: string }>>(
      'SELECT key, value FROM settings'
    )
    return Object.fromEntries(rows.map((r) => [r.key, r.value]))
  },
}

// ============================================================
// 日志操作 / System log operations
// ============================================================

export const logDb = {
  async add(
    level: LogLevel,
    message: string,
    source?: string,
    data?: unknown
  ): Promise<void> {
    await getDb().execute(
      `INSERT INTO system_logs (level, message, source, data) VALUES ($1, $2, $3, $4)`,
      [level, message, source ?? null, data === undefined ? null : JSON.stringify(data)]
    )
  },

  async getRecent(limit = 100): Promise<SystemLog[]> {
    return getDb().select<SystemLog[]>(
      'SELECT * FROM system_logs ORDER BY id DESC LIMIT $1',
      [limit]
    )
  },
}
