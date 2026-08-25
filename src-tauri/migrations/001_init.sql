-- ============================================================
-- 001_init.sql  Kapi 初始表结构
-- 对应设计文档 docs/plan.MD §3.3
-- 迁移由 tauri-plugin-sql 在 Rust 侧执行（唯一入口，见 src/db.rs）
-- 规则：已发布的迁移文件禁止修改，只允许追加新版本
-- ============================================================

PRAGMA foreign_keys = ON;

-- ============ 插件注册表 / Plugin registry ============
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

-- ============ 插件数据存储（隔离 KV）/ Plugin-scoped KV ============
-- 每个插件只能读写自己 plugin_id 命名空间下的数据（由 Rust 权限层强制）
CREATE TABLE plugin_data (
    plugin_id  TEXT NOT NULL REFERENCES plugins(id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,            -- JSON 编码值
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (plugin_id, key)
) WITHOUT ROWID;

-- ============ 工作流定义 / Workflow definition ============
CREATE TABLE workflows (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT,
    graph       TEXT NOT NULL,           -- DAG JSON: {nodes, edges, bindings}，见 plan §7.2
    is_enabled  INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 工作流执行实例 / Workflow run ============
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

-- ============ 工作流步骤日志 / Workflow step log ============
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

-- ============ 应用设置（统一表，含 Dock 设置）/ Unified settings ============
CREATE TABLE settings (
    key        TEXT PRIMARY KEY,         -- 如 theme / dock_enabled / dock_position
    value      TEXT NOT NULL,            -- JSON 编码：'"zh-CN"' / 'true' / '12'
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 插件事件表 / Plugin event history ============
-- 事件总线的历史记录：插件 emit 的事件 + 系统事件，用于触发工作流与审计
CREATE TABLE plugin_events (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type       TEXT NOT NULL,      -- 如 clipboard_changed / processed / saved
    source_plugin_id TEXT REFERENCES plugins(id) ON DELETE SET NULL,  -- NULL = 系统事件
    data             TEXT,               -- JSON
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 系统日志 / System log ============
CREATE TABLE system_logs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    level      TEXT NOT NULL CHECK (level IN ('debug', 'info', 'warn', 'error')),
    message    TEXT NOT NULL,
    source     TEXT,                     -- 模块名，如 dock_service / workflow_engine
    data       TEXT,                     -- JSON
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============ 索引 / Indexes ============
CREATE INDEX idx_plugins_category   ON plugins(category);
CREATE INDEX idx_plugins_sort       ON plugins(sort_order);
CREATE INDEX idx_runs_workflow      ON workflow_runs(workflow_id, started_at);
CREATE INDEX idx_step_logs_run      ON workflow_step_logs(run_id);
CREATE INDEX idx_events_type        ON plugin_events(event_type, created_at);
CREATE INDEX idx_events_plugin      ON plugin_events(source_plugin_id);
CREATE INDEX idx_syslogs_level      ON system_logs(level);
CREATE INDEX idx_syslogs_created    ON system_logs(created_at);
