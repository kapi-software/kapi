-- 007_drop_trigger_cursors.sql  删除触发器游标表
-- 007_drop_trigger_cursors.sql  drop the trigger-cursor table
-- plugin_event 触发器已改为订阅进程内事件总线（bridge::event_bus），
-- plugin_events 表仅作审计历史，last_event_id 游标不再有读写方
-- plugin_event triggers now subscribe to the in-process event bus
-- (bridge::event_bus); plugin_events is audit-only history, so the
-- last_event_id cursor has no readers or writers left
DROP TABLE IF EXISTS trigger_cursors;
