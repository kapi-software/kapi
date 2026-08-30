// 工作流可视化编辑器页（/workflow/new 与 /workflow/:id/edit）
// Workflow visual editor page (/workflow/new and /workflow/:id/edit)
import { useCallback, useEffect, useMemo, useState, type Dispatch, type SetStateAction } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate, useParams, useSearchParams } from "react-router-dom";
import {
  ReactFlow,
  addEdge,
  useNodesState,
  useEdgesState,
  type Connection,
  type Node,
  type Edge,
  Background,
  Controls,
  MiniMap,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Save, Play, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import { usePluginsStore } from "@/stores/plugins";
import { isTauri } from "@/lib/tauri";
import { shortId } from "@/lib/id";
import type { Workflow, WorkflowGraph, WorkflowNode } from "@/types";
import { WorkflowNodeCard } from "@/components/workflow/WorkflowNodeCard";
import { NodePalette } from "@/components/workflow/NodePalette";
import { BindingsEditor } from "@/components/workflow/BindingsEditor";

// 默认节点类型注册（用于 React Flow 自定义节点）
// Default node type registry (for React Flow custom nodes)
const nodeTypes = { plugin: WorkflowNodeCard };

// 空 graph：起点
// Empty graph: starting point for new workflows
const EMPTY_GRAPH: WorkflowGraph = {
  nodes: [],
  edges: [],
  bindings: [],
};

// React Flow Node → WorkflowNode
function rfNodeToGraphNode(n: Node): WorkflowNode {
  return {
    id: n.id,
    type: (n.data?.type as WorkflowNode["type"]) ?? "plugin",
    plugin_id: n.data?.plugin_id as string ?? "",
    action: n.data?.action as string ?? "",
    config: n.data?.config as Record<string, unknown> ?? {},
  };
}

// WorkflowNode → React Flow Node（仅追加 data，不改 position）
// WorkflowNode → React Flow Node (appends data, doesn't move position)
function graphNodeToRfNode(n: WorkflowNode, pos?: { x: number; y: number }): Node {
  return {
    id: n.id,
    type: n.type,
    position: pos ?? { x: 0, y: 0 },
    data: {
      type: n.type,
      plugin_id: n.plugin_id ?? "",
      action: n.action ?? "",
      config: n.config ?? {},
    },
  };
}

// 从已有 graph 初始化 React Flow nodes/edges
// Initialize React Flow nodes/edges from an existing graph
function graphToRfState(graph: WorkflowGraph): { initNodes: Node[]; initEdges: Edge[] } {
  // 用网格布局给节点分配位置（简单行布局）
  // Assign positions using a simple grid layout
  const COLS = 3;
  const initNodes: Node[] = graph.nodes.map((n, i) =>
    graphNodeToRfNode(n, {
      x: (i % COLS) * 280,
      y: Math.floor(i / COLS) * 120,
    }),
  );
  const initEdges: Edge[] = graph.edges.map((e, i) => ({
    id: `e-${i}`,
    source: e.from,
    target: e.to,
  }));
  return { initNodes, initEdges };
}

// 生成唯一节点 id（uuid 短码，去重友好）
// Generate a unique node id (short UUID, friendly to dedupe)
function nextNodeId(): string {
  return `n-${shortId()}`;
}

export default function WorkflowEditor() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { id } = useParams<{ id: string }>();
  // 编辑器路由始终携带 id（NewWorkflowDialog 已生成）。新建与编辑共用此组件，
  // 区别仅在 store 里能否找到该 id（找不到即为新建，预填 name/description）。
  // The editor route always carries an id (NewWorkflowDialog generates it).
  // "New" vs "edit" is decided by whether the id is already in the store.
  const [searchParams] = useSearchParams();
  const prefilledName = searchParams.get("name") ?? "";
  const prefilledDescription = searchParams.get("description") ?? "";

  const { workflows, load, save, run } = useWorkflowsStore();
  const { plugins } = usePluginsStore();

  // 新建/编辑判断：id 在 store 中找不到即为新建（保存前）
  // isNew: id not yet in the store (before first save)
  const existingWf = workflows.find((w) => w.id === id);
  const isNew = !existingWf;

  // 表单状态
  // Form state
  const [name, setName] = useState(prefilledName);
  const [description, setDescription] = useState(prefilledDescription);
  const [graph, setGraph] = useState<WorkflowGraph>(EMPTY_GRAPH);
  const [loadingWf, setLoadingWf] = useState(false);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);

  // React Flow 状态
  // React Flow state
  const { initNodes, initEdges } = useMemo(() => graphToRfState(graph), []);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>(initNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>(initEdges);

  // 显式标注 setNodes / setEdges 回调类型（避免 useMemo 推断失效 → any）
  // Explicitly annotate setNodes / setEdges callback types
  const setNodesTyped = setNodes as Dispatch<SetStateAction<Node[]>>;
  const setEdgesTyped = setEdges as Dispatch<SetStateAction<Edge[]>>;

  // 选中节点（用于右侧属性面板）
  // Selected node (for the right-side inspector)
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const selectedNode = useMemo(
    () => nodes.find((n) => n.id === selectedNodeId) ?? null,
    [nodes, selectedNodeId],
  );

  // 加载工作流：store 里能找到 → 编辑模式；找不到 → 新建（用 URL 预填值）
  // Load workflow: found in the store → edit mode; otherwise → new (pre-fill from URL)
  useEffect(() => {
    setLoadingWf(true);
    if (workflows.length === 0) {
      load().then(() => setLoadingWf(false));
      return;
    }
    const wf = workflows.find((w) => w.id === id) ?? null;
    if (wf) {
      setName(wf.name);
      setDescription(wf.description ?? "");
      setGraph(wf.graph);
    }
    // 找不到 wf 视为新建：name/description 已由 URL 预填，graph 保持空
    // Not found ⇒ new: name/description already pre-filled from the URL; graph stays empty
    setLoadingWf(false);
  }, [id, workflows, load]);

  // React Flow nodes/edges 同步回 graph
  // Sync React Flow nodes/edges back into the graph
  useEffect(() => {
    const gNodes = nodes.map((n) => rfNodeToGraphNode(n));
    const gEdges = edges.map((e) => ({ from: e.source, to: e.target }));
    setGraph((prev) => ({ ...prev, nodes: gNodes, edges: gEdges }));
  }, [nodes, edges]);

  // 从左侧面板拖入新节点
  // Drop a new node from the left palette
  const onDrop = useCallback(
    (pluginId: string, actionName: string) => {
      const newNode: Node = {
        id: nextNodeId(),
        type: "plugin",
        position: { x: 250 + Math.random() * 100, y: 200 + Math.random() * 60 },
        data: { type: "plugin", plugin_id: pluginId, action: actionName, config: {} },
      };
      setNodesTyped((ns: Node[]) => [...ns, newNode]);
      setSelectedNodeId(newNode.id);
    },
    [setNodesTyped],
  );

  // 在画布上连线（React Flow 内置行为）
  // Connect nodes on the canvas (React Flow built-in behavior)
  const onConnect = useCallback(
    (params: Connection) => {
      setEdgesTyped((es: Edge[]) => addEdge({ ...params, type: "default" }, es));
    },
    [setEdgesTyped],
  );

  // 删除选中节点
  // Delete the selected node
  const deleteSelectedNode = useCallback(() => {
    if (!selectedNodeId) return;
    setNodesTyped((ns: Node[]) => ns.filter((n: Node) => n.id !== selectedNodeId));
    setEdgesTyped((es: Edge[]) => es.filter((e: Edge) => e.source !== selectedNodeId && e.target !== selectedNodeId));
    setSelectedNodeId(null);
  }, [selectedNodeId, setNodesTyped, setEdgesTyped]);

  // 更新选中节点属性（右侧面板编辑）
  // Update selected node properties (right panel edit)
  const updateSelectedNode = useCallback(
    (patch: Partial<Node["data"]>) => {
      if (!selectedNodeId) return;
      setNodesTyped((ns: Node[]) =>
        ns.map((n: Node) =>
          n.id === selectedNodeId ? { ...n, data: { ...n.data, ...patch } } : n,
        ),
      );
    },
    [selectedNodeId, setNodesTyped],
  );

  // 保存
  // Save
  const handleSave = async () => {
    if (!name.trim()) {
      toast.error(t("workflowEditor.inspector.name") + " — " + t("workflow.dialog.name"));
      return;
    }
    setSaving(true);
    try {
      const wf: Workflow = {
        id: isNew ? `wf-${shortId()}` : (id ?? ""),
        name: name.trim(),
        description: description.trim() || null,
        graph,
        is_enabled: true,
        created_at: "",
        updated_at: "",
      };
      await save(wf);
      toast.success(t("workflowEditor.saved", { name: wf.name }));
      navigate("/workflow");
    } catch (e) {
      toast.error(t("workflowEditor.saveFailed", { error: String(e) }));
    } finally {
      setSaving(false);
    }
  };

  // 运行
  // Run
  const handleRun = async () => {
    if (!name.trim()) {
      toast.error("请先填写名称");
      return;
    }
    setRunning(true);
    try {
      // 先保存
      const wf: Workflow = {
        id: isNew ? `wf-${shortId()}` : (id ?? ""),
        name: name.trim(),
        description: description.trim() || null,
        graph,
        is_enabled: true,
        created_at: "",
        updated_at: "",
      };
      await save(wf);
      const runRow = await run(wf.id);
      if (runRow.status === "success") {
        toast.success(t("workflowEditor.runSuccess"));
      } else {
        toast.error(t("workflowEditor.runFailed", { error: runRow.error ?? "unknown" }));
      }
    } catch (e) {
      toast.error(t("workflowEditor.runFailed", { error: String(e) }));
    } finally {
      setRunning(false);
    }
  };

  if (loadingWf) {
    return (
      <div className="flex h-64 items-center justify-center">
        <span className="text-muted-foreground">{t("workflow.loading")}</span>
      </div>
    );
  }

  if (!isNew && !existingWf) {
    return (
      <div className="rounded-xl border border-dashed p-10 text-center">
        <p className="font-medium text-destructive">{t("workflowEditor.notFound")}</p>
        <Button className="mt-4" variant="outline" onClick={() => navigate("/workflow")}>
          <Pencil className="size-3.5" />
          {t("workflowEditor.back")}
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-2">
      {/* 顶部栏：名称 + 描述 + 保存/运行（返回由 TopBar 统一管理） */}
      {/* Top bar: name + description + save/run (back lives in TopBar) */}
      <div className="flex flex-wrap items-center gap-2">
        {/* 横向表单：label + input 一行 / Inline form: label + input on one line */}
        <div className="flex items-center gap-2">
          <Label className="shrink-0 text-xs text-muted-foreground">
            {t("workflowEditor.name")}
          </Label>
          <Input
            className="h-7 w-56 text-xs"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={t("workflowEditor.namePlaceholder")}
            maxLength={20}
          />
        </div>
        <div className="flex items-center gap-2">
          <Label className="shrink-0 text-xs text-muted-foreground">
            {t("workflowEditor.description")}
          </Label>
          <Input
            className="h-7 w-72 text-xs"
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            placeholder={t("workflowEditor.descriptionPlaceholder")}
          />
        </div>
        <div className="ml-auto flex items-center gap-2">
          <Button variant="outline" onClick={handleSave} disabled={saving}>
            <Save />
            {saving ? t("workflow.saving") : t("workflowEditor.save")}
          </Button>
          {isTauri() && (
            <Button variant="default" onClick={handleRun} disabled={running}>
              <Play />
              {running ? t("workflowEditor.running") : t("workflowEditor.run")}
            </Button>
          )}
        </div>
      </div>

      {/* 中部：三栏布局 */}
      {/* Middle: three-column layout (flex-1 + min-h-0 so it fills available space) */}
      <div className="flex min-h-0 flex-1 gap-3">
        {/* 左侧：节点面板 */}
        {/* Left: node palette */}
        <NodePalette plugins={plugins} onDrop={onDrop} />

        {/* 中间：React Flow 画布 */}
        {/* Center: React Flow canvas */}
        <div className="relative min-w-0 flex-1 rounded-xl border bg-background">
          {nodes.length === 0 ? (
            <div className="absolute inset-0 flex items-center justify-center text-sm text-muted-foreground">
              {t("workflowEditor.canvas.empty")}
            </div>
          ) : null}
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={(_e: unknown, n: Node) => setSelectedNodeId(n.id)}
            onPaneClick={() => setSelectedNodeId(null)}
            nodeTypes={nodeTypes}
            fitView
            fitViewOptions={{ padding: 0.3 }}
          >
            <Background />
            <Controls />
            <MiniMap />
          </ReactFlow>
        </div>

        {/* 右侧：节点属性面板 */}
        {/* Right: node inspector panel */}
        <NodeInspector
          node={selectedNode}
          plugins={plugins}
          onUpdate={updateSelectedNode}
          onDelete={deleteSelectedNode}
        />
      </div>

      {/* 底部：数据绑定编辑器（固定高度，不参与 flex-1 拉伸） */}
      {/* Bottom: data bindings editor (fixed height, no flex-1 stretch) */}
      <div className="shrink-0">
        <BindingsEditor
          graph={graph}
          nodes={nodes}
          edges={edges}
          selectedNodeId={selectedNodeId}
          onChange={(bindings) => setGraph((prev) => ({ ...prev, bindings }))}
        />
      </div>
    </div>
  );
}

// ============================================================
// 右侧节点属性面板
// Right-side node inspector panel
// ============================================================
function NodeInspector({
  node,
  plugins,
  onUpdate,
  onDelete,
}: {
  node: Node | null;
  plugins: import("@/types").Plugin[];
  onUpdate: (patch: Partial<Node["data"]>) => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const [configText, setConfigText] = useState("");
  const [configError, setConfigError] = useState<string | null>(null);

  // 节点切换时同步 config 文本
  // Sync config text when the node changes
  useEffect(() => {
    if (!node) return;
    setConfigText(
      Object.keys(node.data.config ?? {}).length > 0
        ? JSON.stringify(node.data.config, null, 2)
        : "",
    );
    setConfigError(null);
  }, [node?.id]);

  const handleConfigBlur = () => {
    if (!configText.trim()) {
      onUpdate({ config: {} });
      setConfigError(null);
      return;
    }
    try {
      const parsed = JSON.parse(configText);
      if (typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("must be object");
      onUpdate({ config: parsed });
      setConfigError(null);
    } catch (e) {
      setConfigError(t("workflowEditor.inspector.configInvalid", { error: String(e) }));
    }
  };

  // 当前节点的插件定义
  // The plugin manifest for the current node
  const plugin = plugins.find((p) => p.id === node?.data?.plugin_id);
  const actions = plugin?.manifest?.workflow?.actions ?? [];
  const selectedAction = actions.find((a: { name: string }) => a.name === node?.data?.action);

  return (
    <div className="flex w-56 flex-col gap-2 rounded-xl border bg-card p-3">
      <div className="text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
        {t("workflowEditor.inspector.title")}
      </div>

      {!node ? (
        <p className="text-xs text-muted-foreground">{t("workflowEditor.inspector.empty")}</p>
      ) : (
        <div className="space-y-2">
          {/* 节点 ID（只读） */}
          {/* Node ID (read-only) */}
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">
              {t("workflowEditor.inspector.nodeId")}
            </Label>
            <Input className="h-7 font-mono text-xs" value={node.id} readOnly />
          </div>

          {/* 插件选择 */}
          {/* Plugin selector */}
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">
              {t("workflowEditor.inspector.plugin")}
            </Label>
            <select
              className="h-8 w-full rounded-md border bg-background px-2 text-xs"
              value={node.data.plugin_id as string}
              onChange={(e) => onUpdate({ plugin_id: e.target.value, action: "" })}
            >
              <option value="">{t("workflowEditor.inspector.selectPlugin")}</option>
              {plugins.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>

          {/* 动作选择 */}
          {/* Action selector */}
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">
              {t("workflowEditor.inspector.action")}
            </Label>
            <select
              className="h-8 w-full rounded-md border bg-background px-2 text-xs"
              value={node.data.action as string}
              onChange={(e) => onUpdate({ action: e.target.value })}
              disabled={!node.data.plugin_id}
            >
              <option value="">{t("workflowEditor.inspector.selectAction")}</option>
              {actions.map((a: { name: string }) => (
                <option key={a.name} value={a.name}>
                  {a.name}
                </option>
              ))}
            </select>
          </div>

          {/* 输入/输出声明 */}
          {/* Input/output declarations */}
          {selectedAction && (
            <div className="space-y-1 rounded bg-muted/50 p-2 text-[10px]">
              {selectedAction.inputs && Object.keys(selectedAction.inputs).length > 0 && (
                <div>
                  <span className="text-muted-foreground">inputs: </span>
                  <span className="font-mono">{Object.keys(selectedAction.inputs).join(", ")}</span>
                </div>
              )}
              {selectedAction.outputs && Object.keys(selectedAction.outputs).length > 0 && (
                <div>
                  <span className="text-muted-foreground">outputs: </span>
                  <span className="font-mono">{Object.keys(selectedAction.outputs).join(", ")}</span>
                </div>
              )}
              {!selectedAction.inputs && !selectedAction.outputs && (
                <span className="italic text-muted-foreground">{t("workflowEditor.canvas.noOutputs")}</span>
              )}
            </div>
          )}

          {/* 节点配置 JSON */}
          {/* Node config JSON */}
          <div className="space-y-0.5">
            <Label className="text-[10px] text-muted-foreground">
              {t("workflowEditor.inspector.config")}
            </Label>
            <textarea
              className="h-20 w-full rounded-md border bg-background px-2 py-1 font-mono text-[10px]"
              value={configText}
              onChange={(e) => setConfigText(e.target.value)}
              onBlur={handleConfigBlur}
              placeholder={t("workflowEditor.inspector.configPlaceholder")}
              spellCheck={false}
            />
            {configError && (
              <p className="text-[10px] text-destructive">{configError}</p>
            )}
          </div>

          {/* 删除按钮 */}
          {/* Delete button */}
          <Button
            variant="outline"
            size="sm"
            className="w-full text-destructive hover:bg-destructive/10"
            onClick={onDelete}
          >
            <Pencil className="h-3 w-3" />
            {t("workflowEditor.inspector.delete")}
          </Button>
        </div>
      )}
    </div>
  );
}