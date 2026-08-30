// 插件声明形态解析（UI 展示用）：与 Rust resolve_supported_windows 对齐
// Declared-shape resolution (display-only): mirrors Rust resolve_supported_windows
// 权威裁决仍在 Rust launch_plugin（UnsupportedMode）；此处仅驱动模式下拉的可选项
// The authority stays in Rust launch_plugin (UnsupportedMode); this only feeds the dropdown
import type { WindowMode } from '@/types'

// 解析 manifest → 支持的运行模式集合
// Resolve the manifest -> the set of supported window modes
// windows[] 逐条入位（embedded/independent，首个生效）；无数组时回退 legacy window.mode
// （缺省 embedded）；headless 等价于存在 main.wasm；无 web 入口则无窗口形态
// windows[] entries slot in (embedded/independent, first wins); without the array the
// legacy window.mode (default embedded) applies; headless equals having main.wasm;
// no web entry means no window shapes
export function supportedModes(
  manifest: unknown,
  hasWeb: boolean,
  hasWasm: boolean
): WindowMode[] {
  const modes: WindowMode[] = []
  const push = (m: WindowMode) => {
    if (!modes.includes(m)) modes.push(m)
  }

  if (manifest && typeof manifest === 'object') {
    const windows = (manifest as { windows?: unknown }).windows
    if (Array.isArray(windows)) {
      for (const entry of windows) {
        const mode = (entry as { mode?: unknown } | null)?.mode
        if (mode === 'embedded' || mode === 'independent') push(mode)
      }
    } else {
      // legacy window：单形态声明 / legacy window: a single shape
      const mode = (manifest as { window?: { mode?: unknown } }).window?.mode
      if (!hasWeb) {
        // 无 web 入口：窗口形态不成立（headless 声明不产生窗口形态，同 Rust）
        // No web entry: no window shape at all (a headless declaration yields none)
      } else if (mode === 'independent') {
        push('independent')
      } else {
        // 缺省 embedded（含显式 embedded 与 headless 声明外的回退）
        // Default embedded (explicit embedded, and the fallback otherwise)
        push('embedded')
      }
    }
  }

  if (hasWasm) push('headless')
  return modes
}
