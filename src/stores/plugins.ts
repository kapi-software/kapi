// 插件列表 store：读取 pluginDb，变更走 Rust 命令并广播 plugins:changed
// Plugin list store: reads pluginDb; mutations go through Rust commands and broadcast plugins:changed
import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { emit } from "@tauri-apps/api/event";
import { initDb, pluginDb } from "@/lib/db";
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
  installFromStore: (repo: string, dir: string) => Promise<Plugin>;
  listStore: (repo: string) => Promise<StoreEntry[]>;
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

  // 市场安装/更新：Rust 下载 zipball + 防护提取 + 校验入库，返回新插件
  // Store install/update: Rust fetches the zipball, extracts guarded, validates and
  // writes the row; returns the plugin
  async installFromStore(repo, dir) {
    const plugin = await invoke<Plugin>("store_install", { repo, dir });
    await get().load();
    await broadcastChanged();
    return plugin;
  },

  // 市场列表：Rust 拉取目录源与 manifest（非 Tauri 环境自然报错，页面兜底提示）
  // Store listing: Rust fetches the dir source and manifests (non-Tauri errors out;
  // the page surfaces the hint)
  async listStore(repo) {
    return invoke<StoreEntry[]>("store_list", { repo });
  },

  async uninstall(id) {
    await invoke("plugin_uninstall", { pluginId: id });
    await get().load();
    await broadcastChanged();
  },

  async setEnabled(id, enabled) {
    await pluginDb.updateEnabled(id, enabled);
    await get().load();
    await broadcastChanged();
  },

  async setWindowMode(id, mode) {
    await pluginDb.updateWindowMode(id, mode);
    await get().load();
    await broadcastChanged();
  },

  // 上移 / 下移：与相邻插件交换 sort_order 后整表重排
  // Move up/down: swap with the neighbor then rewrite the full ordering
  async move(id, dir) {
    const { plugins } = get();
    const index = plugins.findIndex((p) => p.id === id);
    const target = index + dir;
    if (index < 0 || target < 0 || target >= plugins.length) return;
    const next = [...plugins];
    [next[index], next[target]] = [next[target], next[index]];
    await pluginDb.updateSortOrder(next.map((p) => p.id));
    await get().load();
    await broadcastChanged();
  },
}));
