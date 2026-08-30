// 插件资源 URL 构造单元测试
// Unit tests for the plugin asset URL builder
import { describe, it, expect } from 'vitest'
import { pluginAssetUrl, safeEntry } from '@/lib/plugin-url'

describe('插件资源 URL 构造 / pluginAssetUrl', () => {
  it('Windows 下应使用 http://kapi-plugin.localhost 形式 / Windows uses the http form', () => {
    expect(pluginAssetUrl('com.example.foo', 'index.html', true)).toBe(
      'http://kapi-plugin.localhost/com.example.foo/index.html'
    )
  })

  it('macOS/Linux 下应使用 kapi-plugin://localhost 形式 / macOS/Linux use the scheme form', () => {
    expect(pluginAssetUrl('com.example.foo', 'index.html', false)).toBe(
      'kapi-plugin://localhost/com.example.foo/index.html'
    )
  })

  it('缺省路径应回退 index.html / Default path falls back to index.html', () => {
    expect(pluginAssetUrl('com.foo', undefined, false)).toBe(
      'kapi-plugin://localhost/com.foo/index.html'
    )
  })

  it('应剥离路径开头的斜杠 / Leading slashes are stripped', () => {
    expect(pluginAssetUrl('com.foo', '/assets/app.js', true)).toBe(
      'http://kapi-plugin.localhost/com.foo/assets/app.js'
    )
  })

  it('嵌套资源路径应保持段序 / Nested asset paths keep segment order', () => {
    expect(pluginAssetUrl('com.foo', 'assets/js/app.main.js', false)).toBe(
      'kapi-plugin://localhost/com.foo/assets/js/app.main.js'
    )
  })
})

describe('入口校验 / safeEntry', () => {
  it('空值应回退 index.html / Nullish values fall back to index.html', () => {
    expect(safeEntry(undefined)).toBe('index.html')
    expect(safeEntry(null)).toBe('index.html')
    expect(safeEntry('')).toBe('index.html')
  })

  it('合法 slug 路径应原样通过 / Valid slug paths pass through unchanged', () => {
    expect(safeEntry('index.html')).toBe('index.html')
    expect(safeEntry('full.html')).toBe('full.html')
    expect(safeEntry('modes/mini-2.html')).toBe('modes/mini-2.html')
  })

  it('前导斜杠应被剥离 / Leading slashes are stripped', () => {
    expect(safeEntry('/modes/mini.html')).toBe('modes/mini.html')
  })

  it('路径穿越应回退 index.html / Traversal attempts fall back to index.html', () => {
    expect(safeEntry('../secret.html')).toBe('index.html')
    expect(safeEntry('a/../../etc/passwd')).toBe('index.html')
    expect(safeEntry('a/./b.html')).toBe('index.html')
  })

  it('非法字符应回退 index.html / Invalid characters fall back to index.html', () => {
    expect(safeEntry('a b.html')).toBe('index.html')
    expect(safeEntry('a%2fb.html')).toBe('index.html')
    expect(safeEntry('a:b.html')).toBe('index.html')
    expect(safeEntry('modes//mini.html')).toBe('index.html')
  })
})
