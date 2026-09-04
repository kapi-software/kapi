// 插件列表 store：读取 pluginDb，变更走 Rust 命令并广播 plugins:changed
// Plugin list store: reads pluginDb; mutations go through Rust commands and broadcast plugins:changed
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { initDb, pluginDb, eventDb } from "@/lib/db";
import type { Plugin, WindowMode } from "@/types";
import type { StoreEntry } from "@/lib/store";

// 变更后通知其它窗口（Dock 重载列表）；失败不影响本地刷新
// Notify other windows after mutations (the Dock reloads); failures don't block local refresh
async function broadcastChanged() {
  try {
    await emit("plugins:changed");
  } catch (e) {
    console.warn("plugins:changed 广播失败 / broadcast failed:", e);
  }
}

interface PluginsState {
  plugins: Plugin[];
  loading: boolean;
  load: () => Promise<void>;
  installFromDir: (sourceDir: string) => Promise<Plugin>;
  // 市场安装/更新：下载校验复制在 Rust，落库后统一刷新广播
  // Store install/update: Rust downloads/validates/copies; we refresh and broadcast after
  installFromStore: (repo: string, dir: string | null) => Promise<Plugin>;
  // 市场列表：refresh=false 读本地缓存（无缓存回源），true 强制回源并更新缓存
  // Store listing: refresh=false serves the local cache (fetching when empty), true refetches
  listStore: (refresh: boolean) => Promise<StoreEntry[]>;
  // 从 plugin_events 历史表读取所有已出现过的事件类型（按最近倒序）
  // Pull distinct event types seen in plugin_events history (most recent first)
  getDistinctEventTypes: () => Promise<string[]>;
  uninstall: (id: string) => Promise<void>;
  setEnabled: (id: string, enabled: boolean) => Promise<void>;
  setWindowMode: (id: string, mode: WindowMode) => Promise<void>;
  move: (id: string, dir: -1 | 1) => Promise<void>;
}

export const usePluginsStore = create<PluginsState>((set, get) => ({
  plugins: [],
  loading: false,

  async load() {
    // 已有数据时静默刷新（不回退骨架屏，避免模式切换/启停/排序时整页闪动）；仅首次加载显示 loading
    // Silent revalidation once data exists (no skeleton fallback, so mode switches /
    // toggles / reordering never flash the whole page); loading shows on the first load only
    if (get().plugins.length === 0) set({ loading: true });
    try {
      await initDb();
      set({ plugins: await pluginDb.getAll() });
    } finally {
      set({ loading: false });
    }
  },

  // 本地导入：Rust 校验 + 复制 + 入库，返回新插件
  // Local import: Rust validates, copies and inserts; returns the new plugin
  async installFromDir(sourceDir) {
    const plugin = await invoke<Plugin>("plugin_install", { sourceDir });
    await get().load();
    await broadcastChanged();
    return plugin;
  },

  // 市场安装/更新：Rust 下载插件仓库 zipball + 防护提取 + 校验入库，返回新插件
  // Store install/update: Rust fetches the plugin-repo zipball, extracts guarded,
  // validates and writes the row; returns the plugin
  async installFromStore(repo, dir) {
    const plugin = await invoke<Plugin>("store_install", { repo, dir });
    await get().load();
    await broadcastChanged();
    return plugin;
  },

  // 市场列表：Rust 读缓存或拉取 index.json（非 Tauri 环境自然报错，页面兜底提示）
  // Store listing: Rust serves the cache or fetches index.json (non-Tauri errors out;
  // the page surfaces the hint)
  async listStore(refresh) {
    return invoke<StoreEntry[]>("store_list", { refresh });
  },

  async getDistinctEventTypes() {
    try {
      const events = await eventDb.getRecent(500);
      const distinct = new Set<string>();
      for (const e of events) distinct.add(e.event_type);
      return Array.from(distinct);
    } catch {
      return [];
    }
  },

  async uninstall(id) {
    await invoke("plugin_uninstall", { pluginId: id });
    await get().load();
    await broadcastChanged();
  },

  // 写操作统一走 Rust 命令（模式合法性在写路径校验），前端不再直写 SQL
  // Mutations go through Rust commands (mode legality enforced in the write path)
  async setEnabled(id, enabled) {
    await invoke("plugin_set_enabled", { pluginId: id, enabled });
    await get().load();
    await broadcastChanged();
  },

  async setWindowMode(id, mode) {
    await invoke("plugin_set_window_mode", { pluginId: id, mode });
    await get().load();
    await broadcastChanged();
  },

  // 上移 / 下移：与相邻插件交换后整表重排（入参即全量顺序）
  // Move up/down: swap with the neighbor then rewrite the full ordering
  async move(id, dir) {
    const { plugins } = get();
    const index = plugins.findIndex((p) => p.id === id);
    const target = index + dir;
    if (index < 0 || target < 0 || target >= plugins.length) return;
    const next = [...plugins];
    [next[index], next[target]] = [next[target], next[index]];
    await invoke("plugin_reorder", { orderedIds: next.map((p) => p.id) });
    await get().load();
    await broadcastChanged();
  },
}));
