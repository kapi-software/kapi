// 全局类型定义：插件、工作流、日志
// Global types: plugins, workflows, logs
// 对应 docs/PLUGINS.md 与 docs/WORKFLOW.md

// ============================================================
// 插件 / Plugin
// ============================================================

// 插件运行模式（docs/PLUGINS.md §2.1）
// Plugin window mode (docs/PLUGINS.md §2.1)
export type WindowMode = 'embedded' | 'independent' | 'headless'

// manifest.window：独立窗口自定义参数（对齐 Tauri 窗口选项，docs/PLUGINS.md §2.2）
// Custom window config from the plugin manifest (aligned with Tauri window options)
export interface PluginWindowConfig {
  mode: WindowMode
  title?: string
  width?: number
  height?: number
  minWidth?: number
  minHeight?: number
  resizable?: boolean
  alwaysOnTop?: boolean
  // 透明背景：窗口与页面 html/body 双透明（Linux X11 无合成器时退化为黑底）
  // Transparent: window + html/body both transparent (black on X11 without a compositor)
  transparent?: boolean
  // 无边框（隐藏标题栏）；配合 startDragging 通道实现自绘拖拽区
  // Frameless (hides the title bar); pair with the startDragging channel for custom drag areas
  decorations?: boolean
  // 不在任务栏显示（macOS 会同时从 Cmd-Tab 隐藏）
  // Hide from the taskbar (also hides from Cmd-Tab on macOS)
  skipTaskbar?: boolean
  // 窗口投影（仅 Windows/Linux 生效，macOS 忽略）
  // Window shadow (Windows/Linux only; ignored on macOS)
  shadow?: boolean
  // 居中创建（默认 true）
  // Center on creation (default true)
  center?: boolean
  fullscreen?: boolean
}

// 工作流能力声明（manifest.workflow）
// Workflow capability declaration in the manifest
export interface PluginWorkflowSpec {
  triggers?: string[]
  actions?: Array<{
    name: string
    inputs?: Record<string, string>
    outputs?: Record<string, string>
  }>
  events?: string[]
}

// 插件 manifest（安装包内 manifest.json）
// Plugin manifest (manifest.json inside the package)
export interface PluginManifest {
  id: string
  name: string
  version: string
  kapi_version?: string
  author?: string
  description?: string
  window?: PluginWindowConfig
  workflow?: PluginWorkflowSpec
  // 权限声明，默认全部拒绝
  // Declared permissions, deny by default
  permissions?: string[]
}

// 插件（解析后，manifest 已为对象）
// Plugin (parsed, manifest is an object)
export interface Plugin {
  id: string
  name: string
  version: string
  author: string | null
  description: string | null
  icon: string | null
  category: string | null
  manifest: PluginManifest
  install_path: string
  wasm_path: string | null
  web_path: string | null
  window_mode: WindowMode
  // manifest 声明的可运行形态（展示用，裁决在 Rust launch_plugin）
  // Declared runnable shapes (display-only; Rust launch_plugin decides)
  supported_modes: WindowMode[]
  window_config: PluginWindowConfig | null
  is_enabled: boolean
  is_installed: boolean
  sort_order: number
  installed_at: string
  updated_at: string
}

// plugins 表原始行（manifest / window_config 为 JSON 字符串，supported_modes 派生不入库）
// Raw plugins row (manifest / window_config as JSON strings; supported_modes is derived, not stored)
export interface PluginRow extends Omit<Plugin, 'manifest' | 'window_config' | 'supported_modes'> {
  manifest: string
  window_config: string | null
}

// ============================================================
// 工作流 / Workflow（docs/WORKFLOW.md §2）
// ============================================================

// DAG 图，持久化为 JSON 存 workflows.graph
// DAG graph persisted as JSON in workflows.graph
export interface WorkflowGraph {
  nodes: WorkflowNode[]
  edges: Array<{ from: string; to: string }>
  // 节点间数据映射
  // Data mapping between nodes
  bindings: DataBinding[]
}

// 工作流节点
// Workflow node
export interface WorkflowNode {
  id: string
  type: 'plugin' | 'transform'
  plugin_id?: string
  action?: string
  config?: Record<string, unknown>
  // 画布坐标（React Flow position），打开工作流时还原画布布局
  // Canvas position (React Flow position), restores layout when reopening
  position?: { x: number; y: number }
}

// 数据绑定：源节点输出 key → 目标节点输入 key
// Data binding: source output key → target input key
export interface DataBinding {
  from: string
  output: string
  to: string
  input: string
}

// 触发器类型
// Trigger type
export type TriggerType = 'clipboard' | 'hotkey' | 'schedule' | 'manual' | 'plugin_event'

// 工作流触发器配置
// Workflow trigger configuration
export interface ScheduleTriggerConfig {
  interval_seconds: number  // 间隔秒数
}

export interface PluginEventTriggerConfig {
  event_type: string  // 监听的事件类型
}

export interface ClipboardTriggerConfig {
  // 留空：只要剪贴板变化就触发
  pattern?: string  // 可选：内容匹配正则
}

export interface HotkeyTriggerConfig {
  shortcut: string  // 快捷键字符串，如 "CmdOrCtrl+Shift+K"
}

export type TriggerConfig =
  | ScheduleTriggerConfig
  | PluginEventTriggerConfig
  | ClipboardTriggerConfig
  | HotkeyTriggerConfig

// 触发器（数据库表行）
// Trigger (DB row)
export interface WorkflowTrigger {
  id: string
  workflow_id: string
  trigger_type: TriggerType
  config: TriggerConfig
  is_enabled: boolean
}

// 工作流（解析后，graph 已为对象）
// Workflow (parsed, graph is an object)
export interface Workflow {
  id: string
  name: string
  description: string | null
  graph: WorkflowGraph
  is_enabled: boolean
  created_at: string
  updated_at: string
}

// workflows 表原始行（graph 为 JSON 字符串）
// Raw workflows row (graph as a JSON string)
export interface WorkflowRow extends Omit<Workflow, 'graph'> {
  graph: string
}

// 执行实例状态
// Run status
export type RunStatus = 'running' | 'success' | 'failed' | 'cancelled'

// 工作流执行实例
// Workflow run
export interface WorkflowRun {
  id: number
  workflow_id: string
  trigger_type: TriggerType | null
  status: RunStatus
  error: string | null
  started_at: string
  finished_at: string | null
  steps?: WorkflowStepLog[]
}

// 步骤日志
// Step log
export interface WorkflowStepLog {
  id: number
  run_id: number
  step_id: string
  plugin_id: string | null
  action: string | null
  status: 'running' | 'success' | 'failed' | 'skipped'
  input: string | null
  output: string | null
  error: string | null
  duration_ms: number | null
  created_at: string
}

// ============================================================
// 日志 / Logs
// ============================================================

export type LogLevel = 'debug' | 'info' | 'warn' | 'error'

// 系统日志
// System log
export interface SystemLog {
  id: number
  level: LogLevel
  message: string
  source: string | null
  data: string | null
  created_at: string
}

// 插件事件
// Plugin event
export interface PluginEvent {
  id: number
  event_type: string
  source_plugin_id: string | null
  data: string | null
  created_at: string
}
