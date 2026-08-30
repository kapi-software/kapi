// 插件市场类型与缓存策略（Rust 侧 store.rs 的前端对应物）
// Store types and caching policy (the frontend counterpart of Rust store.rs)

// 市场索引条目（index.json plugins[] 元素；store_list 返回形状）
// One index entry (a plugins[] element of index.json; the store_list return shape)
export interface StoreEntry {
  id: string
  name?: string | null
  version?: string | null
  author?: string | null
  description?: string | null
  category?: string | null
  // 插件独立仓库 owner/name（安装 zipball 来源）
  // The plugin's own repo as owner/name (the zipball source for installation)
  repo: string
  // 仓库内插件目录；缺省 = 仓库根即插件包
  // In-repo plugin dir; a missing value means the repo root is the package
  dir?: string | null
}

// 市场列表缓存优先：打开页面读 SQLite 缓存（Rust store_list(refresh=false)），
// 仅手动「刷新」或首刷无缓存时回源 index.json —— 后期 Cloudflare Worker 只换源 URL
// Cache-first listing: the page reads the SQLite cache (Rust store_list(refresh=false));
// only a manual refresh or an empty cache hits the index source — when the Cloudflare
// Worker lands, only the source URL changes
