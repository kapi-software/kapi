// 连接校验（连线即数据流 + 类型约束）
// Connection validation (connection-is-data-flow + type constraints)
//
// 核心规则：
// Core rules:
// 1. 上游无输出 / 下游无输入 → 允许（map 可任意）
// 2. 上游 output type 与下游 input type 不一致 → 拒绝
// 3. 至少一个字段对 type 一致才允许
// 4. transform 节点始终允许（Handlebars 模板，类型不敏感）
// 5. 自环（source === target）→ 拒绝
import type { FieldSpec, Plugin } from '@/types'

// ============================================================
// Types
// ============================================================

export type ValidationResult =
  | { ok: true; autoMap: Record<string, string> }
  | { ok: false; reason: string }

/** 简化的节点描述（与 React Flow Node 解耦） */
export interface ConnectionNode {
  id: string
  type?: 'plugin' | 'transform' | string
  plugin_id?: string
  action?: string
}

/** 类似 React Flow Connection 的输入 */
export interface ConnectionRequest {
  source: string
  target: string
  sourceHandle?: string | null
  targetHandle?: string | null
}

// ============================================================
// Helpers
// ============================================================

/** 找节点的插件定义 / Find plugin for a node */
function findPlugin(plugins: Plugin[], node: ConnectionNode | undefined): Plugin | undefined {
  if (!node?.plugin_id) return undefined
  return plugins.find((p) => p.id === node.plugin_id)
}

/** 找节点的 action 定义 / Find action for a node */
function findAction(plugin: Plugin | undefined, actionName: string | undefined) {
  if (!plugin || !actionName) return undefined
  return plugin.manifest?.workflow?.actions?.find((a) => a.name === actionName)
}

/** 提取输出字段（key 列表）/ Extract output field keys */
function getOutputKeys(action: ReturnType<typeof findAction>): string[] {
  if (!action?.outputs) return []
  return Object.keys(action.outputs)
}

/** 提取输入字段（key 列表） */
function getInputKeys(action: ReturnType<typeof findAction>): string[] {
  if (!action?.inputs) return []
  return Object.keys(action.inputs)
}

/** 检查两个 FieldSpec 的 type 是否相同 / Check if two FieldSpecs share the same type */
function typesMatch(source: FieldSpec, target: FieldSpec): boolean {
  // 简单的字符串类型比较：'string' == 'string' 即可
  // 严格模式可加 'number' <-> 'string' coercion 规则；当前保守匹配
  return source.type === target.type
}

// ============================================================
// Main validator
// ============================================================

/**
 * 验证一条候选边，返回是否允许 + 自动同名字段 map
 * Validate a candidate edge; return ok + auto-same-name map, or reason
 */
export function validateConnection(
  req: ConnectionRequest,
  sourceNode: ConnectionNode | undefined,
  targetNode: ConnectionNode | undefined,
  plugins: Plugin[],
): ValidationResult {
  // 1) 自环拒绝
  if (req.source === req.target) {
    return { ok: false, reason: '节点不能连接自身 / A node cannot connect to itself' }
  }

  // 2) transform 节点始终允许（Handlebars 模板，类型无关）
  if (targetNode?.type === 'transform' || sourceNode?.type === 'transform') {
    return { ok: true, autoMap: {} }
  }

  // 3) 找 action 上下文
  const sourcePlugin = findPlugin(plugins, sourceNode)
  const targetPlugin = findPlugin(plugins, targetNode)
  const sourceAction = findAction(sourcePlugin, sourceNode?.action)
  const targetAction = findAction(targetPlugin, targetNode?.action)

  const sourceOutputs = getOutputKeys(sourceAction)
  const targetInputs = getInputKeys(targetAction)

  // 4) 任一端未声明字段 schema → 放行（manifest 可不声明 inputs/outputs）
  if (sourceOutputs.length === 0 || targetInputs.length === 0) {
    return { ok: true, autoMap: {} }
  }

  // 5) 两端都有 schema：同名字段 + 类型匹配生成 autoMap
  const sourceFields = sourceAction!.outputs!
  const targetFields = targetAction!.inputs!

  const autoMap: Record<string, string> = {}
  const typeMismatchFields: string[] = []
  for (const outKey of sourceOutputs) {
    if (!targetInputs.includes(outKey)) continue
    const sourceSpec = sourceFields[outKey]
    const targetSpec = targetFields[outKey]
    if (!sourceSpec || !targetSpec) continue
    if (typesMatch(sourceSpec, targetSpec)) {
      autoMap[outKey] = outKey
    } else {
      // 同名但类型不匹配：记录供提示
      typeMismatchFields.push(`${outKey} (${sourceSpec.type} → ${targetSpec.type})`)
    }
  }

  // 若任何同名输出/输入都类型不匹配 → 拒绝
  // 若至少一个匹配 + 其余不匹配 → 允许但提示用户
  if (Object.keys(autoMap).length === 0) {
    // 没有任何匹配
    if (typeMismatchFields.length > 0) {
      return {
        ok: false,
        reason: `字段类型不匹配：${typeMismatchFields.join(', ')}`,
      }
    }
    // 没有任何同名输出/输入
    return {
      ok: true,
      autoMap: {},
    }
  }

  // 至少有一个匹配；如有类型不匹配，作为 warning 透传（暂只决定 ok）
  return { ok: true, autoMap }
}
