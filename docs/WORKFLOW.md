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

// 节点执行上下文：trigger 原始数据 + 各节点已产出输出
struct WorkflowContext {
    trigger: Value,
    outputs: HashMap<String /* node_id */, Value>,
}
```

要点：

- **真正的 DAG 语义**：按拓扑序调度，无依赖关系的节点**并行执行**，数据流由 `bindings` 显式映射。
- **两级日志**：`workflow_runs`（一次触发）+ `workflow_step_logs`（每节点输入/输出/耗时），编辑器与日志页可逐步回放。
- **触发器**：clipboard 用 `tauri-plugin-clipboard-manager` 监听；hotkey 用 `tauri-plugin-global-shortcut`；schedule 用 tokio 定时；plugin_event 监听事件总线（`plugin_events` 表）。
