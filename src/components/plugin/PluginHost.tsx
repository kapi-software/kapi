// 统一插件宿主：iframe 加载 kapi-plugin:// 资源（内嵌与独立窗口共用）
// Unified plugin host: an iframe loading kapi-plugin:// assets (shared by embedded and independent)
// 桥接链路：postMessage → createPluginBridgeHandler → invoke('plugin_bridge')（docs/PANEL.md §3）
// Bridge: postMessage → createPluginBridgeHandler → invoke('plugin_bridge') (docs/PANEL.md §3)
import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import { cn } from "@/lib/utils";
import { pluginAssetUrl } from "@/lib/plugin-url";
import { createPluginBridgeHandler } from "@/lib/plugin-bridge";
import { invokeTyped, isTauri } from "@/lib/tauri";

export function PluginHost({
  pluginId,
  entry,
  className,
  onLoaded,
}: {
  pluginId: string;
  // 入口文件（web/ 相对路径，来自 manifest windows[].entry 的形态入口；缺省 index.html）
  // Entry file (web/-relative, the per-shape entry from manifest windows[].entry; default index.html)
  entry?: string;
  className?: string;
  // iframe 加载完成回调（独立窗口壳用于就绪后 show，防启动闪白）
  // iframe load callback (the shell shows the window once ready, avoiding a startup flash)
  onLoaded?: () => void;
}) {
  const frameRef = useRef<HTMLIFrameElement>(null);

  // 桥接监听：仅 Tauri 环境挂载（浏览器预览无 IPC）；pluginId 变更时重建
  // Bridge listener: mounted only inside Tauri (no IPC in the browser preview); rebuilt per pluginId
  useEffect(() => {
    if (!isTauri()) return;
    const onMessage = createPluginBridgeHandler({
      pluginId,
      getTargetWindow: () => frameRef.current?.contentWindow ?? null,
      invoke: (command, args) => invokeTyped(command, args),
    });
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [pluginId]);

  // 宿主推送转发：Rust 定向发来 plugin:event（本插件订阅的事件）→ postMessage 进 iframe，
  // 由 SDK（kapi.events.on）分发给插件回调
  // Host push relay: Rust targets this window with plugin:event for the plugin's own
  // subscriptions -> postMessage into the iframe, where the SDK dispatches to callbacks
  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<{ pluginId: string; type: string; data: unknown; source: string }>(
      "plugin:event",
      (e) => {
        if (e.payload?.pluginId !== pluginId) return;
        frameRef.current?.contentWindow?.postMessage(
          {
            kapiEvent: true,
            type: e.payload.type,
            data: e.payload.data ?? null,
            source: e.payload.source,
          },
          "*"
        );
      }
    );
    return () => {
      un.then((f) => f());
    };
  }, [pluginId]);

  return (
    <iframe
      ref={frameRef}
      src={pluginAssetUrl(pluginId, entry)}
      title={`plugin-${pluginId}`}
      onLoad={onLoaded}
      // 沙箱最小权限：允许脚本 / 表单；跨源隔离天然成立（kapi-plugin 与宿主不同源）
      // 不加 allow-same-origin：保持 opaque origin，iframe 内拿不到 Tauri IPC 通道
      // Minimal sandbox: scripts/forms allowed; no allow-same-origin keeps the opaque
      // origin (inherent cross-origin isolation) so the iframe has no Tauri IPC channel
      sandbox="allow-scripts allow-forms allow-popups"
      className={cn("h-full w-full border-0", className)}
    />
  );
}
