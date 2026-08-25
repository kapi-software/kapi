// 插件资源 URL 构造单元测试
// Unit tests for the plugin asset URL builder
import { describe, it, expect } from 'vitest'
import { pluginAssetUrl } from '@/lib/plugin-url'

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
