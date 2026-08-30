// 插件声明形态解析单元测试（对齐 Rust resolve_supported_windows 行为）
// Unit tests for declared-shape resolution (aligned with Rust resolve_supported_windows)
import { describe, it, expect } from 'vitest'
import { supportedModes } from '@/lib/plugin-shapes'

describe('声明形态解析 / supportedModes', () => {
  it('windows[] 双形态应都支持 / windows[] dual shapes both supported', () => {
    const manifest = {
      windows: [
        { mode: 'independent', entry: 'widget.html' },
        { mode: 'embedded', entry: 'index.html' },
      ],
    }
    expect(supportedModes(manifest, true, false)).toEqual(['independent', 'embedded'])
  })

  it('windows[] 仅 embedded 时无 independent / embedded-only windows[] has no independent', () => {
    const manifest = { windows: [{ mode: 'embedded', entry: 'a.html' }] }
    expect(supportedModes(manifest, true, false)).toEqual(['embedded'])
  })

  it('windows[] 重复/非法 mode 取首个且忽略其余 / duplicates keep the first, junk ignored', () => {
    const manifest = {
      windows: [{ mode: 'embedded' }, { mode: 'embedded' }, { mode: 'popup' }],
    }
    expect(supportedModes(manifest, true, false)).toEqual(['embedded'])
  })

  it('legacy window 缺省 embedded / legacy window defaults to embedded', () => {
    expect(supportedModes({ window: { mode: 'embedded' } }, true, false)).toEqual(['embedded'])
    expect(supportedModes({}, true, false)).toEqual(['embedded'])
  })

  it('legacy independent 仅独立形态 / legacy independent is independent-only', () => {
    expect(supportedModes({ window: { mode: 'independent' } }, true, false)).toEqual([
      'independent',
    ])
  })

  it('wasm 存在则支持 headless / a wasm entry adds headless', () => {
    expect(supportedModes({}, false, true)).toEqual(['headless'])
    expect(supportedModes({ windows: [{ mode: 'embedded' }] }, true, true)).toEqual([
      'embedded',
      'headless',
    ])
  })

  it('无 web 时 legacy 窗口形态不成立 / no web entry means no window shape', () => {
    expect(supportedModes({ window: { mode: 'independent' } }, false, false)).toEqual([])
  })

  it('畸形 manifest 一律只看 wasm / malformed manifests yield wasm-only', () => {
    expect(supportedModes(null, true, false)).toEqual([])
    expect(supportedModes('not-an-object', true, true)).toEqual(['headless'])
    // windows 非数组 → 按 legacy 回退（window 缺省 embedded，同 Rust unwrap_or_default）
    // A non-array windows field falls back to legacy (default embedded, like Rust)
    expect(supportedModes({ windows: 'junk' }, true, false)).toEqual(['embedded'])
  })
})
