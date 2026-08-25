// 插件独立窗口壳：/plugin-window/:id（裸 PluginHost，docs/ARCHITECTURE.md §2.2）
// Plugin independent window shell: /plugin-window/:id (a bare PluginHost)
import { useParams } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PluginHost } from "@/components/plugin/PluginHost";
import { isTauri } from "@/lib/tauri";

export default function PluginWindowShell() {
  const { id: paramId } = useParams<{ id: string }>();
  // id 权威来源是路由参数（完整插件 id）；label 作回退（其中 "." 已被替换为 "_"，有损）
  // The route param carries the authoritative plugin id; the label is a lossy
  // fallback (its "." chars were replaced with "_" to satisfy Tauri label rules)
  const label = isTauri() ? getCurrentWindow().label : "";
  const labelId = label.startsWith("plugin-") ? label.slice("plugin-".length) : undefined;
  const id = paramId ?? labelId?.replace(/_/g, ".");

  if (!id) return null;

  return (
    <div className="h-svh w-full bg-background">
      <PluginHost pluginId={id} />
    </div>
  );
}
