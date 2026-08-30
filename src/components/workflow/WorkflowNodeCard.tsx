// React Flow 自定义插件节点：节点 ID + 插件名 + 动作名 + 输入/输出摘要
// React Flow custom plugin node: node id + plugin name + action name + input/output summary
import { Handle, Position, type NodeProps } from "@xyflow/react";

export function WorkflowNodeCard({ data, selected }: NodeProps) {
  const pluginId = (data?.plugin_id as string) ?? "";
  const action = (data?.action as string) ?? "";

  return (
    <div
      className={`min-w-[160px] rounded-md border bg-card px-3 py-2 text-xs shadow-sm transition-colors ${
        selected ? "border-primary ring-1 ring-primary/30" : ""
      }`}
    >
      {/* 上方：入边 */}
      {/* Top: target handle */}
      <Handle
        type="target"
        position={Position.Top}
        className="!h-2 !w-2 !bg-primary"
      />

      <div className="space-y-0.5">
        <div className="font-mono text-[10px] text-muted-foreground">
          {(data?.nodeId as string) ?? ""}
        </div>
        <div className="truncate font-medium">{pluginId || "—"}</div>
        <div className="truncate font-mono text-muted-foreground">
          {action || "—"}
        </div>
      </div>

      {/* 下方：出边 */}
      {/* Bottom: source handle */}
      <Handle
        type="source"
        position={Position.Bottom}
        className="!h-2 !w-2 !bg-primary"
      />
    </div>
  );
}