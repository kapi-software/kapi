-- ============================================================
-- 005_trigger_cursors.sql  触发器游标：plugin_event 的 last_event_id 持久化
-- Trigger cursors: persist last_event_id for plugin_event triggers
-- 避免重启后把历史事件从头重放一遍
-- Avoid replaying all history after a restart
-- ============================================================

CREATE TABLE trigger_cursors (
    trigger_id      TEXT PRIMARY KEY REFERENCES workflow_triggers(id) ON DELETE CASCADE,
    last_event_id   INTEGER NOT NULL DEFAULT 0,  -- 最后已处理 plugin_events.id
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
