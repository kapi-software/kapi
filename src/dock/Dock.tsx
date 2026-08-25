// Dock 窗口入口：弧形插件栏（motion 动画、滚轮循环、点击唤醒）
// Dock window entry: arc-shaped plugin bar (motion animations, wheel cycling, click-to-wake)
// 规格见 docs/DOCK.md §2–§3；状态权威在 Rust（dock:state 事件），浏览器预览用本地状态机
// Spec in docs/DOCK.md §2–§3; Rust owns the state (dock:state events), browser preview uses a local machine
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { AnimatePresence, motion } from "motion/react";
import { ChevronLeft } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@/lib/tauri";
import { initDb, pluginDb } from "@/lib/db";
import { calculateDockPositions, DOCK_WIDTH, DOCK_HEIGHT } from "@/lib/dock-arc";
import { useSettingsStore } from "@/stores/settings";
import type { Plugin } from "@/types";

// Dock 渲染条目（来自 plugins 表；icon 为空时回退首字母/演示图标）
// Dock render item (from the plugins table; falls back to initial/demo icon)
interface DockItem {
  id: string;
  name: string;
  icon: string | null;
}

// 演示条目：插件系统（Phase 4）落地前的占位，便于浏览器与空库预览
// Demo items: placeholders until the plugin system (Phase 4), for browser and empty-DB preview
const DEMO_ITEMS: Array<{ id: string; nameKey: string; icon: null }> = [
  { id: "demo-0", nameKey: "dock.demoClipboard", icon: null },
  { id: "demo-1", nameKey: "dock.demoScreenshot", icon: null },
  { id: "demo-2", nameKey: "dock.demoSnippet", icon: null },
  { id: "demo-3", nameKey: "dock.demoImage", icon: null },
  { id: "demo-4", nameKey: "dock.demoPlugin", icon: null },
];

// 动画速度档位 → 秒（docs/DOCK.md §2.3：展开/收起基准 0.18s）
// Animation speed preset → seconds (docs/DOCK.md §2.3: 0.18s base)
const SPEED_SECONDS = { slow: 0.28, medium: 0.18, fast: 0.12 } as const;

// 展开/收起缓动与箭头弹性曲线（docs/DOCK.md §2.3）
// Easings for expand/collapse and the arrow spring (docs/DOCK.md §2.3)
const EXPAND_EASE: [number, number, number, number] = [0.22, 1, 0.36, 1];
const ARROW_SPRING: [number, number, number, number] = [0.34, 1.56, 0.64, 1];

// 单个插件圆点：64px，选中 1.12 倍 + 光晕 + 名称胶囊，hover 1.1 倍
// One plugin dot: 64px, selected scales 1.12 with glow + name pill, hover 1.1
function DockDot({
  item,
  slot,
  scale,
  duration,
  onClick,
  onHover,
}: {
  item: DockItem;
  slot: { x: number; y: number; actualIndex: number; isActive: boolean };
  scale: number;
  duration: number;
  onClick: () => void;
  onHover: (v: boolean) => void;
}) {
  return (
    <motion.button
      type="button"
      // key 用 actualIndex：滚轮换位时同一插件在槽位间滑动
      // Keyed by actualIndex: the same plugin slides between slots on wheel
      className="absolute top-0 left-0 flex size-16 cursor-pointer items-center justify-center rounded-full border border-white/10 bg-zinc-900/80 backdrop-blur-md"
      initial={{ opacity: 0, scale: 0.6 }}
      animate={{
        // 圆心定位：弧线坐标 − 半径（32px）
        // Center on the arc point minus the radius (32px)
        x: slot.x - 32,
        y: slot.y - 32,
        scale,
        opacity: 1,
      }}
      exit={{ opacity: 0, scale: 0.6 }}
      transition={{ duration, ease: EXPAND_EASE }}
      onClick={onClick}
      onMouseEnter={() => onHover(true)}
      onMouseLeave={() => onHover(false)}
      style={
        slot.isActive
          ? {
              // 选中光晕：跟随主题强调色（--primary 由设置页实时写入）
              // Selected glow: follows the theme accent (--primary is set live by settings)
              boxShadow: "0 0 18px 2px color-mix(in srgb, var(--primary) 50%, transparent)",
              borderColor: "var(--primary)",
            }
          : undefined
      }
    >
      {item.icon ? (
        <span className="text-2xl leading-none">{item.icon}</span>
      ) : (
        <span className="text-lg font-semibold text-white/90 select-none">
          {item.name.slice(0, 1).toUpperCase()}
        </span>
      )}

      {/* 选中位名称胶囊（仅选中显示）*/}
      {/* Name pill on the selected slot only */}
      {slot.isActive && (
        <span className="absolute -bottom-7 rounded-full bg-zinc-900/90 px-2 py-0.5 text-[8px] whitespace-nowrap text-white/80">
          {item.name}
        </span>
      )}
    </motion.button>
  );
}

export default function DockApp() {
  const { t } = useTranslation();
  const settings = useSettingsStore((s) => s.settings);
  const { dock_enabled, dock_visible_items, dock_animation_speed, dock_auto_hide_delay } = settings;

  const [plugins, setPlugins] = useState<Plugin[]>([]);
  const [offset, setOffset] = useState(0);
  const [expanded, setExpanded] = useState(false);
  const [triggerHover, setTriggerHover] = useState(false);
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Dock 窗口必须透明：覆盖全局 body 背景（index.css 默认 bg-background）
  // The dock window must be transparent: override the global body background
  useEffect(() => {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }, []);

  // 加载已安装插件；空库或失败时保留演示条目
  // Load installed plugins; keep demo items on empty DB or failure
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    initDb()
      .then(() => pluginDb.getAll())
      .then((rows) => {
        if (!cancelled) setPlugins(rows);
      })
      .catch((e) => console.error("Dock 插件列表加载失败 / Dock plugin load failed:", e));
    return () => {
      cancelled = true;
    };
  }, []);

  // Tauri：状态由 Rust 权威下发（docs/DOCK.md §3），本地不维护副本
  // Tauri: state comes authoritatively from Rust (docs/DOCK.md §3); no local copy
  useEffect(() => {
    if (!isTauri()) return;
    const un = listen<string>("dock:state", (e) => {
      setExpanded(e.payload === "expanded");
    });
    return () => {
      un.then((f) => f());
    };
  }, []);

  // Tauri：推送 Dock 设置给 Rust 轮询服务（启用开关 + 自动隐藏延迟）
  // Tauri: push dock settings to the Rust polling service
  useEffect(() => {
    if (!isTauri()) return;
    invoke("dock_set_config", {
      enabled: dock_enabled,
      autoHideMs: dock_auto_hide_delay,
    }).catch((e) => console.error("dock_set_config 失败 / failed:", e));
  }, [dock_enabled, dock_auto_hide_delay]);

  // 浏览器预览：本地展开/自动收起（Tauri 下由 Rust 轮询负责）
  // Browser preview: local expand/auto-collapse (Rust polling owns this in Tauri)
  useEffect(() => {
    if (isTauri() || !expanded) return;
    const timer = setTimeout(() => setExpanded(false), dock_auto_hide_delay);
    return () => clearTimeout(timer);
  }, [expanded, dock_auto_hide_delay, isTauri()]);

  // 滚轮循环滚动：仅展开态响应（docs/DOCK.md §2.2），需 preventDefault 故用原生监听
  // Wheel cycling: only when expanded (docs/DOCK.md §2.2); native listener to preventDefault
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!expanded) return;
      e.preventDefault();
      setOffset((o) => o + (e.deltaY > 0 ? 1 : -1));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [expanded]);

  // 渲染条目与槽位：真实插件优先，其次演示条目（名称走 i18n）
  // Render items and slots: real plugins first, then demo items (names via i18n)
  const items: DockItem[] = plugins.length
    ? plugins.map((p) => ({ id: p.id, name: p.name, icon: p.icon }))
    : DEMO_ITEMS.map(({ id, nameKey, icon }) => ({ id, name: t(nameKey), icon }));
  const visibleCount = Math.max(1, Math.min(dock_visible_items, items.length));
  const positions = useMemo(
    () => calculateDockPositions(visibleCount, offset, items.length),
    [visibleCount, offset, items.length]
  );
  const duration = SPEED_SECONDS[dock_animation_speed];

  // 点击唤醒：唯一职责是转发给 Rust（docs/DOCK.md §5）
  // Click-to-wake: the only job is forwarding to Rust (docs/DOCK.md §5)
  const handleDockClick = async (pluginId: string) => {
    if (!isTauri()) return;
    try {
      await invoke("launch_plugin", { pluginId });
    } catch (e) {
      console.error("launch_plugin 失败 / failed:", e);
    }
  };

  // 浏览器预览用的展开入口（Tauri 下由热区轮询触发）
  // Browser-preview expand entry (hotzone polling triggers it under Tauri)
  const previewExpand = () => {
    if (!isTauri()) setExpanded(true);
  };

  return (
    <div
      ref={containerRef}
      className="relative overflow-hidden select-none"
      style={{ width: DOCK_WIDTH, height: DOCK_HEIGHT }}
    >
      {/* 弧形轨道：左半弧虚线（docs/DOCK.md §2.3）*/}
      {/* Arc track: dashed left half arc (docs/DOCK.md §2.3) */}
      <AnimatePresence>
        {expanded && (
          <motion.svg
            key="arc"
            width={DOCK_WIDTH}
            height={DOCK_HEIGHT}
            className="absolute top-0 left-0"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration }}
          >
            <path
              d={`M ${DOCK_WIDTH} 0 A ${DOCK_WIDTH} ${DOCK_HEIGHT / 2} 0 0 0 ${DOCK_WIDTH} ${DOCK_HEIGHT}`}
              fill="none"
              stroke="rgba(255,255,255,0.08)"
              strokeWidth={1}
              strokeDasharray="4 6"
            />
          </motion.svg>
        )}
      </AnimatePresence>

      {/* 插件圆点（仅展开态挂载，AnimatePresence 负责退场）*/}
      {/* Plugin dots (mounted only when expanded; AnimatePresence handles exit) */}
      <AnimatePresence>
        {expanded &&
          positions.map((slot) => {
            const item = items[slot.actualIndex];
            const scale = slot.isActive ? 1.12 : hoveredIndex === slot.actualIndex ? 1.1 : 1;
            return (
              <DockDot
                key={item.id}
                item={item}
                slot={slot}
                scale={scale}
                duration={duration}
                onClick={() => handleDockClick(item.id)}
                onHover={(v) => setHoveredIndex(v ? slot.actualIndex : null)}
              />
            );
          })}
      </AnimatePresence>

      {/* 箭头触发器：12×150 贴右缘垂直居中，hover 展宽 16px（docs/DOCK.md §2.3）*/}
      {/* Arrow trigger: 12x150 at the right edge, widens to 16px on hover */}
      <motion.div
        className="absolute top-1/2 right-0 flex -translate-y-1/2 cursor-pointer items-center justify-center rounded-l-xl bg-zinc-900/70 backdrop-blur-md"
        initial={false}
        animate={{ width: triggerHover || expanded ? 16 : 12, height: 150 }}
        transition={{ duration: 0.16, ease: "easeOut" }}
        onMouseEnter={() => {
          setTriggerHover(true);
          previewExpand();
        }}
        onMouseLeave={() => setTriggerHover(false)}
        onClick={previewExpand}
        title={expanded ? t("dock.collapse") : t("dock.expand")}
      >
        <motion.span
          initial={false}
          animate={{ rotate: expanded ? 180 : 0 }}
          transition={{ duration: 0.12, ease: ARROW_SPRING }}
          className="text-white/80"
        >
          <ChevronLeft className="size-3" />
        </motion.span>
      </motion.div>
    </div>
  );
}
