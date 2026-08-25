// 活跃插件窗口检测：列出 plugin-* 窗口并在创建/销毁时刷新（Plugins 页禁用模式切换用）
// Live plugin-window detection: lists plugin-* windows, refreshed on create/destroy
// (used by the Plugins page to lock the mode switcher)
import { useEffect, useState } from "react";
import { getAllWindows } from "@tauri-apps/api/window";
import { isTauri, onEvent } from "@/lib/tauri";

// 窗口 label 映射（与 Rust plugin_window_label 一致："." → "_"）
// Window-label mapping (matches the Rust plugin_window_label: "." -> "_")
export function pluginWindowLabel(pluginId: string): string {
  return `plugin-${pluginId.replace(/\./g, "_")}`;
}

// 当前存在的插件窗口 label 集合（非 Tauri 环境恒为空）
// The set of live plugin-window labels (always empty outside Tauri)
export function usePluginWindows(): Set<string> {
  const [labels, setLabels] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    const refresh = async () => {
      try {
        const windows = await getAllWindows();
        if (cancelled) return;
        setLabels(
          new Set(windows.map((w) => w.label).filter((l) => l.startsWith("plugin-"))),
        );
      } catch {
        // 权限或环境异常时保持上次结果 / keep the last result on permission/env errors
      }
    };

    void refresh();
    // 窗口创建/销毁实时刷新（独立窗口打开、关闭、卸载清理）
    // Refresh live on window create/destroy (independent windows open, close, uninstall)
    void onEvent("tauri://window-created", refresh).then((un) => unlisteners.push(un));
    void onEvent("tauri://window-destroyed", refresh).then((un) => unlisteners.push(un));

    return () => {
      cancelled = true;
      unlisteners.forEach((un) => un());
    };
  }, []);

  return labels;
}
