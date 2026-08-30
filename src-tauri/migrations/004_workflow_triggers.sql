-- ============================================================
-- 004_workflow_triggers.sql  工作流触发器持久化
-- Persistent workflow triggers
-- 触发器配置从工作流定义中分离：工作流是 DAG + 节点，触发器是激活条件
-- Trigger config is decoupled from workflow definition:
-- a workflow is DAG + nodes, a trigger is the activation condition
-- ============================================================

CREATE TABLE workflow_triggers (
    id            TEXT PRIMARY KEY,       -- 触发器实例 ID（与 WorkflowEngine 注册表共用）
                                         -- Trigger instance id (shared with WorkflowEngine registry)
    workflow_id   TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    trigger_type  TEXT NOT NULL,          -- clipboard / hotkey / schedule / plugin_event
                                         -- clipboard / hotkey / schedule / plugin_event
    config        TEXT NOT NULL,          -- 触发器配置 JSON
                                         -- Trigger configuration JSON
    is_enabled    INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_triggers_workflow ON workflow_triggers(workflow_id);
CREATE INDEX idx_triggers_type     ON workflow_triggers(trigger_type);
