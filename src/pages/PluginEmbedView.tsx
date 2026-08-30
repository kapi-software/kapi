// 内嵌插件视图：/plugin/:id（独立页面，无标题/简介/返回按钮，由 TopBar 提供返回）
// Embedded plugin view: /plugin/:id (standalone, no header/description/back button)
import { useParams, useSearchParams } from "react-router-dom";
import { PluginHost } from "@/components/plugin/PluginHost";
import { safeEntry } from "@/lib/plugin-url";

export default function PluginEmbedView() {
  const { id } = useParams<{ id: string }>();
  // 形态入口由 plugin:navigate 携带来（App.tsx 拼进 ?entry=），非法值回退 index.html
  // The per-shape entry arrives via plugin:navigate (App.tsx appends ?entry=); invalid → index.html
  const [searchParams] = useSearchParams();
  const entry = safeEntry(searchParams.get("entry"));
  if (!id) return null;
  return (
    // flex-1 min-h-0 在 flex-col 父容器中精确填满剩余空间，避开 main padding 的影响
    // -m-3 md:-m-4 反向抵消 StandaloneLayout main 的 p-3 md:p-4
    // flex-1 min-h-0 reliably fills the remaining space inside a flex-col parent
    // -m-3 md:-m-4 counters StandaloneLayout main's padding
    <div className="-m-3 flex min-h-0 flex-1 flex-col overflow-hidden md:-m-4">
      <PluginHost pluginId={id} entry={entry} className="flex-1" />
    </div>
  );
}