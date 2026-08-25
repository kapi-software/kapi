// 内嵌插件视图：/plugin/:id（面板外壳内的 PluginHost，docs/PANEL.md）
// Embedded plugin view: /plugin/:id (PluginHost inside the panel shell)
import { useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { PluginHost } from "@/components/plugin/PluginHost";
import { isTauri } from "@/lib/tauri";
import { initDb, pluginDb } from "@/lib/db";
import type { Plugin } from "@/types";

export default function PluginEmbedView() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [plugin, setPlugin] = useState<Plugin | null>(null);

  // 插件元信息仅用于页头展示；iframe 加载不依赖它
  // Plugin metadata is for the header only; the iframe loads independently
  useEffect(() => {
    if (!id || !isTauri()) return;
    let cancelled = false;
    initDb()
      .then(() => pluginDb.getById(id))
      .then((p) => {
        if (!cancelled) setPlugin(p);
      })
      .catch((e) => console.error("插件信息加载失败 / plugin info load failed:", e));
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (!id) return null;

  return (
    <div className="flex h-full flex-col gap-4">
      <div className="flex items-center gap-3">
        <Button variant="ghost" size="icon" onClick={() => navigate(-1)}>
          <ArrowLeft />
        </Button>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="truncate text-lg font-semibold">
              {plugin?.name ?? id}
            </h1>
            {plugin && <Badge variant="secondary">v{plugin.version}</Badge>}
          </div>
          {plugin?.description && (
            <p className="truncate text-xs text-muted-foreground">{plugin.description}</p>
          )}
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-hidden rounded-md border bg-background">
        <PluginHost pluginId={id} />
      </div>
    </div>
  );
}
