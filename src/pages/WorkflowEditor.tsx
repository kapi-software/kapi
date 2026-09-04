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
import { CURRENT_GRAPH_VERSION, type Workflow, type WorkflowGraph, type WorkflowNode, type GraphError } from "@/types";
import { WorkflowNodeCard } from "@/components/workflow/WorkflowNodeCard";
import { NodePalette } from "@/components/workflow/NodePalette";
import { ActionConfigForm } from "@/components/workflow/ActionConfigForm";

// 默认节点类型注册（用于 React Flow 自定义节点）
// Default node type registry (for React Flow custom nodes)
const nodeTypes = { plugin: WorkflowNodeCard, transform: WorkflowNodeCard };

// React Flow Node → WorkflowNode
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
    display_name: (n.data?.display_name as string | undefined) || undefined,
  };
}

// React Flow 状态 → WorkflowGraph（实时校验与保存共用同一派生逻辑）
// React Flow state → WorkflowGraph (validation and save share the same derivation)
function rfToGraph(nodes: Node[], edges: Edge[]): WorkflowGraph {
  return {
    nodes: nodes.map((n) => rfNodeToGraphNode(n)),
    edges: edges.map((e) => ({
      from: e.source,
      to: e.target,
      map: (e.data as { map?: Record<string, string> } | undefined)?.map,
    })),
  };
}

// WorkflowNode → React Flow Node（仅追加 data，position 原样引用）
// WorkflowNode → React Flow Node (appends data; position passed through)
function graphNodeToRfNode(n: WorkflowNode): Node {
  return {
    id: n.id,
    type: n.type,
    position: n.position,
    data: {
      type: n.type,
      plugin_id: n.plugin_id ?? "",
      action: n.action ?? "",
      config: n.config ?? {},
      display_name: n.display_name ?? "",
    },
  };
}

// 从已有 graph 生成 React Flow nodes/edges（初始化时一次性调用）
// Build React Flow nodes/edges from a graph (one-shot at initialization)
function graphToRfState(graph: WorkflowGraph): { initNodes: Node[]; initEdges: Edge[] } {
  const initNodes: Node[] = graph.nodes.map((n) => graphNodeToRfNode(n));
  // 反序列化 edge.map（加载已保存工作流时还原连线上的数据映射）
  // Deserialize edge.map (restore data routing on edges when loading a saved workflow)
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
  // 从模板 URL 解析预填 graph（来自 NewWorkflowDialog 向导）
  // Parse prefilled graph from template URL (set by NewWorkflowDialog wizard)
  const prefilledGraph = useMemo<WorkflowGraph | null>(() => {
    const raw = searchParams.get("graph");
    if (!raw) return null;
    try {
      return JSON.parse(raw) as WorkflowGraph;
    } catch {
      return null;
    }
  }, [searchParams]);

  const { workflows, load: loadWorkflows, save, run } = useWorkflowsStore();
  const { plugins, load: loadPlugins } = usePluginsStore();

  // 数据就绪标志：workflows + plugins 都加载完才初始化编辑器（刷新后 store 是空的）
  // Hydration flag: the editor initializes only after BOTH stores are loaded
  // (after a page refresh both stores start empty)
  const [hydrated, setHydrated] = useState(false);
  const [loadingWf, setLoadingWf] = useState(true);
  const [saving, setSaving] = useState(false);
  const [running, setRunning] = useState(false);

  // 表单状态
  // Form state
  const [name, setName] = useState(prefilledName);
  const [description, setDescription] = useState(prefilledDescription);

  // React Flow 状态：唯一活跃数据源。
  // React Flow state: THE single source of truth.
  // 不再维护独立的 graph state —— 之前的 graph↔nodes 双状态同步 effect 会用
  // 过期的 initNodes 覆盖画布，导致节点位置重叠、用户编辑丢失。
  // No separate graph state anymore — the old two-way sync effect overwrote the
  // canvas with stale initNodes, scrambling node positions and losing edits.
  const [nodes, setNodes, onNodesChange] = useNodesState<Node>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  // 一次性初始化守卫：防止 store 后续刷新（如 save 内部 load()）冲掉编辑中的内容
  // One-shot init guard: later store refreshes (e.g. load() inside save) never clobber edits
  const initializedRef = useRef(false);
  const hydrateStartedRef = useRef(false);

  // 挂载时并行加载 workflows + plugins（两者就绪前渲染 loading）
  // On mount load workflows + plugins in parallel (render loading until both settle)
  useEffect(() => {
    if (hydrateStartedRef.current) return;
    hydrateStartedRef.current = true;
    setLoadingWf(true);
    Promise.all([loadWorkflows(), loadPlugins()])
      .catch(() => {
        // 非 Tauri 环境必然失败：也解除 loading，让编辑器以空数据可用
        // Non-Tauri environments always fail: clear loading anyway so the editor stays usable
      })
      .finally(() => {
        setLoadingWf(false);
        setHydrated(true);
      });
  }, [loadWorkflows, loadPlugins]);

  // 初始化（只跑一次）：数据就绪后用 store 里的工作流或 URL 模板预填画布
  // One-shot initialization: prefill the canvas from the store or URL template once hydrated
  useEffect(() => {
    if (!hydrated || initializedRef.current) return;
    initializedRef.current = true;
    const wf = workflows.find((w) => w.id === id);
    const source = wf ? wf.graph : prefilledGraph;
    if (wf) {
      setName(wf.name);
      setDescription(wf.description ?? "");
    }
    if (source) {
      const { initNodes, initEdges } = graphToRfState(source);
      setNodes(initNodes);
      setEdges(initEdges);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hydrated]);

  // 新建/编辑判断：id 在 store 中找不到即为新建（保存前）
  // isNew: id not yet in the store (before first save)
  const existingWf = workflows.find((w) => w.id === id);
  const isNew = !existingWf;

  // 实时校验：直接从 React Flow 状态派生 graph 再校验（单一数据源，无同步 effect）
  // Live validation: derive the graph straight from React Flow state (single source, no sync effect)
  const liveGraph = useMemo(() => rfToGraph(nodes, edges), [nodes, edges]);
  const validationErrors = useMemo<GraphError[]>(() => validateGraph(liveGraph), [liveGraph]);
  const hasFatal = hasFatalErrors(validationErrors);

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

  // 从左侧面板拖入新节点（只更新 React Flow 状态 —— 单一数据源）
  // Drop a new node from the left palette (React Flow state only — single source)
  const onDrop = useCallback(
    (pluginId: string, actionName: string) => {
      // 默认 display_name：优先 action.summary，否则用 "步骤 N"（按当前节点数 + 1）
      // Default display_name: prefer action.summary, else "Step N" (node count + 1)
      const plugin = plugins.find((p) => p.id === pluginId)
      const action = plugin?.manifest?.workflow?.actions?.find(
        (a) => a.name === actionName,
      )
      const summary = action?.summary?.trim()
      const defaultName = summary || `步骤 ${nodes.length + 1}`

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
    [setNodesTyped, plugins, nodes.length],
  );

  // 添加 Transform 节点
  // Add a Transform node
  const onDropTransform = useCallback(() => {
    const newNode: Node = {
      id: nextNodeId(),
      type: "transform",
      position: { x: 250 + Math.random() * 100, y: 200 + Math.random() * 60 },
      data: {
        type: "transform",
        plugin_id: "",
        action: "",
        config: { template: '{\n  "output": "{{input}}"}\n' },
        display_name: `步骤 ${nodes.length + 1}`,
      },
    };
    setNodesTyped((ns: Node[]) => [...ns, newNode]);
    setSelectedNodeId(newNode.id);
  }, [setNodesTyped, nodes.length]);

  // 连线校验辅助：按 id 查找 React Flow 节点
  // Connection helper: find a React Flow node by id
  const findNode = useCallback(
    (id: string) => nodes.find((n) => n.id === id),
    [nodes],
  )

  // 拖线过程中实时拒绝非法连接（自环 + 字段类型不匹配）
  // Reject invalid connections live during the drag (self-loop + type mismatch)
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

  // 在画布上连线：按 manifest outputs/inputs 自动生成 edge.map（同名字段自动映射）
  // Connect on the canvas: auto-generate edge.map per manifest outputs/inputs (same-name map)
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
            // 注入 edge.data.map：数据路由信息附着在边上
            // Inject edge.data.map: data routing info lives on the edge
            data: Object.keys(r.autoMap).length > 0 ? { map: r.autoMap } : undefined,
          },
          es,
        ),
      )
    },
    [setEdgesTyped, plugins, findNode],
  );

  // 删除选中节点（级联删除相关连线）
  // Delete the selected node (edges cascade)
  const deleteSelectedNode = useCallback(() => {
    if (!selectedNodeId) return;
    setNodesTyped((ns: Node[]) => ns.filter((n: Node) => n.id !== selectedNodeId));
    setEdgesTyped((es: Edge[]) => es.filter((e: Edge) => e.source !== selectedNodeId && e.target !== selectedNodeId));
    setSelectedNodeId(null);
  }, [selectedNodeId, setNodesTyped, setEdgesTyped]);

  // 更新选中节点属性（右侧面板编辑，只更新 React Flow 节点 data）
  // Update selected node properties (right panel edit, React Flow node data only)
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

  // 保存：从 React Flow 状态派生 graph 落库
  // Save: derive the graph from React Flow state and persist it
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
        graph: liveGraph,
        schema_version: CURRENT_GRAPH_VERSION,
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

  // 运行：先保存（派生 graph），再触发执行
  // Run: save first (derived graph), then execute
  const handleRun = async () => {
    if (!name.trim()) {
      toast.error(t("workflowEditor.inspector.name") + " — " + t("workflow.dialog.name"));
      return;
    }
    if (hasFatal) {
      toast.error(t("workflowEditor.validation.fatalTitle"));
      return;
    }
    setRunning(true);
    try {
      const wf: Workflow = {
        id: isNew ? `wf-${shortId()}` : (id ?? ""),
        name: name.trim(),
        description: description.trim() || null,
        graph: liveGraph,
        schema_version: CURRENT_GRAPH_VERSION,
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

  // 数据未就绪：显示 loading（避免用空画布闪渲染再被覆盖）
  // Data not ready: show loading (avoids flashing an empty canvas then overwriting it)
  if (loadingWf || !hydrated) {
    return (
      <div className="flex h-64 items-center justify-center">
        <span className="text-muted-foreground">{t("workflow.loading")}</span>
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

  // 当前节点的插件定义
  // The plugin manifest for the current node
  const plugin = plugins.find((p) => p.id === node?.data?.plugin_id);
  const actions = plugin?.manifest?.workflow?.actions ?? [];
  const selectedAction = actions.find((a) => a.name === node?.data?.action);
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

          {/* 可编辑显示名 */}
          {/* Editable display name */}
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
                  {actions.map((a) => (
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

              {/* 节点配置：始终按 manifest inputs schema 渲染结构化表单 */}
              {/* Node config: always render the schema-driven form per manifest inputs */}
              {selectedAction && (
                <ActionConfigForm
                  config={(node.data.config as Record<string, unknown>) ?? {}}
                  inputs={selectedAction.inputs}
                  onChange={(c) => onUpdate({ config: c })}
                />
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
