// 插件资源 URL 构造：对接 kapi-plugin:// 自定义协议（Rust 侧 plugin_protocol.rs）
// Plugin asset URL builder: targets the kapi-plugin:// custom protocol (Rust plugin_protocol.rs)

// Windows(WebView2) 将自定义协议映射为 http://<scheme>.localhost，其余平台为 <scheme>://localhost
// Windows (WebView2) maps custom protocols to http://<scheme>.localhost; others use <scheme>://localhost
export function pluginAssetUrl(
  pluginId: string,
  path: string = 'index.html',
  isWindows: boolean = detectWindows()
): string {
  // plugin_id 字符集为 [A-Za-z0-9._-]，encodeURIComponent 实际为恒等变换，仅作防御
  // plugin_id charset is [A-Za-z0-9._-]; encodeURIComponent is identity here, kept defensively
  const id = encodeURIComponent(pluginId)
  const cleanPath = path.replace(/^\/+/, '')
  return isWindows
    ? `http://kapi-plugin.localhost/${id}/${cleanPath}`
    : `kapi-plugin://localhost/${id}/${cleanPath}`
}

// 校验来自 URL 查询的 entry：段必须为 [A-Za-z0-9._-]（对齐 Rust is_safe_entry），
// 非法输入（手改地址栏的注入等）一律回退 index.html
// Validate an entry taken from a URL query: segments must match [A-Za-z0-9._-]
// (mirroring Rust is_safe_entry); anything invalid (e.g. injected via a hand-edited
// URL) falls back to index.html
export function safeEntry(entry: string | null | undefined): string {
  if (!entry) return 'index.html'
  const cleaned = entry.replace(/^\/+/, '')
  const ok = cleaned
    .split('/')
    .every((seg) => /^[A-Za-z0-9._-]+$/.test(seg) && seg !== '.' && seg !== '..')
  return ok ? cleaned : 'index.html'
}

// 当前是否运行在 Windows：与 @tauri-apps/api convertFileSrc 的判定方式一致
// Whether we run on Windows: same detection as @tauri-apps/api convertFileSrc
function detectWindows(): boolean {
  return typeof navigator !== 'undefined' && /Windows/i.test(navigator.userAgent)
}
