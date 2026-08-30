// 数据绑定 Drawer：从底部滑出的绑定编辑器
// Data bindings drawer: slides up from the bottom
import { Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerContent,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
  DrawerDescription,
} from "@/components/ui/drawer";
import { BindingsEditor } from "./BindingsEditor";
import type { DataBinding, WorkflowGraph } from "@/types";
import type { Edge, Node } from "@xyflow/react";

interface Props {
  graph: WorkflowGraph
  nodes: Node[]
  edges: Edge[]
  selectedNodeId: string | null
  onChange: (bindings: DataBinding[]) => void
}

export function BindingsDrawer({ graph, nodes, edges, selectedNodeId, onChange }: Props) {
  return (
    <Drawer>
      <DrawerTrigger asChild>
        <Button variant="outline" size="sm" className="gap-1.5">
          <Settings className="h-3.5 w-3.5" />
          数据绑定
          {graph.bindings.length > 0 && (
            <span className="ml-1 rounded bg-primary/10 px-1.5 py-0.5 text-[10px] font-medium text-primary">
              {graph.bindings.length}
            </span>
          )}
        </Button>
      </DrawerTrigger>
      <DrawerContent className="max-h-[85vh] h-[50vh]">
        <DrawerHeader>
          <DrawerTitle>数据绑定</DrawerTitle>
          <DrawerDescription>
            将上游节点的输出字段映射到下游节点的输入字段
          </DrawerDescription>
        </DrawerHeader>
        <div className="overflow-y-auto px-4 pb-6">
          <BindingsEditor
            graph={graph}
            nodes={nodes}
            edges={edges}
            selectedNodeId={selectedNodeId}
            onChange={onChange}
          />
        </div>
      </DrawerContent>
    </Drawer>
  );
}
