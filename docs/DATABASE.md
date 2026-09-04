# 数据库设计

Kapi 技术文档：SQLite 表结构、迁移与前端访问层。

## 1. 选型与迁移策略

使用 **SQLite**，通过 `tauri-plugin-sql` 访问。

**迁移唯一入口在 Rust 侧**：setup 阶段由 `workflow::db::open_pool_with_migrations` 同步连接并执行迁移（先于触发器启动）；前端 `Database.load('sqlite:kapi.db')` 为幂等打开（两侧共用 `_sqlx_migrations` 版本表，checksum 一致，后到者自动跳过）。

### 1.1 写者矩阵 / Writer Matrix

目标态：**每张表只有一个写者入口**。前端数据面已只读化（`src/lib/db.ts` 仅 SELECT，capability 不再放行 `sql:allow-execute`），所有写操作收敛为 Rust 命令或 Rust 内部路径：

| 表 | 写者 | 入口 | 说明 |
| -- | ---- | ---- | ---- |
| `plugins` | Rust | `plugin_install` / `store_install`（安装/更新）· `plugin_uninstall` · `plugin_set_enabled` · `plugin_set_window_mode` · `plugin_reorder` | 模式合法性在写路径按 manifest 校验 |
| `plugin_data` | Rust | 插件 `kv.set` 等桥接命令（权限层强制命名空间隔离） | 外键级联随 plugins 删除 |
| `workflows` | Rust | `workflow_save` / `workflow_delete` 命令 | 引擎与命令路径共用同一池 |
| `workflow_triggers` | Rust | `trigger_save` / `trigger_delete` 命令（引擎订阅进程内总线，无游标表） | |
| `workflow_runs` / `workflow_step_logs` | Rust | 引擎执行路径内部写入 | |
| `settings` | Rust | `settings_set` 命令 · `store_list`/`store_install` 内部缓存写入 | 前端广播 `settings:changed` 仅同步内存态 |
| `plugin_events` | Rust | `events.emit` 桥接路径落库（仅审计历史，触发器走进程内总线） | janitor 每小时保留最新 10000 条 |
| `system_logs` | Rust | `bridge::log` 内部写入 | janitor 每小时保留最新 20000 条 |

前端读路径：`src/lib/db.ts`（pluginDb / settingsDb / logDb / eventDb，全部 SELECT）。

## 2. 表结构总览

| 表 | 用途 | 关键关系 |
| -- | ---- | -------- |
| `plugins` | 已安装插件注册表 | 被 plugin_data / plugin_events 引用 |
| `plugin_data` | 插件隔离的 KV 存储（每插件仅能访问自己的命名空间） | `(plugin_id, key)` 复合主键 |
| `workflows` | 工作流定义（DAG 图存 JSON） | — |
| `workflow_triggers` | 工作流触发器（schedule / plugin_event / clipboard / hotkey） | CASCADE 删随 workflow |
| `workflow_runs` | 工作流执行实例（一次触发一条） | CASCADE 删随 workflow |
| `workflow_step_logs` | 步骤级执行日志（输入/输出/耗时，可追溯） | CASCADE 删随 run |
| `settings` | **统一**应用设置表（含 `dock_*` 前缀项，无独立 dock 表） | — |
| `plugin_events` | 事件总线历史（触发工作流 + 审计） | `ON DELETE SET NULL` |
| `system_logs` | 系统日志 | — |

## 3. 完整建表语句

```sql
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
    graph       TEXT NOT NULL,           -- DAG JSON: {nodes, edges, bindings}
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 工作流触发器 ============
CREATE TABLE workflow_triggers (
    id            TEXT PRIMARY KEY,
    workflow_id   TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    trigger_type  TEXT NOT NULL
                  CHECK (trigger_type IN ('schedule', 'plugin_event', 'clipboard', 'hotkey')),
    config        TEXT NOT NULL,         -- JSON: {cron} / {event_type} / {content_type} / {hotkey}
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
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
CREATE TABLE plugin_events (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type       TEXT NOT NULL,      -- 如 clipboard_changed / processed / saved
    source_plugin_id TEXT REFERENCES plugins(id) ON DELETE SET NULL,
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
CREATE INDEX idx_triggers_workflow  ON workflow_triggers(workflow_id);
CREATE INDEX idx_events_type        ON plugin_events(event_type, created_at);
CREATE INDEX idx_events_plugin      ON plugin_events(source_plugin_id);
CREATE INDEX idx_syslogs_level      ON system_logs(level);
CREATE INDEX idx_syslogs_created    ON system_logs(created_at);
```

> 说明：`plugin_data` 使用 `(plugin_id, key)` 复合主键 + `WITHOUT ROWID`，无需额外索引；所有外键均带 `ON DELETE` 策略，删除插件/工作流不产生孤儿数据。

## 4. 默认数据

```sql
INSERT OR IGNORE INTO settings (key, value) VALUES
    -- 通用
    ('language',              '"zh-CN"'),
    ('auto_start',            'false'),
    ('check_update',          'true'),
    -- 主题
    ('theme',                 '"system"'),   -- 'light' | 'dark' | 'system'
    ('accent_color',          '"#007AFF"'),
    -- Dock（统一存 settings 表，dock_ 前缀，无独立 dock 表）
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

## 5. 迁移文件规划

```text
src-tauri/migrations/
├── 001_init.sql        # 建表 + 索引（本文档 §3）
├── 002_defaults.sql    # 默认设置种子（本文档 §4）
├── 003_wal.sql         # 启用 WAL 日志模式（Rust 桥接与前端并发读写，修复 database is locked）
└── 004_xxx.sql         # 后续按版本递增，禁止修改已发布的迁移文件
```

## 6. 数据库访问层（前端）

完整实现见 `src/lib/db.ts`（`pluginDb` / `pluginDataDb` / `workflowDb` / `workflowTriggerDb` / `settingsDb` / `eventDb` / `logDb`）。要点：

- `initDb()` 仅获取连接，迁移已由 Rust 完成；重复调用复用连接
- `pluginDb` 返回解析后的对象（`manifest` / `window_config` 反序列化）
- `workflowDb.getRuns()` 两级查询：runs + 批量步骤日志
- `workflowTriggerDb` 触发器 CRUD（save / delete / list）
- `eventDb.getRecent()` 获取历史事件（用于 PluginEvent 触发器事件类型选择）
- `settingsDb.set()` 使用 `ON CONFLICT DO UPDATE` upsert

## 7. 触发器配置 JSON 格式

```typescript
// schedule: { cron: string }
{ "cron": "0 * * * * *" }        // 每分钟
{ "cron": "0 9 * * *" }          // 每天 9:00
{ "cron": "*/5 * * * *" }        // 每 5 分钟

// plugin_event: { event_type: string }
{ "event_type": "clipboard.changed" }

// clipboard: { content_type?: 'text' | 'image' }
{ "content_type": "text" }
{}

// hotkey: { hotkey: string }
{ "hotkey": "CmdOrCtrl+Shift+B" }
{ "hotkey": "Alt+Space" }
```
