# 工作流系统

Kapi 技术文档：工作流概念模型、DAG 数据模型、执行引擎与触发器。

## 1. 概念模型与典型场景

工作流 = **触发器 + DAG 步骤图 + 数据绑定**，实现插件间数据联动。

**典型场景**：

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
  type: 'plugin' | 'transform'                         // transform = handlebars 模板
  plugin_id?: string                                   // type=plugin 时
  action?: string                                      // 插件 action 名
  config?: Record<string, unknown>                     // 节点静态配置
  template?: string                                    // type=transform 时，handlebars 模板
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
  workflow_id: string
  trigger_type: TriggerType
  config: TriggerConfig        // JSON，按 trigger_type 格式不同
  is_enabled: boolean
}

type TriggerType = 'schedule' | 'plugin_event' | 'clipboard' | 'hotkey'

// Schedule 触发器
interface ScheduleConfig { cron: string }
// PluginEvent 触发器
interface PluginEventConfig { event_type: string }
// Clipboard 触发器
interface ClipboardConfig { content_type?: 'text' | 'image' }
// Hotkey 触发器
interface HotkeyConfig { hotkey: string }
```

> 触发器由独立 `workflow_triggers` 表管理（与 workflows 表分离），支持多触发器绑定同一工作流。

## 3. 执行引擎（Rust，DAG 调度）

```rust
// src-tauri/src/workflow_engine.rs
pub struct WorkflowEngine {
    wasm: Arc<WasmRuntime>,
    triggers: TriggerManager,          // schedule / plugin_event / clipboard / hotkey
}

impl WorkflowEngine {
    pub async fn execute(&self, workflow_id: &str, trigger_data: Value) -> Result<RunResult, String> {
        // 1. 加载 graph，校验无环（拓扑排序），无入边节点为起始集
        // 2. INSERT workflow_runs (status='running')
        // 3. 调度：入度归零的节点并发执行（tokio::join）
        //    每节点：
        //    a. 按 bindings 从上下文拼装输入（含 trigger_data）
        //    b. type=plugin → wasm.invoke_action；type=transform → handlebars 渲染
        //    c. 写 workflow_step_logs（input/output/duration/status）
        //    d. 失败 → fail_fast：整条 run 标记 failed 并取消未开始节点(skipped)
        // 4. 全部完成 → 更新 run status/finished_at，返回 RunResult
    }
}
```

> `WasmRuntime::invoke_action(pool, plugin_id, action, payload)` 已随 Phase 4 落地（`src-tauri/src/wasm_runtime.rs`），直接复用。

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
- **handlebars 模板**：Transform 节点使用 `handlebars` crate 渲染，`{{path.to.field}}` 语法访问上下文数据。

## 4. 触发器系统

### 4.1 触发器类型

| 类型 | 配置字段 | 后端实现 | 说明 |
| ---- | -------- | -------- | ---- |
| `schedule` | `{ cron: string }` | `tokio::time::interval` | cron 表达式（秒 分 时 日 月 周），支持 `0 * * * * *` 等格式 |
| `plugin_event` | `{ event_type: string }` | 轮询 `plugin_events` 表 | 插件通过 `kapi.events.emit()` 发射事件 |
| `clipboard` | `{ content_type?: 'text' \| 'image' }` | `tauri-plugin-clipboard-manager` | 监听剪贴板变化 |
| `hotkey` | `{ hotkey: string }` | `tauri-plugin-global-shortcut` | 全局快捷键，如 `CmdOrCtrl+Shift+B` |

### 4.2 事件来源（PluginEvent 触发器）

选择 `plugin_event` 触发器时，事件类型下拉框合并两个来源：

1. **清单**（manifest）：所有已安装插件 `manifest.workflow.events[]` 声明的事件类型
2. **历史**（plugin_events 表）：实际出现过的事件类型（`plugins store` → `getDistinctEventTypes()`）

下拉框展示来源标签：清单 / 仅历史 / 清单+历史，支持搜索过滤。

插件通过 `kapi.events.emit(eventType, data)` 发射事件 → Rust 写入 `plugin_events` 表 → PluginEvent 触发器轮询匹配。

## 5. 前端组件

| 组件 | 文件 | 说明 |
| ---- | ---- | ---- |
| WorkflowCanvas | `src/components/workflow/WorkflowCanvas.tsx` | React Flow 画布 |
| WorkflowNodeCard | `src/components/workflow/WorkflowNodeCard.tsx` | 自定义节点（plugin / transform 区分显示） |
| NodePalette | `src/components/workflow/NodePalette.tsx` | 左侧节点面板，按插件分组 |
| TriggerDialog | `src/components/workflow/TriggerDialog.tsx` | 触发器新建/编辑弹窗 |
| TriggerListPanel | `src/components/workflow/TriggerListPanel.tsx` | 工作流触发器列表 |
| BindingsDrawer | `src/components/workflow/BindingsDrawer.tsx` | 数据绑定编辑器（vaul Drawer） |
| RunHistoryPanel | `src/components/workflow/RunHistoryPanel.tsx` | 运行历史面板 |

## 6. 数据库表

参见 `docs/DATABASE.md` §6（四表：`workflows` / `workflow_triggers` / `workflow_runs` / `workflow_step_logs`）。

## 7. API 命令

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
| `trigger_save` | `{ trigger: WorkflowTrigger }` | `()` |
| `trigger_delete` | `{ triggerId }` | `()` |
| `trigger_list` | `{ workflowId }` | `WorkflowTrigger[]` |

## 8. 尚未实现（扩展点）

| 扩展点 | 说明 |
| ------ | ---- |
| 更多节点类型 | 循环 / 条件 / HTTP 请求 / 变量等 |
| 插件签名验证 | 严格模式下验证插件签名 |
| 工作流导入/导出 | JSON 格式导入导出 |
| 工作流市场 | 分享和发现社区工作流模板 |
