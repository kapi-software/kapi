# Kapi 应用技术文档

**版本**: v2.1.0
**最后更新**: 2026-08-25
**状态**: 设计文档

---

## 📋 目录

1. [项目概述](#1-项目概述)
2. [应用架构](#2-应用架构)
3. [数据库设计](#3-数据库设计)
4. [主面板设计](#4-主面板设计)
5. [Dock 侧边栏](#5-dock-侧边栏)
6. [插件系统](#6-插件系统)
7. [工作流系统](#7-工作流系统)
8. [设置系统](#8-设置系统)
9. [技术实现](#9-技术实现)
10. [开发计划](#10-开发计划)
11. [变更记录](#11-变更记录)

---

## 1. 项目概述

### 1.1 项目简介

**Kapi** 是一款基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。

应用由两类界面组成：

- **主面板**：应用主界面，左侧为功能导航（设置、插件市场、工作流等），右侧为页面内容。插件默认在主面板内嵌显示。
- **Dock 侧边栏**：屏幕右缘的弧形快速启动栏，鼠标悬停展开。**Dock 只负责唤醒插件**（按插件配置分发到主面板内嵌 / 独立窗口 / 无界面执行），本身不承载任何插件逻辑。

插件支持三种运行模式（内嵌 / 独立窗口 / 无界面），每个插件都可以拥有自定义的独立运行窗口；插件之间通过工作流系统实现数据联动（如剪贴板监听 → 代码美化保存 → 生成截图）。所有数据持久化存储在本地 SQLite 数据库中。

### 1.2 核心功能

| 功能模块 | 说明 |
| --------- | ------ |
| **主面板** | 左侧导航（首页/插件/插件市场/工作流/日志/设置）+ 右侧内容区 |
| **Dock 侧边栏** | 弧形布局的插件快速启动栏，仅负责唤醒，鼠标悬停展开 |
| **插件系统** | WASM 沙箱逻辑 + Web UI，支持 embedded / independent / headless 三种模式 |
| **工作流系统** | 插件间数据联动与自动化编排（触发器 + DAG 步骤图） |
| **本地数据库** | SQLite 存储所有应用数据 |

### 1.3 技术栈

| 层级 | 技术选型 |
| ------ | --------- |
| 应用框架 | Tauri v2 |
| 前端框架 | React 19 + TypeScript |
| 样式方案 | Tailwind CSS + shadcn/ui |
| 状态管理 | Zustand |
| 路由 | React Router v7（v6 API 兼容） |
| Dock 动画 | motion (framer-motion)，对齐 Electron 版 Dock 实现 |
| 数据库 | SQLite（tauri-plugin-sql） |
| WASM 运行时 | wasmtime（Rust 侧，最新稳定版） |
| 构建工具 | Vite |

**Tauri 插件清单**：

| 插件 | 用途 |
| ------ | ------ |
| `tauri-plugin-sql` | SQLite 访问与迁移 |
| `tauri-plugin-clipboard-manager` | 剪贴板监听（工作流触发器）+ 插件剪贴板权限 |
| `tauri-plugin-global-shortcut` | 全局快捷键（Alt+Space 唤醒 Dock、工作流 hotkey 触发器） |
| `tauri-plugin-autostart` | 开机自启动设置项 |
| `tauri-plugin-opener` | 打开外部链接（插件市场仓库页等） |

---

## 2. 应用架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Kapi 应用                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                 主窗口 main (Main Window)                            │   │
│  │  ┌──────────────┐  ┌────────────────────────────────────────────┐ │   │
│  │  │  侧边栏导航   │  │           内容区域 (React Router)          │ │   │
│  │  │              │  │                                            │ │   │
│  │  │  🏠 首页     │  │   - 仪表盘 / 插件管理 / 插件市场          │ │   │
│  │  │  🧩 插件     │  │   - 工作流列表 / 工作流编辑器              │ │   │
│  │  │  📦 插件市场  │  │   - 日志 / 设置                          │ │   │
│  │  │  🔄 工作流   │  │   - 插件内嵌视图 (PluginHost + iframe)    │ │   │
│  │  │  📊 日志     │  │     路由 /plugin/:id                      │ │   │
│  │  │  ⚙️ 设置     │  │                                            │ │   │
│  │  └──────────────┘  └────────────────────────────────────────────┘ │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │              Dock 窗口 dock (独立透明窗口, 仅唤醒)                   │   │
│  │   弧形布局 ← 点击图标 → invoke launch_plugin → Rust 按模式分发      │   │
│  │   不渲染任何插件内容，不含插件逻辑                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │              插件窗口 plugin-<id> (可选独立实例)                    │   │
│  │   加载应用路由 /plugin-window/:id (裸 PluginHost 壳)                │   │
│  │   窗口尺寸/标题/可缩放等由插件 manifest 自定义                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Rust 核心 (src-tauri)                           │   │
│  │   dock_service │ plugin_manager │ wasm_runtime │ workflow_engine    │   │
│  │   github_client │ plugin_protocol (kapi-plugin://) │ plugin_bridge  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    SQLite 数据库 (本地存储)                          │   │
│  │  plugins │ plugin_data │ workflows │ workflow_runs                  │   │
│  │  workflow_step_logs │ settings │ plugin_events │ system_logs       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 窗口架构

| 窗口 | Label | 说明 | 特性 |
| ------ | ------- | ------ | ------ |
| **主窗口** | `main` | 应用主界面 | 左侧导航 + 右侧内容区，承载设置、市场、工作流、内嵌插件 |
| **Dock 窗口** | `dock` | 快速启动栏 | 弧形布局，透明无边框，始终置顶，不进任务栏，仅负责唤醒 |
| **插件窗口** | `plugin-<id>` | 插件独立实例 | 仅当插件 window_mode = `independent` 时创建，窗口参数取自插件 manifest，可由用户在插件管理页切换模式 |

### 2.3 统一插件启动流程（核心链路）

无论从 Dock 点击、主面板侧边栏点击还是工作流调用，都走同一条链路：

```
点击图标 (Dock / 主面板)
  └─> invoke('launch_plugin', { pluginId })
        └─> Rust: 读取 plugins 表 window_mode
              ├─ 'embedded'    → emit('plugin:navigate', id) → 主窗口路由 /plugin/:id
              │                  (主窗口若隐藏/最小化则同时 show + set_focus)
              ├─ 'independent' → 创建或聚焦 WebviewWindow('plugin-<id>')
              │                  窗口参数来自 manifest.window (标题/尺寸/可缩放)
              └─ 'headless'    → 直接调用 WASM 入口执行一次动作
                                 (工作流插件，仅数据处理，无 UI)
```

要点：

- **Dock 不做模式判断**，只发 `launch_plugin`，分发逻辑集中在 Rust 一处。
- 同一插件重复点击 `independent` 模式时**聚焦已有窗口**而非重复创建。
- 用户可在插件管理页修改某插件的 `window_mode`，下次启动即生效。

---

## 3. 数据库设计

### 3.1 数据库选型与迁移策略

使用 **SQLite**，通过 `tauri-plugin-sql` 访问。

**迁移唯一入口在 Rust 侧**：使用 `tauri_plugin_sql::Migration` 在插件初始化时自动执行（见 §9.2）。前端 `Database.load('sqlite:kapi.db')` 只做读写，不执行任何 DDL —— 避免双端初始化竞态，保证迁移有版本记录、只跑一次。

### 3.2 表结构总览

| 表 | 用途 | 关键关系 |
| ---- | ------ | --------- |
| `plugins` | 已安装插件注册表 | 被 plugin_data / plugin_events 引用 |
| `plugin_data` | 插件隔离的 KV 存储（每插件仅能访问自己的命名空间） | `(plugin_id, key)` 复合主键 |
| `workflows` | 工作流定义（DAG 图存 JSON） | — |
| `workflow_runs` | 工作流执行实例（一次触发一条） | CASCADE 删随 workflow |
| `workflow_step_logs` | 步骤级执行日志（输入/输出/耗时，可追溯） | CASCADE 删随 run |
| `settings` | **统一**应用设置表（含 `dock_*` 前缀项，无独立 dock 表） | — |
| `plugin_events` | 事件总线历史（触发工作流 + 审计） | `ON DELETE SET NULL` |
| `system_logs` | 系统日志 | — |

### 3.3 完整建表语句

```sql
-- ========================================
-- 001_init.sql  初始表结构
-- ========================================
PRAGMA foreign_keys = ON;

-- ============ 插件注册表 ============
CREATE TABLE plugins (
    id            TEXT PRIMARY KEY,      -- 反向域名，如 com.example.code-beautifier
    name          TEXT NOT NULL,
    version       TEXT NOT NULL,
    author        TEXT,
    description   TEXT,
    icon          TEXT,                  -- 相对插件目录的图标路径，如 icon.png
    category      TEXT,                  -- tool / dev / design / data ...
    manifest      TEXT NOT NULL,         -- 完整 manifest JSON (TEXT)
    install_path  TEXT NOT NULL,         -- 插件安装目录绝对路径
    wasm_path     TEXT,                  -- 相对 install_path 的 WASM 入口；NULL = 纯 UI 插件
    web_path      TEXT,                  -- 相对 install_path 的 UI 入口；NULL = headless 插件
    window_mode   TEXT NOT NULL DEFAULT 'embedded'
                  CHECK (window_mode IN ('embedded', 'independent', 'headless')),
    window_config TEXT,                  -- manifest.window 的 JSON 快照 {title,width,height,...}
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    is_installed  INTEGER NOT NULL DEFAULT 1,
    sort_order    INTEGER NOT NULL DEFAULT 0,  -- Dock 与插件列表排序
    installed_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 插件数据存储（隔离 KV） ============
-- 每个插件只能读写自己 plugin_id 命名空间下的数据（由 Rust 权限层强制）
CREATE TABLE plugin_data (
    plugin_id  TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,            -- JSON 编码值
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_id, key)
) WITHOUT ROWID;

-- ============ 工作流定义 ============
CREATE TABLE workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    graph       TEXT NOT NULL,           -- DAG JSON: {nodes, edges, bindings}，见 §7.2
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 工作流执行实例 ============
CREATE TABLE workflow_runs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    trigger_type TEXT,                   -- clipboard / hotkey / schedule / manual / plugin_event
    status      TEXT NOT NULL
                CHECK (status IN ('running', 'success', 'failed', 'cancelled')),
    error       TEXT,
    started_at  TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT
);

-- ============ 工作流步骤日志 ============
-- 记录每个节点的输入/输出/耗时，支撑"剪贴板→美化→截图"链路的逐步追溯
CREATE TABLE workflow_step_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id      INTEGER NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    step_id     TEXT NOT NULL,           -- 对应 graph.nodes[].id
    plugin_id   TEXT,
    action      TEXT,
    status      TEXT NOT NULL
                CHECK (status IN ('running', 'success', 'failed', 'skipped')),
    input       TEXT,                    -- JSON：节点实际收到的输入
    output      TEXT,                    -- JSON：节点产出
    error       TEXT,
    duration_ms INTEGER,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 应用设置（统一表，含 Dock 设置） ============
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,         -- 如 theme / dock_enabled / dock_position
    value      TEXT NOT NULL,            -- JSON 编码：'"zh-CN"' / 'true' / '12'
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 插件事件表 ============
-- 事件总线的历史记录：插件 emit 的事件 + 系统事件，用于触发工作流与审计
CREATE TABLE plugin_events (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type       TEXT NOT NULL,      -- 如 clipboard_changed / processed / saved
    source_plugin_id TEXT REFERENCES plugins(id) ON DELETE SET NULL,  -- NULL = 系统事件
    data             TEXT,               -- JSON
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 系统日志 ============
CREATE TABLE system_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    level      TEXT NOT NULL CHECK (level IN ('debug', 'info', 'warn', 'error')),
    message    TEXT NOT NULL,
    source     TEXT,                     -- 模块名，如 dock_service / workflow_engine
    data       TEXT,                     -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 索引 ============
CREATE INDEX idx_plugins_category   ON plugins(category);
CREATE INDEX idx_plugins_sort       ON plugins(sort_order);
CREATE INDEX idx_runs_workflow      ON workflow_runs(workflow_id, started_at);
CREATE INDEX idx_step_logs_run      ON workflow_step_logs(run_id);
CREATE INDEX idx_events_type        ON plugin_events(event_type, created_at);
CREATE INDEX idx_events_plugin      ON plugin_events(source_plugin_id);
CREATE INDEX idx_syslogs_level      ON system_logs(level);
CREATE INDEX idx_syslogs_created    ON system_logs(created_at);
```

> 说明：`plugin_data` 使用 `(plugin_id, key)` 复合主键 + `WITHOUT ROWID`，无需额外索引；所有外键均带 `ON DELETE` 策略，删除插件/工作流不产生孤儿数据。

### 3.4 默认数据

```sql
-- ========================================
-- 002_defaults.sql  默认设置种子
-- ========================================
INSERT OR IGNORE INTO settings (key, value) VALUES
    -- 通用
    ('language',              '"zh-CN"'),
    ('auto_start',            'false'),
    ('check_update',          'true'),
    -- 主题
    ('theme',                 '"system"'),   -- 'light' | 'dark' | 'system'
    ('accent_color',          '"#007AFF"'),
    -- Dock（统一存本表，dock_ 前缀，无独立 dock_settings 表）
    ('dock_enabled',          'true'),
    ('dock_hotzone_width',    '12'),
    ('dock_animation_speed',  '"medium"'),  -- 'slow' | 'medium' | 'fast'
    ('dock_expand_delay',     '0'),
    ('dock_auto_hide_delay',  '3000'),
    ('dock_visible_items',    '9'),
    ('dock_position',         '"right"'),
    -- 插件
    ('plugin_auto_update',    'false'),
    ('plugin_sandbox_strict', 'true'),
    ('plugin_log_level',      '"info"');
```

### 3.5 迁移文件规划

```
src-tauri/migrations/
├── 001_init.sql        # 建表 + 索引（§3.3）
├── 002_defaults.sql    # 默认设置种子（§3.4）
└── 003_xxx.sql         # 后续按版本递增，禁止修改已发布的迁移文件
```

### 3.6 数据库访问层（前端）

```typescript
// src/lib/db.ts
import Database from '@tauri-apps/plugin-sql'

let db: Database

/**
 * 连接数据库。迁移已由 Rust 侧 tauri-plugin-sql 自动完成（§9.2），此处仅获取连接。
 */
export async function initDb() {
  db = await Database.load('sqlite:kapi.db')
  return db
}

export function getDb() {
  return db
}

// ============ 插件操作 ============
export const pluginDb = {
  async getAll(): Promise<Plugin[]> {
    return db.select('SELECT * FROM plugins WHERE is_installed = 1 ORDER BY sort_order')
  },

  async getById(id: string): Promise<Plugin | null> {
    const rows = await db.select<Plugin[]>('SELECT * FROM plugins WHERE id = $1', [id])
    return rows[0] ?? null
  },

  async save(plugin: Plugin): Promise<void> {
    await db.execute(
      `INSERT OR REPLACE INTO plugins
       (id, name, version, author, description, icon, category, manifest,
        install_path, wasm_path, web_path, window_mode, window_config,
        is_enabled, sort_order, updated_at)
       VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,CURRENT_TIMESTAMP)`,
      [
        plugin.id, plugin.name, plugin.version, plugin.author,
        plugin.description, plugin.icon, plugin.category,
        JSON.stringify(plugin.manifest),
        plugin.install_path, plugin.wasm_path, plugin.web_path,
        plugin.window_mode, JSON.stringify(plugin.window_config ?? null),
        plugin.is_enabled ? 1 : 0, plugin.sort_order ?? 0,
      ]
    )
  },

  async delete(id: string): Promise<void> {
    await db.execute('DELETE FROM plugins WHERE id = $1', [id]) // 级联清 plugin_data
  },

  async updateWindowMode(id: string, mode: 'embedded' | 'independent' | 'headless'): Promise<void> {
    await db.execute(
      'UPDATE plugins SET window_mode = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2',
      [mode, id]
    )
  },

  async updateSortOrder(orderedIds: string[]): Promise<void> {
    await db.execute('BEGIN')
    for (let i = 0; i < orderedIds.length; i++) {
      await db.execute('UPDATE plugins SET sort_order = $1 WHERE id = $2', [i, orderedIds[i]])
    }
    await db.execute('COMMIT')
  },
}

// ============ 插件数据操作（命名空间隔离在 Rust 侧强制） ============
export const pluginDataDb = {
  async get(pluginId: string, key: string): Promise<string | null> {
    const rows = await db.select<{ value: string }[]>(
      'SELECT value FROM plugin_data WHERE plugin_id = $1 AND key = $2',
      [pluginId, key]
    )
    return rows[0]?.value ?? null
  },

  async set(pluginId: string, key: string, value: string): Promise<void> {
    await db.execute(
      `INSERT OR REPLACE INTO plugin_data (plugin_id, key, value, updated_at)
       VALUES ($1, $2, $3, CURRENT_TIMESTAMP)`,
      [pluginId, key, value]
    )
  },

  async getAll(pluginId: string): Promise<Record<string, string>> {
    const rows = await db.select<{ key: string; value: string }[]>(
      'SELECT key, value FROM plugin_data WHERE plugin_id = $1',
      [pluginId]
    )
    return Object.fromEntries(rows.map(r => [r.key, r.value]))
  },

  async delete(pluginId: string, key: string): Promise<void> {
    await db.execute('DELETE FROM plugin_data WHERE plugin_id = $1 AND key = $2', [pluginId, key])
  },
}

// ============ 工作流操作 ============
export const workflowDb = {
  async getAll(): Promise<Workflow[]> {
    const rows = await db.select<WorkflowRow[]>('SELECT * FROM workflows ORDER BY updated_at DESC')
    return rows.map(parseWorkflow)
  },

  async getById(id: string): Promise<Workflow | null> {
    const rows = await db.select<WorkflowRow[]>('SELECT * FROM workflows WHERE id = $1', [id])
    return rows[0] ? parseWorkflow(rows[0]) : null
  },

  async save(w: Workflow): Promise<void> {
    await db.execute(
      `INSERT OR REPLACE INTO workflows (id, name, description, graph, is_enabled, updated_at)
       VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)`,
      [w.id, w.name, w.description, JSON.stringify(w.graph), w.enabled ? 1 : 0]
    )
  },

  async delete(id: string): Promise<void> {
    await db.execute('DELETE FROM workflows WHERE id = $1', [id]) // 级联清 runs
  },

  /** 查询执行历史（含步骤日志，供工作流详情页展示） */
  async getRuns(workflowId: string, limit = 20): Promise<WorkflowRun[]> {
    const runs = await db.select<WorkflowRun[]>(
      `SELECT * FROM workflow_runs WHERE workflow_id = $1
       ORDER BY started_at DESC LIMIT $2`,
      [workflowId, limit]
    )
    if (runs.length === 0) return runs
    const ids = runs.map(r => r.id)
    const steps = await db.select<WorkflowStepLog[]>(
      `SELECT * FROM workflow_step_logs WHERE run_id IN (${ids.map((_, i) => `$${i + 1}`).join(',')})
       ORDER BY id`,
      ids
    )
    return runs.map(r => ({ ...r, steps: steps.filter(s => s.run_id === r.id) }))
  },
}

function parseWorkflow(row: WorkflowRow): Workflow {
  return { ...row, graph: JSON.parse(row.graph) }
}

// ============ 设置操作（统一表） ============
export const settingsDb = {
  async get(key: string): Promise<string | null> {
    const rows = await db.select<{ value: string }[]>(
      'SELECT value FROM settings WHERE key = $1', [key]
    )
    return rows[0]?.value ?? null
  },

  async set(key: string, value: string): Promise<void> {
    await db.execute(
      `INSERT INTO settings (key, value, updated_at) VALUES ($1, $2, CURRENT_TIMESTAMP)
       ON CONFLICT(key) DO UPDATE SET value = $2, updated_at = CURRENT_TIMESTAMP`,
      [key, value]
    )
  },

  async getAll(): Promise<Record<string, string>> {
    const rows = await db.select<{ key: string; value: string }[]>(
      'SELECT key, value FROM settings'
    )
    return Object.fromEntries(rows.map(r => [r.key, r.value]))
  },
}

// ============ 日志操作 ============
export const logDb = {
  async add(level: 'debug' | 'info' | 'warn' | 'error', message: string, source?: string, data?: unknown): Promise<void> {
    await db.execute(
      `INSERT INTO system_logs (level, message, source, data) VALUES ($1, $2, $3, $4)`,
      [level, message, source ?? null, data ? JSON.stringify(data) : null]
    )
  },

  async getRecent(limit = 100): Promise<SystemLog[]> {
    return db.select(
      'SELECT * FROM system_logs ORDER BY id DESC LIMIT $1', [limit]
    )
  },
}
```

> 类型定义（`Plugin` / `Workflow` / `WorkflowRun` 等）见 §7.2 与 `src/types/index.ts`。

---

## 4. 主面板设计

### 4.1 布局结构

主面板采用**左侧导航 + 右侧内容区**布局，使用 React + TypeScript + Tailwind CSS + shadcn/ui 开发：

```
┌──────────────────────────────────────────────────────────────────┐
│ ┌────────┐ ┌───────────────────────────────────────────────────┐ │
│ │ Logo    │ │  顶栏 (面包屑 + 全局搜索 + 窗口控制)              │ │
│ ├────────┤ ├───────────────────────────────────────────────────┤ │
│ │ 导航    │ │                                                   │ │
│ │         │ │                                                   │ │
│ │ 🏠 首页 │ │              内容区域                             │ │
│ │ 🧩 插件 │ │                                                   │ │
│ │ 📦 市场 │ │        (由 React Router 渲染)                     │ │
│ │ 🔄 工作流│ │                                                   │ │
│ │ 📊 日志 │ │        /plugin/:id 时渲染插件内嵌视图             │ │
│ │ ⚙️ 设置 │ │                                                   │ │
│ │         │ │                                                   │ │
│ └────────┘ └───────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

侧边栏使用 shadcn/ui `Sidebar` 组件（可折叠），主导航分组：

| 分组 | 项 | 路由 |
| ------ | ----- | ------ |
| 概览 | 首页（仪表盘） | `/` |
| 插件 | 插件（已安装管理） | `/plugins` |
| 插件 | 插件市场 | `/store` |
| 自动化 | 工作流 | `/workflow` |
| 系统 | 日志 | `/logs` |
| 系统 | 设置 | `/settings` |

### 4.2 页面路由

| 路由 | 页面 | 组件 | 说明 |
| ------ | ------ | ------ | ------ |
| `/` | 首页 | `Dashboard` | 概览：最近使用插件、工作流运行状态、快捷入口 |
| `/plugins` | 插件管理 | `PluginsPage` | 已安装列表、启用/禁用、切换 window_mode、拖拽排序（写 sort_order） |
| `/store` | 插件市场 | `StorePage` | GitHub 来源浏览、安装/更新/卸载 |
| `/plugin/:id` | 插件内嵌视图 | `PluginEmbedView` | 主面板内嵌运行插件（§4.3） |
| `/plugin-window/:id` | 插件独立壳 | `PluginWindowShell` | 独立窗口加载的**裸壳路由**（无侧边栏），内嵌同一 `PluginHost` |
| `/workflow` | 工作流列表 | `WorkflowPage` | 列表、启停、手动运行、运行历史 |
| `/workflow/:id` | 工作流编辑器 | `WorkflowEditor` | React Flow 画布编辑 DAG |
| `/logs` | 日志 | `LogsPage` | system_logs 过滤查看 |
| `/settings` | 设置 | `SettingsPage` | 见 §8 |

### 4.3 插件内嵌视图与 PluginHost（核心组件）

**内嵌模式（embedded）与独立窗口模式（independent）共用同一个宿主组件 `PluginHost`**，区别仅在外壳：

- embedded → `PluginEmbedView`（带主面板侧边栏的 `/plugin/:id`）
- independent → `PluginWindowShell`（独立 `WebviewWindow` 加载的 `/plugin-window/:id`，无侧边栏）

`PluginHost` 职责：加载插件 UI（iframe 指向 `kapi-plugin://` 自定义协议，天然隔离）+ 桥接插件与 Rust 之间的 RPC。

```tsx
// src/components/plugin/PluginHost.tsx（示意）
import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export function PluginHost({ pluginId }: { pluginId: string }) {
  const frameRef = useRef<HTMLIFrameElement>(null)

  // 1. 监听 iframe 的 postMessage，转发到 Rust（权限检查在 Rust 侧统一执行）
  useEffect(() => {
    const onMessage = async (e: MessageEvent) => {
      if (e.source !== frameRef.current?.contentWindow) return   // 只信任自家 iframe
      const req = e.data                                        // { id, channel, payload }
      if (typeof req?.channel !== 'string' || !req.channel.startsWith('kapi:')) return

      try {
        const result = await invoke('plugin_bridge', {
          pluginId, channel: req.channel, payload: req.payload,
        })
        e.source!.postMessage({ id: req.id, ok: true, data: result }, '*')
      } catch (err) {
        e.source!.postMessage({ id: req.id, ok: false, error: String(err) }, '*')
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [pluginId])

  // 2. 插件 UI 通过自定义协议加载（Windows 下为 http://kapi-plugin.localhost/...）
  return (
    <iframe
      ref={frameRef}
      className="h-full w-full border-0"
      src={`kapi-plugin://localhost/${pluginId}/index.html`}
      sandbox="allow-scripts allow-forms"   // 不给 allow-same-origin，保持隔离
    />
  )
}
```

设计要点：

- **为什么不直接 `innerHTML`**：innerHTML 注入的 `<script>` 不执行、资源相对路径不解析、插件与宿主共享上下文零隔离。iframe + 自定义协议解决全部三个问题。
- **为什么独立窗口也走应用路由**：插件 iframe 内没有 Tauri IPC；让独立窗口加载应用自己的 `/plugin-window/:id` 壳页，桥接链路与内嵌完全一致，权限检查只写一处。
- 插件 UI 与插件 WASM 的通信同样经 `PluginHost` → `plugin_bridge` → `wasm_runtime`（§6.5）。

---

## 5. Dock 侧边栏

> 本章已与 Electron 版源码逐项对齐：`kapi-main/src/main/dock.ts`（窗口与热区轮询）、
> `window-manager.ts`（状态机与穿透策略）、`renderer/src/dock/Dock.tsx` + `dock.css`（前端）、
> `renderer/src/lib/arc-math.ts`（弧线数学）、`shared/types.ts`（DockState）。
> 其余章节（插件系统/数据库/工作流）为 Tauri 版全新设计，不移植 Electron 实现。

### 5.1 窗口参数（对齐 `main/dock.ts`）

| 参数 | 值 | 说明 |
| ------ | ----- | ------ |
| 尺寸 | 320 × 560 | 固定，`resizable: false`、`movable: false` |
| 位置 | 光标所在显示器 workArea 右缘、垂直居中 | **创建时一次计算**（Rust 启动时定位），之后不动 |
| 透明 | `transparent: true` | — |
| 无边框 | `decorations: false` | — |
| 阴影 | `shadow: false` | — |
| 置顶 | `alwaysOnTop: true` | Electron 用 'screen-saver' 级别 |
| 任务栏 | `skipTaskbar: true` | — |
| 初始可见 | `visible: false` | 加载完成后按 `dock_enabled` 设置决定是否 show |

**核心原则：展开/收起永远不做 setBounds / resize**——透明窗口 resize 会整窗闪烁（Electron 踩坑结论）。窗口大小与位置终身固定；收起态的"消失"靠**鼠标穿透**实现：整窗 `set_ignore_cursor_events(true)`，物理上只剩右缘 12px 箭头条带可交互，其余区域全部穿透。

### 5.2 弧线几何与视觉规格（对齐 `arc-math.ts` + `Dock.tsx` + `dock.css`）

#### 弧线数学

```text
圆心: C = (320, 280)          # 容器右缘 x + 垂直中心
半径: R = 260
角度: θ ∈ [π/2 + 0.1π, 3π/2 − 0.1π]   # 左半弧，两端各留 10% 边距

插件位置:
  x = 320 + 260 × cos(θ)
  y = 280 + 260 × sin(θ)

可见数: VISIBLE_COUNT = 9（centerIndex = ⌊9/2⌋ = 4 为选中态）
```

纯函数平移到 `src/lib/dock-arc.ts`，保持签名 `getPositionOnArc(index, total)`，配套单元测试（CLAUDE.md §2.3.1 示例）。

#### 滚轮循环滚动

```text
可见槽位 i ∈ [0, 9)，实际插件索引:
  actualIndex = ((scrollOffset + i) % total + total) % total
滚轮 deltaY > 0 → offset + 1，反之 −1（仅展开态响应，preventDefault）
```

#### 视觉规格

| 元素 | 规格 |
| ------ | ------ |
| 箭头触发器 | 12 × 150 px，贴右缘垂直居中，左圆角 `12px 0 0 12px`；hover 展宽至 16px |
| 箭头图标 | chevron-left SVG，展开态旋转 180°（0.2s ease） |
| 弧形轨道 | SVG path `M 320 0 A 320 280 0 0 0 320 560`，虚线 `4 6`，`rgba(255,255,255,0.03)` |
| 插件项 | 64×64 圆形；选中放大 1.12 倍并显示 label（8px 胶囊）；hover 1.1 倍 |
| 选中态配色 | 蓝色系光晕 `rgba(0,122,255,*)`（与主题强调色联动可后置） |
| 动画 | **全部用 motion (framer-motion)，禁用 CSS transition/transform**：展开/收起 0.18s `cubic-bezier(0.22,1,0.36,1)`；箭头 0.12s 弹性 `[0.34,1.56,0.64,1]`（展宽 0.16s easeOut） |

### 5.3 状态机（对齐 `shared/types.ts` + `window-manager.ts`）

```text
状态: hidden | peeking | expanded
      # peeking 为过渡中间态，渲染层按收起处理（Dock.tsx toUiState）

状态归属: Rust 主进程持有唯一状态，渲染层被动接收 dock:state 事件，
          不维护本地状态副本（与 Electron window.kapi.onStateChange 同构）

转换:
  hidden → expanded    光标进入右缘 12px 热区（轮询上升沿，见 §5.4）
  expanded → hidden    光标离开窗口（轮询下降沿）或 点击窗口外部（渲染层通知）
  expanded → hidden    延迟自动收起 scheduleCollapse（默认 3000ms = dock_auto_hide_delay）
  Alt+Space 切换       Tauri 版新增（global-shortcut）；Electron 版无此入口
```

**鼠标穿透策略（移植重点，Electron 踩坑结论）**：

1. 收起态：整窗穿透（`set_ignore_cursor_events(true)`），热区展开完全依赖 Rust 轮询；
2. **收起瞬间不能立即恢复穿透**——光标可能还停在箭头上，立即穿透会吞掉下一次"再点一下展开"；由轮询确认光标离开 12px 条带后**幂等恢复**穿透；
3. 展开第一动作 = 关闭穿透；
4. 每次状态变更后进入 **120ms 冷却期**，期间跳过热区检测（防边界抖动）；
5. `dock_enabled = false` → 先收起再 `hide()` 整窗；轮询遇窗口不可见直接跳过并重置热区观察值；重新启用 → `show()`。

> **Tauri 移植差异**：Electron `setIgnoreMouseEvents(ignore, { forward: true })` 可向渲染层转发 mousemove（渲染层 hover 兜底展开）；Tauri `set_ignore_cursor_events` 无转发能力。**Rust 轮询是展开的唯一权威路径**——Electron 版穿透态下渲染层本就收不到事件（其注释原话："热区展开完全依赖这里的轮询兜底"），行为一致，无功能损失。

### 5.4 Rust 热区轮询（`dock_service.rs`）

100ms 轮询，**边沿触发**（非电平触发），对齐 `main/dock.ts startMouseCheck`：

```rust
// 伪代码
loop {
    sleep(100ms);
    if 距上次状态变更 < 120ms { continue; }            // 冷却期
    if dock 窗口隐藏（dock_enabled=false）{ was_in_hotzone = false; continue; }

    let cursor = 全局光标位置();                         // 跨平台 API 见下表
    let in_window_y = cursor.y ∈ [win.y, win.y + win.h];

    // 热区几何随状态变化：收起态只有右缘 12px 条带；展开态是整个窗口
    let in_hotzone = match state {
        Hidden   => in_window_y && cursor.x ∈ [win.right - 12, win.right],
        Expanded | Peeking => in_window_y && cursor.x ∈ [win.x, win.right],
    };

    match (state, in_hotzone, was_in_hotzone) {
        (Hidden, true, false)   => { expand(); 记录状态变更时间; }    // 上升沿
        (Hidden, false, _)      => { 恢复鼠标穿透(); }               // 幂等
        (Expanded, false, true) => { collapse(); 记录状态变更时间; }  // 下降沿
        _ => {}
    }
    was_in_hotzone = in_hotzone;   // 每轮无条件刷新（防外部改状态后卡死）
}
```

Electron `screen.getCursorScreenPoint()` 的 Rust 等价物：

| 平台 | 光标位置 | 显示器归属 |
| ------ | --------- | ----------- |
| Windows | `GetCursorPos` (Win32) | `MonitorFromPoint` → `GetMonitorInfo` 取 workArea |
| macOS | CGEventTap | NSScreen |
| Linux X11 | `XQueryPointer` | RandR |
| Linux Wayland | 不可用 | 降级为仅 Alt+Space 快捷键唤醒（§10.4） |

### 5.5 Dock 只负责唤醒

对齐需求与 Electron 现状的差异：

- **Electron 现状**：点击插件 = 滚动到中心位（`handlePluginClick` 仅 `setScrollOffset`），点击"设置"临时打开主面板；
- **Tauri 版目标**：点击 = 唤醒分发，一行核心调用：

```typescript
// Dock 点击处理（唯一职责：唤醒）
const handleDockClick = async (pluginId: string) => {
  await invoke('launch_plugin', { pluginId })   // 分发逻辑全部在 Rust（§2.3）
}
```

Dock 前端职责收敛为：弧形布局渲染（motion 动画）、滚轮循环切换、点击唤醒、点击外部收起通知、接收 `dock:state` 事件驱动动画。**不做任何模式判断、不渲染插件内容。**

---

## 6. 插件系统

> **架构定位**：Tauri 版插件系统**基于 WASM**（逻辑运行在 wasmtime 沙箱），为全新设计；
> Electron 版的插件宿主/沙箱实现（web worker 形态）**不移植**，仅插件产品概念（安装源、
> 独立窗口、KV 存储、事件）在本设计中等价重现。

### 6.1 插件包结构

```
com.example.code-beautifier/        # 目录名 = 插件 id
├── manifest.json                   # 元数据与权限声明（§6.2）
├── icon.png                        # 128×128 图标
├── main.wasm                       # 逻辑入口（可选，纯 UI 插件可省略）
└── web/                            # UI 资源（可选，headless 插件可省略）
    ├── index.html
    ├── main.js                     # 引入 @kapi/plugin-sdk（桥接 RPC 封装）
    └── style.css
```

安装后解压至 `{app_data}/plugins/{id}/`，`plugins` 表记录 `install_path` 及相对入口。

### 6.2 manifest.json

```jsonc
{
  "id": "com.example.code-beautifier",
  "name": "代码美化",
  "version": "1.0.0",
  "kapi_version": "^1.0.0",          // 兼容的宿主 API 版本
  "author": "Your Name",
  "description": "代码格式化与收藏",

  // 运行模式与独立窗口自定义（三选一；用户可在插件管理页覆盖 mode）
  "window": {
    "mode": "embedded",              // 'embedded' | 'independent' | 'headless'
    "title": "代码美化",
    "width": 420,
    "height": 640,
    "minWidth": 320,
    "minHeight": 400,
    "resizable": true,
    "alwaysOnTop": false
  },

  // 工作流能力声明（headless 插件的主要形态）
  "workflow": {
    "triggers": ["clipboard_changed"],
    "actions": [
      { "name": "format", "inputs": { "text": "string" }, "outputs": { "formatted": "string" } },
      { "name": "save",   "inputs": { "formatted": "string" }, "outputs": { "savedId": "string" } }
    ],
    "events": ["processed", "saved"]
  },

  // 权限声明（默认全部拒绝，逐项显式申请）
  "permissions": [
    "storage:read", "storage:write",
    "clipboard:read",
    "network:host:api.github.com"
  ]
}
```

### 6.2.1 运行模式

| 模式 | UI | 逻辑 | 典型场景 |
| ------ | ----- | ------ | --------- |
| **embedded** | 主面板 `/plugin/:id` 内嵌 iframe | WASM / 桥接 API | 工具类插件，与主界面紧密集成 |
| **independent** | 独立 `WebviewWindow`，窗口参数取自 manifest.window | 同上 | 需要独立空间、复杂 UI、常驻的插件 |
| **headless** | 无 UI | 仅 WASM | 工作流数据处理插件、后台服务 |

三种模式共用同一套桥接 API 与 WASM 入口；`window_mode` 用户可随时在插件管理页切换。

### 6.3 权限模型

- **默认拒绝**：manifest 未声明的权限，桥接调用一律返回 `PermissionDenied`。
- **检查点唯一**：所有权限检查集中在 Rust `plugin_bridge` 命令与 wasmtime 宿主函数两处（共用同一个 `PermissionGuard`）。
- **网络权限带域名白名单**：`network:host:<domain>`，宿主侧代理请求并校验目标域名。
- **存储天然隔离**：`storage:*` 只能访问 `plugin_data` 中自身 `plugin_id` 命名空间。
- **严格模式**：`plugin_sandbox_strict = true` 时，未签名插件拒绝加载（签名验证 Phase 7，见 §10 风险项）。

### 6.4 桥接 API（插件 SDK 通道）

插件 UI 通过 `@kapi/plugin-sdk` 调用，SDK 内部走 `postMessage('kapi:*')` → `PluginHost` → `plugin_bridge`（权限检查）→ 执行：

| 通道 | 权限 | 说明 |
| ------ | ------ | ------ |
| `kapi:storage.get/set/remove` | `storage:*` | 插件隔离 KV |
| `kapi:clipboard.read/write` | `clipboard:read/write` | 剪贴板 |
| `kapi:http.fetch` | `network:host:<domain>` | 宿主代理 HTTP |
| `kapi:events.emit` / `kapi:events.on` | `events:emit/subscribe` | 事件总线（可触发工作流） |
| `kapi:plugin.invoke(action, payload)` | 无需声明（调用自身） | 调用本插件 WASM 入口 |
| `kapi:window.setTitle/close/minimize` | — | independent 模式窗口控制 |
| `kapi:log.debug/info/warn/error` | — | 写 system_logs |

### 6.5 WASM 运行时（wasmtime）

```rust
// src-tauri/src/wasm_runtime.rs（示意）
pub struct WasmRuntime {
    /// plugin_id → 已实例化 engine，懒加载 + 复用
    instances: Mutex<HashMap<String, PluginInstance>>,
}

impl WasmRuntime {
    /// 调用插件 WASM 入口：JSON 进 / JSON 出
    pub async fn invoke_action(
        &self,
        plugin_id: &str,
        action: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        // 1. 取插件实例（懒加载 install_path/main.wasm，wasmtime 编译缓存）
        // 2. 序列化 { action, payload } 写入线性内存
        // 3. 调用导出函数 kapi_invoke(ptr, len) -> i64（结果 JSON 回读）
        // 4. 反序列化返回
    }
}
```

- **ABI（v1，刻意保持简单）**：单一导出函数 `kapi_invoke`，JSON 字符串经线性内存传递。提供 `kapi-plugin-sdk`（Rust crate，编译目标 wasm32-wasip1）封装序列化与宿主函数导入。后续如需更强类型再引入 WIT 组件模型。
- **宿主函数**：WASM 内可导入与 §6.4 相同能力的 `kapi_host_*` 函数（storage/clipboard/http/events/log），同样经 `PermissionGuard` 检查。
- **资源限制**：fuel metering 限制单次执行指令数（防死循环），执行超时 5s 强制中断。
- **UI ↔ WASM 通路**：插件 UI `kapi:plugin.invoke` → `plugin_bridge` → `WasmRuntime::invoke_action` → JSON 结果原路返回。

### 6.6 插件生命周期

```
安装(市场/本地导入) → 校验 manifest → 解压到 plugins/{id}/ → 写 plugins 表 → 就绪
    ↓
运行: launch_plugin 按模式分发（§2.3）
    ↓
禁用: is_enabled = 0，Dock 与侧边栏隐藏，工作流节点标记不可用
    ↓
卸载: 关闭其独立窗口 → DELETE plugins 行（CASCADE 清 plugin_data）→ 删除安装目录
```

---

## 7. 工作流系统

### 7.1 概念模型与典型场景

工作流 = **触发器 + DAG 步骤图 + 数据绑定**，实现插件间数据联动。

**典型场景（本期的能力目标，示例插件暂不实现）**：

```
[触发器] clipboard_changed（剪贴板出现新内容）
   └─> [节点1] 代码美化插件 . save(text)      → 保存并输出格式化结果
          └─> [节点2] 截图插件 . render({ content: 节点1.formatted })
                                              → 生成一张截图文件
```

一次剪贴板事件，多个插件按图依次联动处理——工作流引擎负责调度与数据传递，插件只实现自己的 action。

### 7.2 数据模型

```typescript
// src/types/index.ts
interface Workflow {
  id: string
  name: string
  description: string
  enabled: boolean
  graph: WorkflowGraph
  created_at: string
  updated_at: string
}

/** DAG 图，持久化为 JSON 存 workflows.graph */
interface WorkflowGraph {
  nodes: WorkflowNode[]
  edges: { from: string; to: string }[]          // 节点执行依赖
  bindings: DataBinding[]                        // 节点间数据映射
}

interface WorkflowNode {
  id: string                                     // 如 'n1'
  type: 'plugin' | 'transform'                   // v1 内置 transform（JSON 映射，无代码）
  plugin_id?: string                             // type=plugin 时
  action?: string                                // 插件 action 名
  config?: Record<string, unknown>               // 节点静态配置
}

interface DataBinding {
  from: string                                   // 源节点 id
  output: string                                 // 源节点输出 key
  to: string                                     // 目标节点 id
  input: string                                  // 目标节点输入 key
}

interface WorkflowTrigger {
  id: string
  type: 'clipboard' | 'hotkey' | 'schedule' | 'manual' | 'plugin_event'
  config: Record<string, unknown>                // 如 { hotkey: 'CmdOrCtrl+Shift+B', cron: '...' }
  workflow_id: string                            // 触发后执行的工作流
}
```

> 触发器不放入 `graph`，由引擎单独管理注册表（内存态），触发配置冗余存 `graph` 顶层便于编辑器渲染，加载时重建注册。

### 7.3 执行引擎（Rust，DAG 调度）

```rust
// src-tauri/src/workflow_engine.rs（示意）
pub struct WorkflowEngine {
    wasm: Arc<WasmRuntime>,
    triggers: TriggerManager,          // clipboard / hotkey / schedule / manual / plugin_event
}

impl WorkflowEngine {
    pub async fn execute(&self, workflow_id: &str, trigger_data: Value) -> Result<RunResult, String> {
        // 1. 加载 graph，校验无环（拓扑排序），无入边节点为起始集
        // 2. INSERT workflow_runs (status='running')
        // 3. 调度：入度归零的节点并发执行（tokio::join）
        //    每节点：
        //    a. 按 bindings 从上下文拼装输入（含 trigger_data）
        //    b. type=plugin → wasm.invoke_action；type=transform → 纯 JSON 映射
        //    c. 写 workflow_step_logs（input/output/duration/status）
        //    d. 失败 → v1 策略 fail_fast：整条 run 标记 failed 并取消未开始节点(skipped)
        // 4. 全部完成 → 更新 run status/finished_at，返回 RunResult
    }
}

/// 节点执行上下文：trigger 原始数据 + 各节点已产出输出
struct WorkflowContext {
    trigger: Value,
    outputs: HashMap<String /* node_id */, Value>,
}
```

要点：

- **真正的 DAG 语义**：按拓扑序调度，无依赖关系的节点**并行执行**，数据流由 `bindings` 显式映射（区别于 v1.0 文档的线性遍历）。
- **两级日志**：`workflow_runs`（一次触发）+ `workflow_step_logs`（每节点输入/输出/耗时），编辑器与日志页可逐步回放。
- **触发器**：clipboard 用 `tauri-plugin-clipboard-manager` 监听；hotkey 用 `tauri-plugin-global-shortcut`；schedule 用 tokio 定时；plugin_event 监听事件总线（§3.3 plugin_events）。

---

## 8. 设置系统

### 8.1 存储

全部设置统一存 `settings` 表（key-value，value 为 JSON 编码），**Dock 设置以 `dock_` 前缀存于同一张表**，无独立 dock_settings 表。

```typescript
// src/stores/settings.ts
import { create } from 'zustand'
import { settingsDb } from '@/lib/db'

interface SettingsStore {
  settings: AppSettings
  loading: boolean
  loadSettings: () => Promise<void>
  updateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => Promise<void>
  resetSettings: () => Promise<void>
}

export const useSettingsStore = create<SettingsStore>((set) => ({
  settings: DEFAULT_SETTINGS,
  loading: false,

  loadSettings: async () => {
    set({ loading: true })
    try {
      const raw = await settingsDb.getAll()
      const settings = { ...DEFAULT_SETTINGS }
      for (const [key, value] of Object.entries(raw)) {
        try { (settings as Record<string, unknown>)[key] = JSON.parse(value) }
        catch { /* 跳过非法值，保留默认 */ }
      }
      set({ settings, loading: false })
    } catch (error) {
      console.error('加载设置失败:', error)
      set({ loading: false })
    }
  },

  updateSetting: async (key, value) => {
    await settingsDb.set(key, JSON.stringify(value))
    set((state) => ({ settings: { ...state.settings, [key]: value } }))
  },

  resetSettings: async () => {
    for (const [key, value] of Object.entries(DEFAULT_SETTINGS)) {
      await settingsDb.set(key, JSON.stringify(value))
    }
    set({ settings: DEFAULT_SETTINGS })
  },
}))
```

设置变更实时生效：`dock_enabled` → 通知 Rust 挂起/恢复轮询；`theme`/`accent_color` → 切换 Tailwind `dark` class 与 CSS 变量。

### 8.2 设置项清单（与 §3.4 种子一一对应）

| 分类 | 设置项 | Key | 类型 / 取值 |
| ------ | -------- | ----- | ------------ |
| **通用** | 语言 | `language` | `zh-CN` / `en-US` |
| | 开机启动 | `auto_start` | bool（tauri-plugin-autostart） |
| | 更新检查 | `check_update` | bool |
| **主题** | 主题模式 | `theme` | `light` / `dark` / `system` |
| | 强调色 | `accent_color` | 色值 `#007AFF` |
| **Dock** | 启用 Dock | `dock_enabled` | bool（打开/关闭 Dock 栏） |
| | 热区宽度 | `dock_hotzone_width` | 6–24 px |
| | 动画速度 | `dock_animation_speed` | `slow` / `medium` / `fast` |
| | 展开延迟 | `dock_expand_delay` | ms |
| | 自动隐藏延迟 | `dock_auto_hide_delay` | ms |
| | 可见数量 | `dock_visible_items` | 5–13 |
| | 位置 | `dock_position` | `right`（预留 left） |
| **插件** | 自动更新 | `plugin_auto_update` | bool |
| | 沙箱严格模式 | `plugin_sandbox_strict` | bool（true 时未签名插件拒绝加载） |
| | 日志级别 | `plugin_log_level` | `debug` / `info` / `warn` / `error` |

设置页使用 shadcn/ui 表单组件（`Switch` / `Select` / `Slider` / 颜色选择器），按上表分组渲染。

---

## 9. 技术实现

### 9.1 项目结构

```
kapi/
├── src/
│   ├── components/
│   │   ├── ui/                          # shadcn/ui 生成组件
│   │   ├── navigation/
│   │   │   ├── AppSidebar.tsx           # 主面板侧边栏（shadcn Sidebar）
│   │   │   └── TopBar.tsx
│   │   ├── dock/
│   │   │   ├── DockWindow.tsx           # Dock 前端（弧形布局渲染）
│   │   │   ├── DockItem.tsx
│   │   │   └── useDock.ts
│   │   ├── plugin/
│   │   │   ├── PluginHost.tsx           # 统一插件宿主（iframe + 桥接，§4.3）
│   │   │   └── PluginCard.tsx
│   │   └── workflow/
│   │       ├── WorkflowCanvas.tsx       # React Flow 画布
│   │       ├── WorkflowNode.tsx
│   │       └── WorkflowRunPanel.tsx     # 运行历史/步骤日志
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   ├── Plugins.tsx
│   │   ├── Store.tsx
│   │   ├── Workflow.tsx
│   │   ├── PluginEmbedView.tsx          # /plugin/:id（带侧边栏外壳）
│   │   ├── PluginWindowShell.tsx        # /plugin-window/:id（独立窗口裸壳）
│   │   ├── Logs.tsx
│   │   └── Settings.tsx
│   ├── stores/
│   │   ├── settings.ts
│   │   ├── plugins.ts
│   │   └── workflow.ts
│   ├── lib/
│   │   ├── db.ts                        # 数据库访问层（§3.6）
│   │   ├── tauri.ts                     # invoke/listen 封装
│   │   └── dock-arc.ts                  # 弧线纯函数计算
│   ├── types/
│   │   └── index.ts
│   ├── App.tsx
│   ├── main.tsx
│   └── routes.tsx
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs                       # Builder 装配：插件注册/迁移/协议/命令
│   │   ├── db.rs                        # 连接与迁移装配
│   │   ├── dock_service.rs              # 热区轮询（边沿触发，§5.4）
│   │   ├── plugin_manager.rs            # 安装/卸载/窗口创建
│   │   ├── plugin_protocol.rs           # kapi-plugin:// 自定义协议
│   │   ├── plugin_bridge.rs             # 统一权限检查 + 桥接分发
│   │   ├── wasm_runtime.rs              # wasmtime 运行时（§6.5）
│   │   ├── workflow_engine.rs           # DAG 调度引擎（§7.3）
│   │   └── github_client.rs             # 插件市场来源
│   ├── migrations/
│   │   ├── 001_init.sql
│   │   └── 002_defaults.sql
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

### 9.2 数据库初始化（Rust 侧迁移，唯一入口）

```rust
// src-tauri/src/db.rs
use tauri_plugin_sql::{Migration, MigrationKind};

pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "init tables",
            sql: include_str!("../migrations/001_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "seed default settings",
            sql: include_str!("../migrations/002_defaults.sql"),
            kind: MigrationKind::Up,
        },
    ]
}

// lib.rs 装配（迁移随插件加载自动执行且只执行一次）
// tauri::Builder::default()
//     .plugin(
//         tauri_plugin_sql::Builder::default()
//             .add_migrations("sqlite:kapi.db", db::migrations())
//             .build(),
//     )
```

### 9.3 Tauri 命令（v2 API）

```rust
// src-tauri/src/lib.rs（示意）
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

/// 统一插件启动入口（Dock / 主面板 / 快捷键共用，§2.3）
#[tauri::command]
async fn launch_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let plugin = state.plugin_manager.get(&plugin_id).await?;

    match plugin.window_mode.as_str() {
        // 内嵌：通知主窗口路由切换；主窗口隐藏时先唤起
        "embedded" => {
            if let Some(win) = app.get_webview_window("main") {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
                win.emit("plugin:navigate", &plugin_id).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        // 独立窗口：存在则聚焦，不存在按 manifest.window 创建
        "independent" => {
            let label = format!("plugin-{}", plugin_id);
            if let Some(win) = app.get_webview_window(&label) {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
            } else {
                state.plugin_manager.create_window(&app, &plugin).await?;
            }
            Ok(())
        }
        // headless：直接执行一次默认动作（工作流插件）
        _ => state.workflow_engine.execute_plugin_once(&plugin_id).await,
    }
}

/// 插件桥接统一入口：权限检查 + 分发（§6.4）
#[tauri::command]
async fn plugin_bridge(
    state: State<'_, AppState>,
    plugin_id: String,
    channel: String,          // 'kapi:storage.get' / 'kapi:plugin.invoke' / ...
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    state.bridge.dispatch(&plugin_id, &channel, payload).await
}
```

### 9.4 自定义协议（插件 UI 资源服务）

```rust
// src-tauri/src/plugin_protocol.rs（示意）
// 注册 kapi-plugin 协议，将插件静态资源映射到安装目录：
//   kapi-plugin://localhost/<plugin_id>/<path>  →  {app_data}/plugins/<plugin_id>/web/<path>
//   (Windows WebView2 下实际 URL 形如 http://kapi-plugin.localhost/<plugin_id>/<path>)
//
// 安全要求：
//   1. 路径规范化，拒绝包含 ".." 的穿越请求
//   2. 仅允许 web/ 子目录，main.wasm 等逻辑文件不可经协议访问
//   3. 响应带正确 Content-Type；默认 no-store 避免更新后缓存旧资源
```

### 9.5 窗口配置（tauri.conf.json5 摘要）

> 实际使用 `tauri.conf.json5`（JSON5）：本机透明加密软件会加密 `.json` 文件导致 cargo 构建失败，
> 详见 §10.4 风险表；capability 内联于 `app.security.capabilities`，无 capabilities/ 目录。

```jsonc
{
  "app": {
    "windows": [
      // 主面板：对齐 Electron panel（1200×1000，无边框自绘标题栏）
      { "label": "main", "title": "Kapi", "width": 1200, "height": 1000, "decorations": false },
      // Dock：完整参数见 §5.1；位置由 Rust 启动时按光标所在显示器计算
      {
        "label": "dock",
        "width": 320, "height": 560,
        "transparent": true, "decorations": false, "shadow": false,
        "alwaysOnTop": true, "skipTaskbar": true,
        "visible": false, "resizable": false
      }
    ]
  }
}
```

前端按窗口 label 区分入口：`dock` 窗口加载 Dock UI，`main` 加载主面板；独立插件窗口由 Rust 创建并指向 `/plugin-window/:id`。主面板**关闭 = 隐藏复用**（Rust 拦截 `CloseRequested` → prevent + hide，对齐 Electron 的 close 拦截模式），应用退出流程中放行。

---

## 10. 开发计划

### 10.1 里程碑

| 阶段 | 周期 | 目标 |
| ------ | ------ | ------ |
| **Phase 1** | 1 周 | 项目初始化（Tailwind + shadcn/ui + Router + Zustand + Tauri 插件注册）、数据库迁移与访问层 |
| **Phase 2** | 2 周 | 主面板框架：侧边栏导航、页面路由、主题系统、设置页（含 Dock 开关） |
| **Phase 3** | 2 周 | Dock 侧边栏：Rust 光标轮询（跨平台）、透明窗口、弧形 UI、唤醒链路（launch_plugin） |
| **Phase 4** | 3 周 | 插件系统：kapi-plugin 协议、PluginHost（内嵌+独立窗口）、wasmtime 运行时、桥接 API 与权限模型 |
| **Phase 5** | 1 周 | 插件市场：GitHub 来源浏览、下载安装、卸载更新 |
| **Phase 6** | 2 周 | 工作流：触发器管理、DAG 引擎与两级日志、React Flow 编辑器 |
| **Phase 7** | 1 周 | 打磨、插件签名验证（严格模式收尾）、测试、打包 |

### 10.2 当前任务

- [x] 确定技术方案
- [x] 完善设计文档（v2.0.0，含数据库结构与插件系统细化）
- [x] 数据库设计（8 表 + 索引 + 迁移规划）
- [x] 初始化项目（Tailwind v4 + shadcn/ui + Router + Zustand + vitest，2026-08-25）
- [x] 实现数据库层（Rust 迁移 + 前端 db.ts，2026-08-25）
- [ ] 实现主面板框架
- [ ] 移植 Dock 侧边栏

### 10.3 待开发清单

- [x] 项目初始化（Tauri + React 19 + Tailwind + shadcn/ui）
- [x] 数据库迁移（tauri-plugin-sql Migration，Rust 侧唯一入口）
- [x] i18n（react-i18next，zh-CN / en-US，TS 语言包规避 .json 加密）
- [ ] 主面板布局（侧边栏 + 内容区 + 路由）
- [ ] 设置页面（统一 settings 表 + 实时生效：Dock 开关/主题/强调色）
- [ ] 主题系统（light/dark/system + accent CSS 变量）
- [ ] Dock 窗口（边沿触发热区轮询 + motion 弧形前端 + 仅唤醒，§5）
- [ ] kapi-plugin:// 自定义协议（静态资源服务 + 路径安全）
- [ ] PluginHost（iframe + postMessage 桥接，内嵌/独立共用）
- [ ] 插件独立窗口（manifest 自定义窗口参数，重复点击聚焦）
- [ ] 插件管理器（安装/卸载/启停/模式切换/排序）
- [ ] wasmtime 运行时（kapi_invoke ABI + 宿主函数 + fuel 限制）
- [ ] 桥接 API 与权限模型（PermissionGuard，默认拒绝）
- [ ] @kapi/plugin-sdk（插件前端 SDK）与 kapi-plugin-sdk（WASM Rust SDK）
- [ ] 插件市场（GitHub API + 安装流程）
- [ ] 工作流引擎（触发器 + DAG 调度 + 两级日志）
- [ ] 工作流编辑器（React Flow）
- [ ] 全局快捷键（Alt+Space 唤醒 Dock、hotkey 触发器）
- [ ] 打包配置与签名

### 10.4 风险与待定项

| 项 | 说明 | 缓解 |
| ---- | ------ | ------ |
| Electron Dock 移植精度 | v2.1.0 已与源码逐项对齐（§5.1–5.5：窗口参数/几何/状态机/轮询算法/穿透策略） | 遗留差异一项：Tauri 无 mousemove 转发，见 §5.3 注 |
| 插件签名验证 | 严格模式依赖签名，签名体系（算法/密钥分发）未设计 | Phase 7 收尾；开发期严格模式默认关 |
| Wayland 下 Dock 热区 | X11 轮询在 Wayland 不可用 | 降级为快捷键唤醒，文档已注明 |
| **本机透明加密软件（TSD）** | 开发机有 DLP 类软件按扩展名加密 .json/.txt（受信进程写入时加密，cargo 等非受信进程读到密文导致构建失败；实测 .ts/.tsx/.toml/.json5/.md 不受影响） | **Tauri 配置一律用 `tauri.conf.json5`（Cargo 已启用 config-json5 feature），capability 内联进配置而非 capabilities/*.json；新增配置文件避免 .json/.txt 扩展名**。根治需在加密软件中排除项目目录或加白 cargo（待用户处理） |
| WASM ABI 稳定性 | JSON 字符串 ABI 简单但弱类型 | kapi_version 字段 + 后续可迁移 WIT 组件模型 |

---

## 11. 变更记录

### v2.2.0 (2026-08-25)

**Phase 1 落地与实现修正**：

24. React Router 实际安装 v7（v6 API 兼容），技术栈表已更新。
25. Tauri 配置迁移到 `tauri.conf.json5` + `config-json5` feature，capability 内联进配置——规避本机透明加密软件对 `.json` 的加密（§10.4 新增风险行）。
26. §10.2 / §10.3 勾选 Phase 1 完成项（项目初始化 + 数据库层）。
27. 新增 i18n：react-i18next + TS 语言包（`src/i18n/locales/`，zh-CN / en-US，key 结构一致性有单元测试），语言切换持久化于 `settings.language`；注释规范改为两行制双语（CLAUDE.md §一）。

### v2.1.0 (2026-08-25)

**Dock 章节与 Electron 版源码逐项对齐**（依据 `kapi-main/src/main/dock.ts`、`window-manager.ts`、`platform-adapter/windows.ts`、`renderer/src/dock/*`、`renderer/src/lib/arc-math.ts`、`shared/types.ts`）：

 1. §5 全面重写：补 `peeking` 中间态（三态状态机）、边沿触发热区算法、120ms 冷却、"收起后延迟恢复鼠标穿透"策略、"展开/收起永不 setBounds"原则（透明窗口 resize 闪烁）。
 2. 窗口参数补全：`resizable/movable: false`、`shadow: false`、初始 `visible: false`、启动时按光标显示器定位一次；主面板对齐 Electron panel（1200×1000 无边框、关闭 = 隐藏复用）。
 3. 视觉规格落地：箭头触发器 12×150px、轨道 SVG 路径与虚线参数、选中态 1.12 倍放大、motion 动画时长与缓动曲线；技术栈新增 motion (framer-motion)。
 4. 明确 Tauri 移植差异：`set_ignore_cursor_events` 无 mousemove 转发，Rust 轮询为展开唯一权威路径（与 Electron 实际行为一致）。
 5. 明确架构边界：**仅 Dock 对齐 Electron**；插件系统基于 WASM（wasmtime）全新设计（§6 头注），数据库/工作流同为全新设计。
 6. 点击行为差异显式化：Electron 现状为"滚动到中心"，Tauri 版按需求改为 `launch_plugin` 唤醒分发（§5.5）。

### v2.0.0 (2026-08-25)

**修复 v1.0.0 的文档矛盾**：

1. 补建 `plugin_data` 表 DDL（v1.0 §6.3 使用但未定义），复合主键 + WITHOUT ROWID。
2. 取消独立 `dock_settings` 表，Dock 设置统一为 `settings` 表 `dock_*` 前缀键，与设置页 loadSettings 单一数据源。
3. `window_mode` 明确三值 `embedded | independent | headless`（含 CHECK 约束），与 §2.3 分发逻辑一致。
4. 统一命令名为 `launch_plugin`（v1.0 前端调用 `launch_plugin_window` 与 Rust 定义不一致）。
5. 工作流引擎由线性遍历改为**拓扑排序 DAG 调度**，`edges`/`bindings` 真正参与执行；日志拆分为 `workflow_runs` + `workflow_step_logs` 两级。
6. 项目结构补充 `migrations/` 目录，与 `include_str!` 引用一致。
7. React 版本对齐实际依赖（React 19）。

**技术方案修正**：

1. `app.emit_all`（v1 API）改为 Tauri v2 `Emitter::emit`。
2. 删除不可编译的 `tauri::plugin::sql` 伪代码，改为 tauri-plugin-sql 官方 `Migration` 机制，迁移收敛到 Rust 单一入口。
3. 内嵌插件渲染由 `innerHTML` 改为 **iframe + kapi-plugin:// 自定义协议 + PluginHost 桥接**（隔离/脚本执行/资源路径三问题一并解决）。
4. 补全 wasmtime 运行时设计：ABI、宿主函数、fuel 资源限制。
5. 补全跨平台光标轮询方案与 Wayland 降级策略、全局快捷键与剪贴板 Tauri 插件选型。

**需求细化（本次新增）**：

 1. 明确"Dock 只负责唤醒"：分发逻辑全部集中 Rust `launch_plugin`，Dock 前端零插件逻辑。
 2. 明确主面板"左导航 + 右内容"布局与 shadcn/ui 组件选型，路由表扩充 `/plugin-window/:id` 裸壳路由。
 3. 插件独立窗口：manifest.window 自定义（标题/尺寸/可缩放/置顶），用户可切换运行模式，重复点击聚焦已有窗口。
 4. 工作流补充典型场景（剪贴板 → 代码美化保存 → 截图生成）作为能力目标，示例插件本期不实现。
 5. 数据库结构完整设计：8 张表、外键级联策略、索引、种子数据、迁移文件规划。
