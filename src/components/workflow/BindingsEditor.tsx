// 数据绑定编辑器：行表 + 添加/删除；选中下游节点时聚焦
// Data bindings editor: row table + add/remove; focuses the downstream node when one is selected
import { useTranslation } from "react-i18next";
import { Plus, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { DataBinding, WorkflowGraph, WorkflowNode } from "@/types";
import type { Edge, Node } from "@xyflow/react";

interface Props {
  graph: WorkflowGraph
  nodes: Node[]
  edges: Edge[]
  selectedNodeId: string | null
  onChange: (bindings: DataBinding[]) => void
}

export function BindingsEditor({ graph, nodes, selectedNodeId, onChange }: Props) {
  const { t } = useTranslation()

  // 节点 ID 集合，便于校验
  // Node ID set for validation
  const nodeIds = new Set(nodes.map((n) => n.id))

  // 默认过滤：选中下游节点时只显示以该节点为 to 的绑定
  // Default filter: show bindings whose 'to' is the selected downstream node
  const focusNodeId = selectedNodeId
  const visibleBindings = focusNodeId
    ? graph.bindings.filter((b) => b.to === focusNodeId)
    : graph.bindings

  // 节点 → 可用 action 名（v1 简化：直接从 manifest 取）
  // Node → available action names (v1 simplification: read directly from manifest)
  const actionOf = (nodeId: string): string => {
    const node = graph.nodes.find((x: WorkflowNode) => x.id === nodeId)
    return node?.action ?? ""
  }

  // 添加新绑定（默认聚焦当前选中节点，否则放到末尾空行）
  // Add a new binding (defaulting to the currently selected node, else blank)
  const handleAdd = () => {
    const blank: DataBinding = {
      from: "",
      output: "",
      to: focusNodeId ?? "",
      input: "",
    }
    onChange([...graph.bindings, blank])
  }

  const handleUpdate = (index: number, patch: Partial<DataBinding>) => {
    const next = [...graph.bindings]
    next[index] = { ...next[index], ...patch }
    onChange(next)
  }

  const handleRemove = (index: number) => {
    onChange(graph.bindings.filter((_, i) => i !== index))
  }

  return (
    <div className="rounded-xl border bg-card p-3">
      <div className="mb-2 flex items-center justify-between">
        <div className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
          {t('workflowEditor.bindings.title')}
        </div>
        <Button variant="outline" size="sm" onClick={handleAdd}>
          <Plus className="h-3 w-3" />
          {t('workflowEditor.bindings.add')}
        </Button>
      </div>

      <p className="mb-2 text-[10px] text-muted-foreground/70">
        {focusNodeId
          ? `${t('workflowEditor.bindings.to')}: ${focusNodeId} · ${actionOf(focusNodeId)}`
          : t('workflowEditor.bindings.hint')}
      </p>

      {/* 表头 */}
      {/* Header */}
      <div className="grid grid-cols-[1fr_1fr_1fr_1fr_auto] gap-2 px-2 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        <span>{t('workflowEditor.bindings.from')}</span>
        <span>{t('workflowEditor.bindings.output')}</span>
        <span>{t('workflowEditor.bindings.to')}</span>
        <span>{t('workflowEditor.bindings.input')}</span>
        <span />
      </div>

      {/* 行 */}
      {/* Rows */}
      {visibleBindings.length === 0 ? (
        <p className="px-2 py-3 text-center text-xs text-muted-foreground">
          {focusNodeId
            ? t('workflowEditor.bindings.selectSourceFirst')
            : t('workflowEditor.bindings.empty')}
        </p>
      ) : (
        <div className="space-y-1.5">
          {visibleBindings.map((b, _i) => {
            const realIndex = graph.bindings.indexOf(b)
            const invalid = !b.from || !b.output || !b.to || !b.input
            return (
              <div
                key={`${realIndex}-${b.from}-${b.to}`}
                className="grid grid-cols-[1fr_1fr_1fr_1fr_auto] items-center gap-2"
              >
                {/* 源节点选择 */}
                {/* Source node selector */}
                <select
                  className="h-7 rounded-md border bg-background px-2 font-mono text-xs"
                  value={b.from}
                  onChange={(e) => handleUpdate(realIndex, { from: e.target.value })}
                >
                  <option value="">—</option>
                  {Array.from(nodeIds).map((id) => (
                    <option key={id} value={id}>
                      {id}
                    </option>
                  ))}
                </select>

                {/* 输出字段名（自由输入；后续可用 action.outputs 下拉） */}
                {/* Output field name (free input; later use action.outputs dropdown) */}
                <input
                  className="h-7 rounded-md border bg-background px-2 font-mono text-xs"
                  value={b.output}
                  onChange={(e) => handleUpdate(realIndex, { output: e.target.value })}
                  placeholder="output field"
                />

                {/* 目标节点选择 */}
                {/* Target node selector */}
                <select
                  className="h-7 rounded-md border bg-background px-2 font-mono text-xs"
                  value={b.to}
                  onChange={(e) => handleUpdate(realIndex, { to: e.target.value })}
                >
                  <option value="">—</option>
                  {Array.from(nodeIds).map((id) => (
                    <option key={id} value={id}>
                      {id}
                    </option>
                  ))}
                </select>

                {/* 输入字段名 */}
                <input
                  className="h-7 rounded-md border bg-background px-2 font-mono text-xs"
                  value={b.input}
                  onChange={(e) => handleUpdate(realIndex, { input: e.target.value })}
                  placeholder="input field"
                />

                {/* 删除按钮 */}
                {/* Remove button */}
                <button
                  className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
                  onClick={() => handleRemove(realIndex)}
                  aria-label={t('workflowEditor.bindings.remove')}
                  title={t('workflowEditor.bindings.remove')}
                >
                  <X className="h-3 w-3" />
                </button>

                {/* 错误提示行 */}
                {/* Validation hint */}
                {invalid && (
                  <p className="col-span-5 -mt-1 text-[10px] text-destructive">
                    {t('workflowEditor.bindings.invalid')}
                  </p>
                )}
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}