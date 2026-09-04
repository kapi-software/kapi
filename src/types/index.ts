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

// ============================================================
// 工作流 manifest schema（P2：结构化字段描述）
// Workflow manifest schema (P2: structured field descriptors)
// ============================================================

/** 字段类型 / Field type */
export type FieldType = 'string' | 'number' | 'boolean' | 'enum' | 'file' | 'json'

/** enum 类型的选项 / enum option */
export interface FieldOption {
  value: string
  label: string
}

/** 单个字段描述（inputs/outputs 数组的项）
 * Single field descriptor (item in inputs/outputs arrays)
 *
 * 与提案的差异：key（字段名）放到对象 key 上（保持对象结构便于按名查）
 * Difference from proposal: key (field name) is the object's key (preserves object shape for lookup)
 */
export interface FieldSpec {
  /** 字段类型 / Field type */
  type: FieldType
  /** 人类可读标签（用于 UI 展示）/ Human-readable label (shown in UI) */
  label?: string
  /** 描述（tooltip）/ Description (tooltip) */
  description?: string
  /** 是否必填（运行时校验）/ Required (runtime validation) */
  required?: boolean
  /** 默认值 / Default value */
  default?: unknown
  /** 占位符 / Placeholder */
  placeholder?: string
  /** enum 选项（type=enum 时必填）/ enum options (required for type=enum) */
  options?: FieldOption[]
  /** number 类型的最小值 / Min (for number) */
  min?: number
  /** number 类型的最大值 / Max (for number) */
  max?: number
  /** string 类型的最小长度 / Min length (for string) */
  minLength?: number
  /** string 类型的最大长度 / Max length (for string) */
  maxLength?: number
  /** file 类型的接受类型 / Accepted types (for file) */
  accept?: string
}

// 工作流能力声明（manifest.workflow）
// Workflow capability declaration in the manifest
export interface PluginWorkflowSpec {
  triggers?: string[]
  actions?: Array<{
    name: string
    summary?: string
    // 字段 schema：Record<key, FieldSpec>
    // Field schema: Record<key, FieldSpec>
    inputs?: Record<string, FieldSpec>
    outputs?: Record<string, FieldSpec>
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
  // 边携带数据映射（upstream output → downstream input）
  // Edges carry the data map (upstream output → downstream input)
  edges: WorkflowEdge[]
}

/** 数据流边：节点 A → 节点 B，map 描述 A 的 outputs 哪些字段喂给 B 的 inputs 哪些字段
 * Data edge: node A → node B, map describes which fields of A.outputs feed which fields of B.inputs
 * 例：map: { text: "content" } 表示 A.outputs.text 喂给 B.inputs.content
 * Example: map: { text: "content" } means A.outputs.text → B.inputs.content
 */
export interface WorkflowEdge {
  from: string
  to: string
  map?: Record<string, string>
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
  position: { x: number; y: number }
  // 可编辑显示名（默认按 action.summary / "步骤 N" 推断）
  // Editable display name (defaults to action.summary or "Step N")
  display_name?: string
}

// 触发器类型
// Trigger type
export type TriggerType = 'clipboard' | 'hotkey' | 'schedule' | 'manual' | 'plugin_event'

// 工作流触发器配置
// Workflow trigger configuration
export interface ScheduleTriggerConfig {
  /** 5 字段 cron 表达式（分 时 日 月 周），支持 * , - / 特殊字符
   * 5-field cron expression (min hour dom month dow), supports * , - /
   * 例 / 每5分钟: "* /5 * * * *"  / 每天9点: "0 9 * * *"  / 每周一早8点: "0 8 * * 1" */
  cron: string
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

// 当前 graph schema 版本（与 Rust 端 CURRENT_GRAPH_VERSION 对齐）
// Current graph schema version (mirrors CURRENT_GRAPH_VERSION on the Rust side)
export const CURRENT_GRAPH_VERSION = 1

// 工作流（解析后，graph 已为对象）
// Workflow (parsed, graph is an object)
export interface Workflow {
  id: string
  name: string
  description: string | null
  graph: WorkflowGraph
  // graph schema 版本
  // graph schema version
  schema_version: number
  is_enabled: boolean
  created_at: string
  updated_at: string
}

// ============================================================
// 图校验
// Graph validation
// ============================================================

/** 校验错误严重程度 / Validation error severity */
export type GraphErrorKind = 'fatal' | 'warning'

/** 单条校验错误 / Single validation error */
export interface GraphError {
  kind: GraphErrorKind
  /** 机器可读代码 / Machine-readable code */
  code: string
  /** 人类可读消息 / Human-readable message */
  message: string
  /** 涉及到的 node id（环/孤儿场景）/ Affected node ids */
  node_ids?: string[]
  /** 涉及到的边索引 / Affected edge indices */
  edge_indices?: number[]
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
