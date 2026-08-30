// 工作流 store：列表 CRUD + 手动执行；变更走 Tauri 命令并广播 workflows:changed
// Workflow store: CRUD + manual run; mutations go through Tauri commands and broadcast workflows:changed
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import type { Workflow, WorkflowRun, WorkflowStepLog } from "@/types";

// 变更后通知其它窗口（编辑器 / 日志页订阅）；失败不影响本地刷新
// Notify other windows after mutations (editor / logs page subscribe); failures don't block local refresh
async function broadcastChanged() {
  try {
    await emit("workflows:changed");
  } catch (e) {
    console.warn("workflows:changed 广播失败 / broadcast failed:", e);
  }
}

interface WorkflowsState {
  workflows: Workflow[];
  loading: boolean;
  load: () => Promise<void>;
  save: (w: Workflow) => Promise<void>;
  remove: (id: string) => Promise<void>;
  run: (id: string) => Promise<WorkflowRun>;
  getRuns: (id: string, limit?: number) => Promise<WorkflowRun[]>;
  // 拉取单次运行的步骤日志（前端按 run_id 走 workflow_run_steps）
  // Fetch step logs for a single run (frontend hits workflow_run_steps by run_id)
  getRunSteps: (runId: number) => Promise<WorkflowStepLog[]>;
}

export const useWorkflowsStore = create<WorkflowsState>((set, get) => ({
  workflows: [],
  loading: false,

  async load() {
    if (get().workflows.length === 0) set({ loading: true });
    try {
      set({ workflows: await invoke<Workflow[]>("workflow_list") });
    } finally {
      set({ loading: false });
    }
  },

  async save(w) {
    await invoke("workflow_save", { workflow: w });
    await get().load();
    await broadcastChanged();
  },

  async remove(id) {
    await invoke("workflow_delete", { workflowId: id });
    await get().load();
    await broadcastChanged();
  },

  // 手动触发：直接走 workflow_execute 命令，引擎完成 DAG 调度 + 落库后返回 run
  // Manual run: straight to workflow_execute; the engine schedules and persists the run
  async run(id) {
    return invoke<WorkflowRun>("workflow_execute", { workflowId: id });
  },

  async getRuns(id, limit = 20) {
    return invoke<WorkflowRun[]>("workflow_runs", { workflowId: id, limit });
  },

  async getRunSteps(runId) {
    return invoke<WorkflowStepLog[]>("workflow_run_steps", { runId });
  },
}));