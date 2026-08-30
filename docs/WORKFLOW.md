# 工作流系统

Kapi 技术文档：工作流概念模型、DAG 数据模型与执行引擎。

## 1. 概念模型与典型场景

工作流 = **触发器 + DAG 步骤图 + 数据绑定**，实现插件间数据联动。

**典型场景（能力目标，示例插件暂不实现）**：

```text
[触发器] clipboard_changed（剪贴板出现新内容）
   └─> [节点1] 代码美化插件 . save(text)      → 保存并输出格式化结果
          └─> [节点2] 截图插件 . render({ content: 节点1.formatted })
                                              → 生成一张截图文件
```

一次剪贴板事件，多个插件按图依次联动处理——工作流引擎负责调度与数据传递，插件只实现自己的 action。

## 2. 数据模型

```typescript
// src/types/index.ts
interface Workflow {
  id: string
  name: string
  description: string
  enabled: boolean
  graph: WorkflowGraph
  created_at: string
  updated_at: string
}

// DAG 图，持久化为 JSON 存 workflows.graph
interface WorkflowGraph {
  nodes: WorkflowNode[]
  edges: Array<{ from: string; to: string }>          // 节点执行依赖
  bindings: DataBinding[]                              // 节点间数据映射
}

interface WorkflowNode {
  id: string                                           // 如 'n1'
  type: 'plugin' | 'transform'                         // v1 内置 transform（JSON 映射，无代码）
  plugin_id?: string                                   // type=plugin 时
  action?: string                                      // 插件 action 名
  config?: Record<string, unknown>                     // 节点静态配置
}

// 数据绑定：源节点输出 key → 目标节点输入 key
interface DataBinding {
  from: string
  output: string
  to: string
  input: string
}

interface WorkflowTrigger {
  id: string
  type: 'clipboard' | 'hotkey' | 'schedule' | 'manual' | 'plugin_event'
  config: Record<string, unknown>                      // 如 { hotkey: 'CmdOrCtrl+Shift+B', cron: '...' }
  workflow_id: string                                   // 触发后执行的工作流
}
```

> 触发器不放入 `graph`，由引擎单独管理注册表（内存态），触发配置冗余存 `graph` 顶层便于编辑器渲染，加载时重建注册。

## 3. 执行引擎（Rust，DAG 调度）

```rust
// src-tauri/src/workflow_engine.rs（示意）
pub struct WorkflowEngine {
    wasm: Arc<WasmRuntime>,
    triggers: TriggerManager,          // clipboard / hotkey / schedule / manual / plugin_event
}

impl WorkflowEngine {
    pub async fn execute(&self, workflow_id: &str, trigger_data: Value) -> Result<RunResult, String> {
        // 1. 加载 graph，校验无环（拓扑排序），无入边节点为起始集
        // 2. INSERT workflow_runs (status='running')
        // 3. 调度：入度归零的节点并发执行（tokio::join）
        //    每节点：
        //    a. 按 bindings 从上下文拼装输入（含 trigger_data）
        //    b. type=plugin → wasm.invoke_action；type=transform → 纯 JSON 映射
        //    c. 写 workflow_step_logs（input/output/duration/status）
        //    d. 失败 → v1 策略 fail_fast：整条 run 标记 failed 并取消未开始节点(skipped)
        // 4. 全部完成 → 更新 run status/finished_at，返回 RunResult
    }
}
```

> **已就绪依赖**：`WasmRuntime::invoke_action(pool, plugin_id, action, payload)` 已随 Phase 4 落地（`src-tauri/src/wasm_runtime.rs`，含 fuel/epoch/内存沙箱与权限守卫），即上述 `wasm.invoke_action` 的正式入口——Phase 6 引擎实现时直接复用，无需再写 WASM 调用层。插件向工作流暴露的 action 面即 manifest.workflow.actions（示例见 `plugins/pluginD`）。

```rust
// 节点执行上下文：trigger 原始数据 + 各节点已产出输出
struct WorkflowContext {
    trigger: Value,
    outputs: HashMap<String /* node_id */, Value>,
}
```

要点：

- **真正的 DAG 语义**：按拓扑序调度，无依赖关系的节点**并行执行**，数据流由 `bindings` 显式映射。
- **两级日志**：`workflow_runs`（一次触发）+ `workflow_step_logs`（每节点输入/输出/耗时），编辑器与日志页可逐步回放。

---

## §4 Phase 6 落地说明

> 本节记录 Phase 6 已实现部分与尚未实现的扩展点。

### 4.1 已实现

| 能力 | 文件 | 说明 |
| ---- | ---- | ---- |
| DAG 调度 | `src-tauri/src/workflow_engine.rs` | Kahn 拓扑排序，fail_fast，节点内 `tokio::join!` 并发 |
| plugin 节点执行 | `workflow_engine.rs::execute_one_node` | 调用 `WasmRuntime::invoke_action` |
| transform 节点 | `workflow_engine.rs::execute_one_node` | 记录 warning 日志后跳过（占位） |
| 两级日志 | `workflow_engine.rs` | `workflow_runs` + `workflow_step_logs` |
| Manual 触发命令 | `src-tauri/src/lib.rs` | `workflow_execute / workflow_get / workflow_list / workflow_save / workflow_delete / workflow_runs / workflow_run_steps` |
| 前端 store | `src/stores/workflows.ts` | Zustand store（模式同 `plugins.ts`）；含 `getRunSteps` |
| Workflow 列表页 | `src/pages/Workflow.tsx` | 卡片 + 启停 + 运行 + 跳转路由入口 |
| Workflow 编辑器页 | `src/pages/WorkflowEditor.tsx` | 新路由 `/workflow/new` 与 `/workflow/:id/edit`；React Flow 可视化（@xyflow/react v12） |
| Node palette | `src/components/workflow/NodePalette.tsx` | 左栏按 plugin 分组的 action 列表；点击或 HTML5 拖拽到画布 |
| Node inspector | `WorkflowEditor.tsx::NodeInspector` | 右栏选中节点编辑（plugin / action / config JSON） |
| Bindings editor | `src/components/workflow/BindingsEditor.tsx` | 底栏源→输出→目标→输入字段映射表；选中下游节点时聚焦 |
| 自定义节点 | `src/components/workflow/WorkflowNodeCard.tsx` | React Flow 自定义节点（plugin 名 + action + Handle） |
| 运行历史面板 | `src/components/workflow/RunHistoryPanel.tsx` | 列表页（折叠模式）+ 历史页（整页模式）共用 |
| Workflow 运行历史页 | `src/pages/WorkflowRuns.tsx` | 新路由 `/workflow/:id/runs`；整页展示历史 |
| 运行历史面板（已抽） | `src/pages/Workflow.tsx::RunHistoryPanel`（旧内联） | 已抽出到独立文件，列表页改用折叠卡入口 |

### 4.2 尚未实现（扩展点）

| 扩展点 | 状态 | 说明 |
| ------ | ---- | ---- |
| clipboard 触发器 | 占位 | `TriggerType::Clipboard`，需 `tauri-plugin-clipboard-manager` 轮询 |
| hotkey 触发器 | 占位 | `TriggerType::Hotkey`，需 `tauri-plugin-global-shortcut` |
| schedule 触发器 | 占位 | `TriggerType::Schedule`，需 `tokio::time::interval` |
| plugin_event 触发器 | 占位 | `TriggerType::PluginEvent`，需监听 `plugin_events` 表 |
| transform 节点实现 | 占位 | JSON 模板映射（`jq` 或手动拼装） |
| React Flow 可视化编辑器 | 已做 | Phase 7 完成；`/workflow/new` 与 `/workflow/:id/edit` 两路由；编辑器含 palette / canvas / inspector / bindings 四区域 |

### 4.3 数据库表

参见 `docs/DATABASE.md` §6（三表：`workflows` / `workflow_runs` / `workflow_step_logs`）。

### 4.4 API 命令

所有命令均通过 Tauri `invoke` 调用：

| 命令 | 参数 | 返回 |
| ---- | ---- | ---- |
| `workflow_execute` | `{ workflowId }` | `WorkflowRun` |
| `workflow_get` | `{ workflowId }` | `Workflow \| null` |
| `workflow_list` | — | `Workflow[]` |
| `workflow_save` | `{ workflow: Workflow }` | `()` |
| `workflow_delete` | `{ workflowId }` | `()` |
| `workflow_runs` | `{ workflowId, limit }` | `WorkflowRun[]` |
| `workflow_run_steps` | `{ runId }` | `WorkflowStepLog[]` |

---

- **触发器**：clipboard 用 `tauri-plugin-clipboard-manager` 监听；hotkey 用 `tauri-plugin-global-shortcut`；schedule 用 tokio 定时；plugin_event 监听事件总线（`plugin_events` 表）。
