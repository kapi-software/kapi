// 工作流编辑器左侧节点面板：按插件列出可用的 workflow.actions，点击/拖入创建节点
// Left-side node palette: lists available workflow.actions per plugin,
// click or drop on the canvas to add a node
import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Plus } from "lucide-react";
import type { Plugin } from "@/types";

// 聚合插件动作：把 plugin.manifest.workflow?.actions 平铺成可拖入项
// Aggregate plugin actions: flatten plugin.manifest.workflow?.actions into drop targets
export interface PaletteItem {
  pluginId: string
  pluginName: string
  actionName: string
}

export function buildPalette(plugins: Plugin[]): PaletteItem[] {
  const items: PaletteItem[] = []
  for (const p of plugins) {
    const actions = p.manifest?.workflow?.actions ?? []
    for (const a of actions) {
      items.push({
        pluginId: p.id,
        pluginName: p.name,
        actionName: a.name,
      })
    }
  }
  return items
}

export function NodePalette({
  plugins,
  onDrop,
}: {
  plugins: Plugin[]
  onDrop: (pluginId: string, actionName: string) => void
}) {
  const { t } = useTranslation()
  const items = useMemo(() => buildPalette(plugins), [plugins])

  // 按插件分组展示
  // Group by plugin for display
  const grouped = useMemo(() => {
    const map = new Map<string, { name: string; actions: string[] }>()
    for (const it of items) {
      const g = map.get(it.pluginId) ?? { name: it.pluginName, actions: [] }
      g.actions.push(it.actionName)
      map.set(it.pluginId, g)
    }
    return Array.from(map.entries())
  }, [items])

  return (
    <div className="flex w-56 shrink-0 flex-col gap-2 rounded-xl border bg-card p-3">
      <div className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {t('workflowEditor.palette.title')}
      </div>
      <p className="text-[10px] text-muted-foreground/70">
        {t('workflowEditor.palette.dropHint')}
      </p>

      {grouped.length === 0 ? (
        <p className="mt-4 text-xs text-muted-foreground">
          {t('workflowEditor.palette.empty')}
        </p>
      ) : (
        <div className="mt-1 space-y-3 overflow-y-auto">
          {grouped.map(([pluginId, { name, actions }]) => (
            <div key={pluginId} className="space-y-1">
              <div className="truncate text-[10px] font-medium text-foreground/80">{name}</div>
              <div className="flex flex-col gap-1">
                {actions.map((actionName) => (
                  <button
                    key={actionName}
                    className="flex items-center gap-2 rounded border bg-background px-2 py-1 text-left text-xs transition-colors hover:bg-muted"
                    onClick={() => onDrop(pluginId, actionName)}
                    // 拖入画布触发：dataTransfer 携带 pluginId + actionName
                    // HTML5 drag to the canvas: dataTransfer carries pluginId + actionName
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData(
                        'application/x-kapi-node',
                        JSON.stringify({ pluginId, actionName }),
                      )
                      e.dataTransfer.effectAllowed = 'copy'
                    }}
                    title={`${name}.${actionName}`}
                  >
                    <Plus className="h-3 w-3 text-muted-foreground" />
                    <span className="font-mono">{actionName}</span>
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}