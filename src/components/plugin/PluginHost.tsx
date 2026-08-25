// 统一插件宿主：iframe 加载 kapi-plugin:// 资源（内嵌与独立窗口共用）
// Unified plugin host: an iframe loading kapi-plugin:// assets (shared by embedded and independent)
// 桥接 API（postMessage → plugin_bridge）属 Phase 4 后续步骤
// The bridge API (postMessage → plugin_bridge) is a later Phase 4 step
import { cn } from "@/lib/utils";
import { pluginAssetUrl } from "@/lib/plugin-url";

export function PluginHost({ pluginId, className }: { pluginId: string; className?: string }) {
  return (
    <iframe
      src={pluginAssetUrl(pluginId)}
      title={`plugin-${pluginId}`}
      // 沙箱最小权限：允许脚本 / 表单；跨源隔离天然成立（kapi-plugin 与宿主不同源）
      // Minimal sandbox: scripts/forms allowed; cross-origin isolation is inherent
      sandbox="allow-scripts allow-forms allow-popups"
      className={cn("h-full w-full border-0", className)}
    />
  );
}
