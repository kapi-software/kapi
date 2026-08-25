-- ============================================================
-- 002_defaults.sql  Kapi 默认设置种子
-- 对应设计文档 docs/plan.MD §3.4
-- INSERT OR IGNORE：用户已修改过的设置不会被覆盖
-- ============================================================

INSERT OR IGNORE INTO settings (key, value) VALUES
    -- 通用 / General
    ('language',              '"zh-CN"'),
    ('auto_start',            'false'),
    ('check_update',          'true'),
    -- 主题 / Theme
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
    -- 插件 / Plugin
    ('plugin_auto_update',    'false'),
    ('plugin_sandbox_strict', 'true'),
    ('plugin_log_level',      '"info"');
