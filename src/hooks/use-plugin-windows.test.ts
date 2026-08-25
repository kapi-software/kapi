// 窗口 label 映射单元测试：与 Rust plugin_window_label 保持一致
// Window-label mapping unit tests: must stay in sync with the Rust plugin_window_label
import { describe, it, expect } from 'vitest'
import { pluginWindowLabel } from '@/hooks/use-plugin-windows'

describe('pluginWindowLabel / 窗口 label 映射', () => {
  it('应将反向域名 id 的 "." 映射为 "_" / should map dots in reverse-domain ids to underscores', () => {
    expect(pluginWindowLabel('com.kapi.sample.plugin-c')).toBe(
      'plugin-com_kapi_sample_plugin-c',
    )
  })

  it('无点 id 应原样保留 / dot-free ids pass through unchanged', () => {
    expect(pluginWindowLabel('simple_id-1')).toBe('plugin-simple_id-1')
  })
})
