-- ============ 工作流 graph schema 版本 ============
-- Workflow graph schema version
-- graph JSON 中节点携带 position、边携带 map（v1）；列上恒为当前版本
-- Nodes carry position and edges carry map in graph JSON (v1); the column is pinned to the current version
ALTER TABLE workflows ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1;
