// 内置工作流模板（P6 模板市场）
// Built-in workflow templates
// 每个模板：name / description / 所需插件（前端用作可执行性检查）/ graph 模板
// Each template: name / description / required plugins (frontend pre-flight check) / graph template
import type { WorkflowGraph } from "@/types";

export interface WorkflowTemplate {
  /** 模板唯一 id / Template unique id */
  id: string
  /** 显示名（i18n key 后缀） / Display name (i18n key suffix) */
  name: string
  /** 描述 / Description */
  description: string
  /** 所需插件 manifest.workflow.actions 中的 (plugin_id, action_name) 列表
   * Required plugin actions — frontend pre-flight check before applying */
  requires: Array<{ pluginId: string; actionName: string }>
  /** 模板 graph（含节点显示名 / 边映射） / Template graph */
  graph: WorkflowGraph
  /** i18n 翻译覆盖（可选） / i18n override (optional) */
  i18n?: {
    nameKey?: string
    descriptionKey?: string
  }
}

// ============================================================
// 内置模板 / Built-in templates
// ============================================================

/** 空白模板：用户从 0 开始 / Blank template */
const BLANK_GRAPH: WorkflowGraph = {
  nodes: [],
  edges: [],
}

/** 定时整理（占位模板）：schedule 触发 → 一个占位动作节点
 * Schedule cleanup (placeholder template): schedule trigger + one placeholder action node
 * 实际插件 id/action 是占位的；用户进编辑器后填具体插件
 * Plugin ids/actions are placeholders; user picks real plugins in editor
 */
const SCHEDULE_CLEANUP_GRAPH: WorkflowGraph = {
  nodes: [
    {
      id: "n-1",
      type: "plugin",
      plugin_id: "", // 用户进编辑器后从 palette 选
      action: "",
      config: {},
      position: { x: 240, y: 160 },
      display_name: "步骤 1",
    },
  ],
  edges: [],
}

/** 剪贴板格式化（占位）：手动触发 → 一个 action 节点
 * Clipboard formatter (placeholder): manual trigger + one action node
 */
const CLIPBOARD_FORMAT_GRAPH: WorkflowGraph = {
  nodes: [
    {
      id: "n-1",
      type: "plugin",
      plugin_id: "",
      action: "",
      config: {},
      position: { x: 240, y: 160 },
      display_name: "步骤 1",
    },
  ],
  edges: [],
}

/** 快捷键唤起（占位）：hotkey 触发 + 一个 action
 * Hotkey invoke (placeholder): hotkey trigger + one action
 */
const HOTKEY_INVOKE_GRAPH: WorkflowGraph = {
  nodes: [
    {
      id: "n-1",
      type: "plugin",
      plugin_id: "",
      action: "",
      config: {},
      position: { x: 240, y: 160 },
      display_name: "步骤 1",
    },
  ],
  edges: [],
}

// ============================================================
// 模板列表 / Template list
// ============================================================

export const WORKFLOW_TEMPLATES: WorkflowTemplate[] = [
  {
    id: "blank",
    name: "空白工作流",
    description: "从空白画布开始，自由组合插件与动作",
    requires: [],
    graph: BLANK_GRAPH,
    i18n: {
      nameKey: "templates.blank.name",
      descriptionKey: "templates.blank.description",
    },
  },
  {
    id: "schedule-cleanup",
    name: "定时整理",
    description: "按计划自动执行某个动作（每天 / 每周 / 间隔）",
    requires: [],
    graph: SCHEDULE_CLEANUP_GRAPH,
    i18n: {
      nameKey: "templates.scheduleCleanup.name",
      descriptionKey: "templates.scheduleCleanup.description",
    },
  },
  {
    id: "clipboard-format",
    name: "剪贴板格式化",
    description: "捕获剪贴板内容并自动格式化（开发中：需 clipboard 触发器接线）",
    requires: [],
    graph: CLIPBOARD_FORMAT_GRAPH,
    i18n: {
      nameKey: "templates.clipboardFormat.name",
      descriptionKey: "templates.clipboardFormat.description",
    },
  },
  {
    id: "hotkey-invoke",
    name: "快捷键唤起",
    description: "按快捷键触发一个动作（开发中：需 hotkey 触发器接线）",
    requires: [],
    graph: HOTKEY_INVOKE_GRAPH,
    i18n: {
      nameKey: "templates.hotkeyInvoke.name",
      descriptionKey: "templates.hotkeyInvoke.description",
    },
  },
]

/** 按 id 查模板 / Lookup template by id */
export function getTemplate(id: string | null | undefined): WorkflowTemplate | null {
  if (!id) return null
  return WORKFLOW_TEMPLATES.find((t) => t.id === id) ?? null
}
