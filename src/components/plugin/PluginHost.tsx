// 统一插件宿主：iframe 加载 kapi-plugin:// 资源（内嵌与独立窗口共用）
// Unified plugin host: an iframe loading kapi-plugin:// assets (shared by embedded and independent)
// 桥接链路：postMessage → createPluginBridgeHandler → invoke('plugin_bridge')（docs/PANEL.md §3）
// Bridge: postMessage → createPluginBridgeHandler → invoke('plugin_bridge') (docs/PANEL.md §3)
import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";
import { pluginAssetUrl } from "@/lib/plugin-url";
import { createPluginBridgeHandler } from "@/lib/plugin-bridge";
import { invokeTyped, isTauri } from "@/lib/tauri";

export function PluginHost({ pluginId, className }: { pluginId: string; className?: string }) {
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

  return (
    <iframe
      ref={frameRef}
      src={pluginAssetUrl(pluginId)}
      title={`plugin-${pluginId}`}
      // 沙箱最小权限：允许脚本 / 表单；跨源隔离天然成立（kapi-plugin 与宿主不同源）
      // 不加 allow-same-origin：保持 opaque origin，iframe 内拿不到 Tauri IPC 通道
      // Minimal sandbox: scripts/forms allowed; no allow-same-origin keeps the opaque
      // origin (inherent cross-origin isolation) so the iframe has no Tauri IPC channel
      sandbox="allow-scripts allow-forms allow-popups"
      className={cn("h-full w-full border-0", className)}
    />
  );
}
