// 插件独立窗口壳：/plugin-window/:id（裸 PluginHost，docs/ARCHITECTURE.md §2.2）
// Plugin independent window shell: /plugin-window/:id (a bare PluginHost)
import { useParams } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PluginHost } from "@/components/plugin/PluginHost";
import { isTauri } from "@/lib/tauri";

export default function PluginWindowShell() {
  const { id: paramId } = useParams<{ id: string }>();
  // id 权威来源是窗口 label（plugin-<id>）；路由参数作回退
  // The window label (plugin-<id>) is authoritative; the route param is a fallback
  const label = isTauri() ? getCurrentWindow().label : "";
  const id = label.startsWith("plugin-") ? label.slice("plugin-".length) : paramId;

  if (!id) return null;

  return (
    <div className="h-svh w-full bg-background">
      <PluginHost pluginId={id} />
    </div>
  );
}
