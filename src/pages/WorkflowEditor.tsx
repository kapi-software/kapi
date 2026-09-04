// 工作流可视化编辑器页（/workflow/new 与 /workflow/:id/edit）
// Workflow visual editor page (/workflow/new and /workflow/:id/edit)
import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
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
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { Save, Play, Pencil, AlertTriangle, AlertCircle } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { toast } from "sonner";
import { useWorkflowsStore } from "@/stores/workflows";
import { usePluginsStore } from "@/stores/plugins";
import { isTauri } from "@/lib/tauri";
import { shortId } from "@/lib/id";
import { validateGraph, hasFatalErrors } from "@/lib/workflow-graph";
import { validateConnection } from "@/lib/workflow-connection";
import { isStructuredField, type Workflow, type WorkflowEdge, type WorkflowGraph, type WorkflowNode, type GraphError } from "@/types";
import { WorkflowNodeCard } from "@/components/workflow/WorkflowNodeCard";
import { NodePalette } from "@/components/workflow/NodePalette";
import { ActionConfigForm } from "@/components/workflow/ActionConfigForm";

// 默认节点类型注册（用于 React Flow 自定义节点）
// Default node type registry (for React Flow custom nodes)
const nodeTypes = { plugin: WorkflowNodeCard, transform: WorkflowNodeCard };

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
    // 保留 position：拖动节点会修改 n.position，需要落库
    // Keep position: dragging modifies n.position, must be persisted
    position: { x: n.position.x, y: n.position.y },
    // P5: 透传 display_name
    // P5: pass through display_name
    display_name: (n.data?.display_name as string | undefined) || undefined,
  };
}

// 检测 manifest action 是否有结构化 schema（v2）—— 至少一个 input/output 含 type 字段
// Detect whether a manifest action has structured schema (v2) — at least one input/output has a `type` field
function isStructuredAction(action: { inputs?: Record<string, unknown>; outputs?: Record<string, unknown> }): boolean {
  const hasStructured = (map: Record<string, unknown> | undefined) =>
    !!map && Object.values(map).some((v) => isStructuredField(v))
  return hasStructured(action.inputs) || hasStructured(action.outputs)
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
      // P5: 透传 display_name
      // P5: pass through display_name
      display_name: n.display_name ?? "",
    },
  };
}

// 从已有 graph 初始化 React Flow nodes/edges
// Initialize React Flow nodes/edges from an existing graph
// 优先用存的 position（v2 起节点位置落库），没有才网格排
// Prefer persisted position (since v2, positions are persisted); fall back to grid layout
function graphToRfState(graph: WorkflowGraph): { initNodes: Node[]; initEdges: Edge[] } {
  // 用网格布局给节点分配位置（仅在无 position 时使用）
  // Assign positions using a simple grid layout (only when no position is stored)
  const COLS = 3;
  const initNodes: Node[] = graph.nodes.map((n, i) =>
    graphNodeToRfNode(n, {
      x: n.position?.x ?? (i % COLS) * 280,
      y: n.position?.y ?? Math.floor(i / COLS) * 120,
    }),
  );
  // P1：加载已保存工作流时反序列化 edge.map
  // P1: deserialize edge.map when loading saved workflow
  const initEdges: Edge[] = graph.edges.map((e, i) => ({
    id: `e-${i}`,
    source: e.from,
    target: e.to,
    data: e.map ? { map: e.map } : undefined,
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
  // P6：从模板 URL 解析预填 graph（来自 NewWorkflowDialog 向导）
  // P6: parse prefilled graph from template URL (set by NewWorkflowDialog wizard)
  const prefilledGraph = useMemo<WorkflowGraph | null>(() => {
    const raw = searchParams.get("graph");
    if (!raw) return null;
    try {
      return JSON.parse(raw) as WorkflowGraph;
    } catch {
      return null;
    }
  }, [searchParams]);

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
  // P6：初始 graph 来自模板（或空白）；保存后 store 的 graph 接管
  // P6: initial graph comes from template (or blank); store takes over after save
  const [graph, setGraph] = useState<WorkflowGraph>(prefilledGraph ?? EMPTY_GRAPH);
  const [loadingWf, setLoadingWf] = useState(false);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);

  // 实时校验：graph 每次变化重算错误列表
  // Live validation: recompute errors on every graph change
  const validationErrors = useMemo<GraphError[]>(() => validateGraph(graph), [graph]);
  const hasFatal = hasFatalErrors(validationErrors);

  // React Flow 状态
  // React Flow state
  // 注意：useMemo 不能用空依赖 []，否则打开已保存工作流时 initNodes 永远是空图（mount 时的 graph 几乎都是 EMPTY_GRAPH）。
  // 这里以 graph 引用作为依赖：graph 一旦被 useEffect 146 行从 store 加载进 setGraph，graphToRfState 立即重算。
  // NOTE: useMemo cannot use [] dep — otherwise initNodes stays the empty graph captured on mount.
  // graph is the dep: once the loader effect (line ~146) calls setGraph(wf.graph), this recomputes.
  const { initNodes, initEdges } = useMemo(() => graphToRfState(graph), [graph]);
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>(initNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>(initEdges);

  // 同步 store graph → React Flow（load 完成后用最新 graph 全量替换）
  // Sync store graph → React Flow (full replace after the loader effect sets graph)
  // 之前是单向 nodes→graph，导致打开已保存工作流时画布空白 + 编辑后保存覆盖原图。
  // Previously one-way nodes→graph; opening a saved workflow left the canvas blank and saving overwrote the original.
  // P-∞：用 ref 记录"上一帧的 graph 引用"，只有外部 graph 变化才同步到 React Flow，
  // 防止 setNodes → useEffect([nodes]) → setGraph → useEffect([graph]) → setNodes 死循环
  // P-∞: use a ref to track the last graph reference; only sync to React Flow when
  // graph changes from the outside, breaking the potential setNodes → nodes→graph → graph → setNodes cycle
  const lastGraphRef = useRef<WorkflowGraph | null>(null)
  useEffect(() => {
    if (lastGraphRef.current === graph) return
    lastGraphRef.current = graph
    setNodes(initNodes)
    setEdges(initEdges)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [graph])

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
      // 新 graph 引用，触发 useMemo([graph]) 重算 + graph→nodes effect 全量替换
      // New graph reference triggers useMemo([graph]) recompute + graph→nodes full replace
      setName(wf.name);
      setDescription(wf.description ?? "");
      setGraph({ ...wf.graph, nodes: [...wf.graph.nodes], edges: [...wf.graph.edges], bindings: [...wf.graph.bindings] });
    }
    // 找不到 wf 视为新建：name/description 已由 URL 预填，graph 保持空
    // Not found ⇒ new: name/description already pre-filled from the URL; graph stays empty
    setLoadingWf(false);
  }, [id, load]);

  // React Flow nodes/edges 同步回 graph
  // Sync React Flow nodes/edges back into the graph
  // P-∞：用 ref 比较上一次写入的 nodes/edges 引用，避免 React Flow 内部 store
  // 触发的"假 setNodes"再次写入 graph，形成 feedback loop
  // P-∞: use refs to compare last written nodes/edges references; break the
  // feedback cycle where React Flow's internal store fires a "fake" setNodes
  // that re-triggers setGraph
  const lastNodesRef = useRef<Node[]>([])
  const lastEdgesRef = useRef<Edge[]>([])
  useEffect(() => {
    // 跳过初次 mount（initNodes/initEdges 已是 graph 的真值）
    // Skip first mount: initNodes/initEdges are already derived from graph
    if (lastNodesRef.current.length === 0 && lastEdgesRef.current.length === 0
        && graph.nodes.length === 0 && graph.edges.length === 0) {
      lastNodesRef.current = nodes
      lastEdgesRef.current = edges
      return
    }
    // 引用相等 → React Flow 内部没动；跳过
    // References equal -> no React Flow change; skip
    if (lastNodesRef.current === nodes && lastEdgesRef.current === edges) {
      return
    }
    lastNodesRef.current = nodes
    lastEdgesRef.current = edges
    const gNodes = nodes.map((n) => rfNodeToGraphNode(n));
    // P1：序列化 edge.data.map —— 用户连线时按 manifest 自动填的默认映射
    // P1: serialize edge.data.map — defaults filled in on connect per manifest
    const gEdges: WorkflowEdge[] = edges.map((e) => ({
      from: e.source,
      to: e.target,
      map: (e.data as { map?: Record<string, string> } | undefined)?.map,
    }));
    setGraph((prev) => {
      // 引用相等则不更新（避免循环）
      // Skip if references are equal (avoid loop)
      if (
        prev.nodes.length === gNodes.length &&
        prev.edges.length === gEdges.length &&
        prev.nodes.every((n, i) => n === gNodes[i]) &&
        prev.edges.every((e, i) => e === gEdges[i])
      ) {
        return prev;
      }
      return { ...prev, nodes: gNodes, edges: gEdges };
    });
  }, [nodes, edges, graph.nodes.length, graph.edges.length]);

  // 从左侧面板拖入新节点
  // Drop a new node from the left palette
  const onDrop = useCallback(
    (pluginId: string, actionName: string) => {
      // P5: 默认 display_name 推断
      // P5: default display_name inference
      // 优先 action.summary，否则用 "步骤 N" 形式（按当前 graph 节点数 + 1）
      // Prefer action.summary, else "Step N" (current node count + 1)
      const plugin = plugins.find((p) => p.id === pluginId)
      const action = plugin?.manifest?.workflow?.actions?.find(
        (a) => a.name === actionName,
      )
      const summary = action?.summary?.trim()
      const stepNumber = graph.nodes.length + 1
      const defaultName = summary || `步骤 ${stepNumber}`

      const newNode: Node = {
        id: nextNodeId(),
        type: "plugin",
        position: { x: 250 + Math.random() * 100, y: 200 + Math.random() * 60 },
        data: {
          type: "plugin",
          plugin_id: pluginId,
          action: actionName,
          config: {},
          display_name: defaultName,
        },
      };
      setNodesTyped((ns: Node[]) => [...ns, newNode]);
      setSelectedNodeId(newNode.id);
    },
    [setNodesTyped, plugins, graph.nodes.length],
  );

  // 添加 Transform 节点
  // Add a Transform node
  const onDropTransform = useCallback(() => {
    const stepNumber = graph.nodes.length + 1;
    const newNode: Node = {
      id: nextNodeId(),
      type: "transform",
      position: { x: 250 + Math.random() * 100, y: 200 + Math.random() * 60 },
      data: {
        type: "transform",
        plugin_id: "",
        action: "",
        config: { template: '{\n  "output": "{{input}}"}\n' },
        display_name: `步骤 ${stepNumber}`,
      },
    };
    setNodesTyped((ns: Node[]) => [...ns, newNode]);
    setSelectedNodeId(newNode.id);
  }, [setNodesTyped, graph.nodes.length]);

  // 在画布上连线（React Flow 内置行为）
  // Connect nodes on the canvas (React Flow built-in behavior)
  // P1：连线时按 manifest outputs/inputs 自动生成 edge.map（同名字段自动映射）
  // P1: on connect, auto-generate edge.map per manifest outputs/inputs (same-name auto-map)
  // P1-2：连接前用 validateConnection 验证（同名输出/输入类型不匹配则拒绝）
  // P1-2: validate connection before accepting (reject on type mismatch)
  const findNode = useCallback(
    (id: string) => nodes.find((n) => n.id === id),
    [nodes],
  )

  // 实时校验：拖线过程中显示拒绝
  // Live validation: react when dragging an edge
  const isValidConnection = useCallback(
    (params: Connection | { source: string; target: string; sourceHandle?: string | null; targetHandle?: string | null }) => {
      if (!params.source || !params.target) return false
      const sourceNode = findNode(params.source)
      const targetNode = findNode(params.target)
      const r = validateConnection(
        { source: params.source, target: params.target },
        sourceNode as { id: string; type?: string; plugin_id?: string; action?: string } | undefined,
        targetNode as { id: string; type?: string; plugin_id?: string; action?: string } | undefined,
        plugins,
      )
      return r.ok
    },
    [findNode, plugins],
  )

  const onConnect = useCallback(
    (params: Connection) => {
      const sourceNode = findNode(params.source ?? "")
      const targetNode = findNode(params.target ?? "")
      const r = validateConnection(
        { source: params.source ?? "", target: params.target ?? "" },
        sourceNode as { id: string; type?: string; plugin_id?: string; action?: string } | undefined,
        targetNode as { id: string; type?: string; plugin_id?: string; action?: string } | undefined,
        plugins,
      )
      if (!r.ok) {
        // 兜底提示（isValidConnection 已阻止；这里是键盘触发的兜底）
        // Fallback toast (isValidConnection blocks mouse drag; this catches keyboard)
        toast.error(r.reason)
        return
      }

      setEdgesTyped((es: Edge[]) =>
        addEdge(
          {
            ...params,
            type: "default",
            // 注入 edge.data.map，P1 核心：数据路由信息附着在边上
            // Inject edge.data.map — P1 core: data routing info lives on the edge
            data: Object.keys(r.autoMap).length > 0 ? { map: r.autoMap } : undefined,
          },
          es,
        ),
      )
    },
    [setEdgesTyped, plugins, findNode],
  );

  // 删除选中节点
  // Delete the selected node
  const deleteSelectedNode = useCallback(() => {
    if (!selectedNodeId) return;
    setNodesTyped((ns: Node[]) => ns.filter((n: Node) => n.id !== selectedNodeId));
    // P1：删边时自动级联删除（edges 携带数据，无需单独清理 bindings）
    // P1: deleting edges auto-cascades data (edges carry their data; no bindings to clean)
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
    // 前端校验闸（致命错误直接拒绝）；后端会再次校验
    // Frontend validation gate (fatal errors reject); backend re-validates
    if (hasFatal) {
      toast.error(t("workflowEditor.validation.fatalTitle"));
      return;
    }
    setSaving(true);
    try {
      const wf: Workflow = {
        id: isNew ? `wf-${shortId()}` : (id ?? ""),
        name: name.trim(),
        description: description.trim() || null,
        graph,
        schema_version: 1,
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
    if (hasFatal) {
      toast.error(t("workflowEditor.validation.fatalTitle"));
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
        schema_version: 1,
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
          <Button variant="outline" onClick={handleSave} disabled={hasFatal || saving}>
            <Save />
            {saving ? t("workflow.saving") : t("workflowEditor.save")}
          </Button>
          {isTauri() && (
            <Button variant="default" onClick={handleRun} disabled={hasFatal || running}>
              <Play />
              {running ? t("workflowEditor.running") : t("workflowEditor.run")}
            </Button>
          )}
        </div>
      </div>

      {/* 图校验错误提示 */}
      {/* Graph validation error bar */}
      {validationErrors.length > 0 && (
        <div
          className={`rounded border px-3 py-1.5 text-xs ${
            hasFatal
              ? "border-destructive/50 bg-destructive/10 text-destructive"
              : "border-yellow-500/50 bg-yellow-500/10 text-yellow-700 dark:text-yellow-400"
          }`}
        >
          <div className="flex items-center gap-1.5 font-medium">
            {hasFatal ? <AlertCircle className="size-3.5 shrink-0" /> : <AlertTriangle className="size-3.5 shrink-0" />}
            {hasFatal ? t("workflowEditor.validation.fatalTitle") : t("workflowEditor.validation.warningTitle")}
          </div>
          <ul className="mt-0.5 space-y-0.5 pl-4">
            {validationErrors.map((e, i) => (
              <li key={i}>{e.message}</li>
            ))}
          </ul>
        </div>
      )}

      {/* 中部：三栏布局 */}
      {/* Middle: three-column layout (flex-1 + min-h-0 so it fills available space) */}
      <div className="flex min-h-0 flex-1 gap-3">
        {/* 左侧：节点面板 */}
        {/* Left: node palette */}
        <NodePalette plugins={plugins} onDrop={onDrop} onDropTransform={onDropTransform} />

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
            // P1-2：拖线过程中拒绝非法连接（自环 + 字段类型不匹配）
            // P1-2: reject invalid connections during drag (self-loop + type mismatch)
            isValidConnection={isValidConnection}
            onNodeClick={(_e: unknown, n: Node) => setSelectedNodeId(n.id)}
            onPaneClick={() => setSelectedNodeId(null)}
            nodeTypes={nodeTypes}
            fitView
            fitViewOptions={{ padding: 0.3 }}
            proOptions={{ hideAttribution: true }}
          >
            <Background />
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
  const isTransform = node?.data?.type === "transform";

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

          {/* P5: 可编辑显示名 */}
          {/* P5: editable display name */}
          {!isTransform && (
            <div className="space-y-0.5">
              <Label className="text-[10px] text-muted-foreground">显示名称</Label>
              <Input
                className="h-7 text-xs"
                value={(node.data.display_name as string) ?? ""}
                onChange={(e) => onUpdate({ display_name: e.target.value || undefined })}
                placeholder="留空使用默认名称"
              />
            </div>
          )}

          {isTransform ? (
            /* Transform 节点：只显示模板编辑器 */
            <>
              <div className="rounded bg-blue-100 px-2 py-1 text-[10px] text-blue-700 dark:bg-blue-900 dark:text-blue-200">
                JSON 模板映射（Handlebars 语法）
              </div>
              <div className="space-y-0.5">
                <Label className="text-[10px] text-muted-foreground">
                  模板（Template）
                </Label>
                <textarea
                  className="h-40 w-full rounded-md border bg-background px-2 py-1 font-mono text-[10px]"
                  value={
                    (node.data.config as Record<string, unknown>)?.template as string ?? ""
                  }
                  onChange={(e) =>
                    onUpdate({
                      config: { ...(node.data.config as object), template: e.target.value },
                    })
                  }
                  placeholder='{"output": "{{input}}"}'
                  spellCheck={false}
                />
                <p className="text-[9px] text-muted-foreground">
                  使用 <code className="font-mono">{"{{path.to.field}}"}</code> 引用上游输出
                </p>
              </div>
            </>
          ) : (
            <>
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

              {/* 节点配置：按 manifest inputs schema 渲染；v1 老 manifest 回退 JSON textarea */}
              {/* Node config: render per manifest inputs schema; v1 legacy manifests fall back to JSON textarea */}
              {selectedAction && isStructuredAction(selectedAction) ? (
                <ActionConfigForm
                  config={(node.data.config as Record<string, unknown>) ?? {}}
                  inputs={selectedAction.inputs}
                  onChange={(c) => onUpdate({ config: c })}
                />
              ) : (
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
              )}
            </>
          )}

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