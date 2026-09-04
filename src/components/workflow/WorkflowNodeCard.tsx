// React Flow 自定义插件节点：节点 ID + 插件名 + 动作名 + 输入/输出摘要
// React Flow custom plugin node: node id + plugin name + action name + input/output summary
import { Handle, Position, type NodeProps } from "@xyflow/react";

// 节点主标题：有 display_name 则优先显示；无则回退
// Node title: display_name > plugin_id (raw id)
function NodeTitle({ data }: { data: Record<string, unknown> }) {
  const displayName = (data?.display_name as string | undefined) || "";
  const pluginId = (data?.plugin_id as string) ?? "";
  if (displayName) {
    return <div className="truncate font-medium">{displayName}</div>;
  }
  return <div className="truncate font-medium">{pluginId || "—"}</div>;
}

export function WorkflowNodeCard({ data, selected }: NodeProps) {
  const nodeType = (data?.type as string) ?? "plugin";
  const action = (data?.action as string) ?? "";
  const isTransform = nodeType === "transform";

  return (
    <div
      className={`min-w-[160px] rounded-md border bg-card px-3 py-2 text-xs shadow-sm transition-colors ${
        selected ? "border-primary ring-1 ring-primary/30" : ""
      } ${isTransform ? "border-dashed border-blue-400 bg-blue-50 dark:bg-blue-950" : ""}`}
    >
      {/* 上方：入边 */}
      {/* Top: target handle */}
      <Handle
        type="target"
        position={Position.Top}
        className={`!h-2 !w-2 ${isTransform ? "!bg-blue-400" : "!bg-primary"}`}
      />

      <div className="space-y-0.5">
        <div className="flex items-center gap-1">
          {isTransform ? (
            <span className="rounded bg-blue-200 px-1 py-0.5 text-[9px] font-medium text-blue-700 dark:bg-blue-800 dark:text-blue-200">
              Transform
            </span>
          ) : (
            <div className="font-mono text-[10px] text-muted-foreground">
              {(data?.nodeId as string) ?? ""}
            </div>
          )}
        </div>
        <div className="truncate font-medium">
          {isTransform ? <NodeTitle data={data as Record<string, unknown>} /> : <NodeTitle data={data as Record<string, unknown>} />}
        </div>
        <div className="truncate font-mono text-muted-foreground">
          {isTransform ? "数据映射" : (action || "—")}
        </div>
      </div>

      {/* 下方：出边 */}
      {/* Bottom: source handle */}
      <Handle
        type="source"
        position={Position.Bottom}
        className={`!h-2 !w-2 ${isTransform ? "!bg-blue-400" : "!bg-primary"}`}
      />
    </div>
  );
}