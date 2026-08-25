// 插件独立窗口壳：/plugin-window/:id（裸 PluginHost，docs/ARCHITECTURE.md §2.2）
// Plugin independent window shell: /plugin-window/:id (a bare PluginHost)
import { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PluginHost } from "@/components/plugin/PluginHost";
import { initDb, pluginDb } from "@/lib/db";
import { isTauri } from "@/lib/tauri";

// 读取 window_config.transparent；非 Tauri 或读取失败一律按不透明处理
// Read window_config.transparent; non-Tauri or failures fall back to opaque
async function loadTransparent(pluginId: string): Promise<boolean> {
  if (!isTauri()) return false;
  try {
    // 壳自己建立数据库连接：独立窗口链路不依赖主窗口是否加载过 DB
    // The shell opens the DB itself: no dependency on the main window having loaded it
    await initDb();
    const plugin = await pluginDb.getById(pluginId);
    return Boolean(plugin?.window_config?.transparent);
  } catch {
    return false;
  }
}

export default function PluginWindowShell() {
  const { id: paramId } = useParams<{ id: string }>();
  // id 权威来源是路由参数（完整插件 id）；label 作回退（其中 "." 已被替换为 "_"，有损）
  // The route param carries the authoritative plugin id; the label is a lossy
  // fallback (its "." chars were replaced with "_" to satisfy Tauri label rules)
  const label = isTauri() ? getCurrentWindow().label : "";
  const labelId = label.startsWith("plugin-") ? label.slice("plugin-".length) : undefined;
  const id = paramId ?? labelId?.replace(/_/g, ".");

  // null = 加载中：首帧即透明，避免透明窗口闪过一帧不透明背景
  // null = loading: the first frame is already transparent, no opaque flash
  const [transparent, setTransparent] = useState<boolean | null>(null);

  useEffect(() => {
    if (!id) return;
    let cancelled = false;
    loadTransparent(id).then((value) => {
      if (!cancelled) setTransparent(value);
    });
    return () => {
      cancelled = true;
    };
  }, [id]);

  // 透明模式：内联样式覆盖 html 与 body 的不透明 bg-background（index.css @layer base 所设）
  // Transparent mode: inline styles override the opaque html/body bg-background set by index.css
  useEffect(() => {
    if (transparent !== true) return;
    const { documentElement, body } = document;
    documentElement.style.background = "transparent";
    body.style.background = "transparent";
    return () => {
      documentElement.style.background = "";
      body.style.background = "";
    };
  }, [transparent]);

  if (!id || transparent === null) return null;

  return (
    <div className={transparent ? "h-svh w-full" : "h-svh w-full bg-background"}>
      <PluginHost pluginId={id} />
    </div>
  );
}
