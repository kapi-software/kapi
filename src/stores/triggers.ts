// 工作流触发器 store：CRUD + 同步广播
// Workflow trigger store: CRUD + sync broadcast
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import type { WorkflowTrigger } from "@/types";

async function broadcastChanged() {
  try {
    await emit("triggers:changed");
  } catch (e) {
    console.warn("triggers:changed 广播失败 / broadcast failed:", e);
  }
}

interface TriggersState {
  triggers: WorkflowTrigger[];
  loading: boolean;
  load: (workflowId?: string) => Promise<void>;
  save: (t: WorkflowTrigger) => Promise<void>;
  remove: (id: string) => Promise<void>;
}

export const useTriggersStore = create<TriggersState>((set, get) => ({
  triggers: [],
  loading: false,

  async load(workflowId) {
    set({ loading: true });
    try {
      const triggers = await invoke<WorkflowTrigger[]>("trigger_list", { workflowId });
      set({ triggers });
    } finally {
      set({ loading: false });
    }
  },

  async save(t) {
    await invoke("trigger_save", { trigger: t });
    await get().load();
    await broadcastChanged();
  },

  async remove(id) {
    await invoke("trigger_delete", { triggerId: id });
    await get().load();
    await broadcastChanged();
  },
}));
