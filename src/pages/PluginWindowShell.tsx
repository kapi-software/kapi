// 插件独立窗口壳：/plugin-window/:id（裸 PluginHost，docs/ARCHITECTURE.md §2.2）
// Plugin independent window shell: /plugin-window/:id (a bare PluginHost)
import { useEffect, useRef, useState } from "react";
import { useParams, useSearchParams } from "react-router-dom";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { PluginHost } from "@/components/plugin/PluginHost";
import { safeEntry } from "@/lib/plugin-url";
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

// 就绪后显示窗口：窗口以 visible:false 创建，内容就绪（iframe onLoad 或 1.5s 兜底定时）
// 才 show + setFocus，避免启动白屏闪烁；只执行一次，返回"就绪"回调给 iframe onLoad。
// Show the window once ready: created with visible:false, it shows (and focuses) only
// after the content is ready — iframe onLoad or a 1.5s fallback timer — avoiding the
// white startup flash. Runs at most once; returns the ready callback for iframe onLoad.
function useShowWhenReady(enabled: boolean): () => void {
  const showRef = useRef<() => void>(() => {});

  useEffect(() => {
    if (!enabled) return;
    let done = false;
    let timer: number | null = null;
    const show = async () => {
      if (done) return;
      done = true;
      if (timer !== null) window.clearTimeout(timer);
      try {
        const win = getCurrentWindow();
        await win.show();
        await win.setFocus();
      } catch {
        // 显示失败不阻断渲染 / a failed show never blocks rendering
      }
    };
    showRef.current = () => void show();
    // 兜底：iframe 迟迟不触发 onLoad（加载失败等）也要把窗口放出来
    // Fallback: surface the window even if iframe onLoad never fires
    timer = window.setTimeout(show, 1500);
    return () => {
      if (timer !== null) window.clearTimeout(timer);
    };
  }, [enabled]);

  return () => showRef.current();
}

export default function PluginWindowShell() {
  const { id: paramId } = useParams<{ id: string }>();
  // 形态入口由 Rust 建窗时拼进 ?entry=（create_plugin_window），非法值回退 index.html
  // The per-shape entry rides ?entry= when Rust creates the window (create_plugin_window);
  // invalid values fall back to index.html
  const [searchParams] = useSearchParams();
  const entry = safeEntry(searchParams.get("entry"));
  // id 权威来源是路由参数（完整插件 id）；label 作回退（其中 "." 已被替换为 "_"，有损）
  // The route param carries the authoritative plugin id; the label is a lossy
  // fallback (its "." chars were replaced with "_" to satisfy Tauri label rules)
  const label = isTauri() ? getCurrentWindow().label : "";
  const labelId = label.startsWith("plugin-") ? label.slice("plugin-".length) : undefined;
  const id = paramId ?? labelId?.replace(/_/g, ".");

  // null = 加载中：首帧即透明，避免透明窗口闪过一帧不透明背景
  // null = loading: the first frame is already transparent, no opaque flash
  const [transparent, setTransparent] = useState<boolean | null>(null);
  const markReady = useShowWhenReady(isTauri());

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
      <PluginHost pluginId={id} entry={entry} onLoaded={markReady} />
    </div>
  );
}
