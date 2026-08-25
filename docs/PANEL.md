# 主面板与设置系统

Kapi 技术文档：主面板布局、路由、PluginHost 与设置系统。

## 1. 布局结构

主面板采用**左侧导航 + 右侧内容区**布局，使用 React + TypeScript + Tailwind CSS + shadcn/ui 开发：

```text
┌──────────────────────────────────────────────────────────────────┐
│ ┌────────┐ ┌───────────────────────────────────────────────────┐ │
│ │ Logo    │ │  顶栏 (面包屑 + 全局搜索 + 窗口控制)              │ │
│ ├────────┤ ├───────────────────────────────────────────────────┤ │
│ │ 导航    │ │                                                   │ │
│ │         │ │              内容区域                             │ │
│ │ 🏠 首页 │ │                                                   │ │
│ │ 🧩 插件 │ │        (由 React Router 渲染)                     │ │
│ │ 📦 市场 │ │                                                   │ │
│ │ 🔄 工作流│ │        /plugin/:id 时渲染插件内嵌视图             │ │
│ │ 📊 日志 │ │                                                   │ │
│ │ ⚙️ 设置 │ │                                                   │ │
│ │         │ │                                                   │ │
│ └────────┘ └───────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

侧边栏使用 shadcn/ui `Sidebar` 组件（可折叠），主导航分组：

| 分组 | 项 | 路由 |
| ---- | -- | ---- |
| 概览 | 首页（仪表盘） | `/` |
| 插件 | 插件（已安装管理） | `/plugins` |
| 插件 | 插件市场 | `/store` |
| 自动化 | 工作流 | `/workflow` |
| 系统 | 日志 | `/logs` |
| 系统 | 设置 | `/settings` |

## 2. 页面路由

| 路由 | 页面 | 组件 | 说明 |
| ---- | ---- | ---- | ---- |
| `/` | 首页 | `Dashboard` | 概览：最近使用插件、工作流运行状态、快捷入口 |
| `/plugins` | 插件管理 | `Plugins` | 已安装列表、本地导入、启用/禁用、切换 window_mode、上移/下移排序（写 sort_order）、卸载（两步确认） |
| `/plugin/:id` | 插件内嵌视图 | `PluginEmbedView` | 面板外壳内的 PluginHost（iframe，kapi-plugin:// 协议加载） |
| `/store` | 插件市场 | `StorePage` | GitHub 来源浏览、安装/更新/卸载 |
| `/plugin/:id` | 插件内嵌视图 | `PluginEmbedView` | 主面板内嵌运行插件（§3） |
| `/plugin-window/:id` | 插件独立壳 | `PluginWindowShell` | 独立窗口加载的**裸壳路由**（无侧边栏），内嵌同一 `PluginHost` |
| `/workflow` | 工作流列表 | `WorkflowPage` | 列表、启停、手动运行、运行历史 |
| `/workflow/:id` | 工作流编辑器 | `WorkflowEditor` | React Flow 画布编辑 DAG |
| `/logs` | 日志 | `LogsPage` | system_logs 过滤查看 |
| `/settings` | 设置 | `SettingsPage` | 见 §4 |

## 3. 插件内嵌视图与 PluginHost（核心组件）

**内嵌模式（embedded）与独立窗口模式（independent）共用同一个宿主组件 `PluginHost`**，区别仅在外壳：

- embedded → `PluginEmbedView`（带主面板侧边栏的 `/plugin/:id`）
- independent → `PluginWindowShell`（独立 `WebviewWindow` 加载的 `/plugin-window/:id`，无侧边栏）

`PluginHost` 职责：加载插件 UI（iframe 指向 `kapi-plugin://` 自定义协议，天然隔离）+ 桥接插件与 Rust 之间的 RPC。

```tsx
// src/components/plugin/PluginHost.tsx（示意）
import { useEffect, useRef } from 'react'
import { invoke } from '@tauri-apps/api/core'

export function PluginHost({ pluginId }: { pluginId: string }) {
  const frameRef = useRef<HTMLIFrameElement>(null)

  // 监听 iframe 的 postMessage，转发到 Rust（权限检查在 Rust 侧统一执行）
  useEffect(() => {
    const onMessage = async (e: MessageEvent) => {
      if (e.source !== frameRef.current?.contentWindow) return   // 只信任自家 iframe
      const req = e.data                                        // { id, channel, payload }
      if (typeof req?.channel !== 'string' || !req.channel.startsWith('kapi:')) return

      try {
        const result = await invoke('plugin_bridge', {
          pluginId, channel: req.channel, payload: req.payload,
        })
        e.source!.postMessage({ id: req.id, ok: true, data: result }, '*')
      } catch (err) {
        e.source!.postMessage({ id: req.id, ok: false, error: String(err) }, '*')
      }
    }
    window.addEventListener('message', onMessage)
    return () => window.removeEventListener('message', onMessage)
  }, [pluginId])

  // 插件 UI 通过自定义协议加载
  return (
    <iframe
      ref={frameRef}
      className="h-full w-full border-0"
      src={`kapi-plugin://localhost/${pluginId}/index.html`}
      sandbox="allow-scripts allow-forms"   // 不给 allow-same-origin，保持隔离
    />
  )
}
```

设计要点：

- **为什么不直接 `innerHTML`**：innerHTML 注入的 `<script>` 不执行、资源相对路径不解析、插件与宿主共享上下文零隔离。iframe + 自定义协议解决全部三个问题。
- **为什么独立窗口也走应用路由**：插件 iframe 内没有 Tauri IPC；让独立窗口加载应用自己的 `/plugin-window/:id` 壳页，桥接链路与内嵌完全一致，权限检查只写一处。
- 插件 UI 与插件 WASM 的通信同样经 `PluginHost` → `plugin_bridge` → `wasm_runtime`（见 PLUGINS.md）。

## 4. 设置系统

### 4.1 存储

全部设置统一存 `settings` 表（key-value，value 为 JSON 编码），**Dock 设置以 `dock_` 前缀存于同一张表**，无独立 dock_settings 表。

设置变更实时生效：`dock_enabled` → 通知 Rust 挂起/恢复轮询；`theme`/`accent_color` → 切换 Tailwind `dark` class 与 CSS 变量；`language` → `i18n.changeLanguage` + `html lang`。

### 4.2 设置项清单

与 DATABASE.md §4 种子一一对应：

| 分类 | 设置项 | Key | 类型 / 取值 |
| ---- | ------ | --- | ----------- |
| **通用** | 语言 | `language` | `zh-CN` / `en-US` |
| | 开机启动 | `auto_start` | bool（tauri-plugin-autostart） |
| | 更新检查 | `check_update` | bool |
| **主题** | 主题模式 | `theme` | `light` / `dark` / `system` |
| | 强调色 | `accent_color` | 色值 `#007AFF` |
| **Dock** | 启用 Dock | `dock_enabled` | bool（打开/关闭 Dock 栏） |
| | 热区宽度 | `dock_hotzone_width` | 6–24 px |
| | 动画速度 | `dock_animation_speed` | `slow` / `medium` / `fast` |
| | 展开延迟 | `dock_expand_delay` | ms |
| | 可见数量 | `dock_visible_items` | 5–13 |
| | 位置 | `dock_position` | `right` / `left`（主显示器贴靠边，切换实时重定位） |

> `dock_auto_hide_delay` 已废弃（2026-08-25 需求变更：光标在 Dock 内即保持展开，
> 无延迟自动收起）。settings 表种子保留以兼容，设置 UI 已移除。
> `dock_auto_hide_delay` is deprecated (changed 2026-08-25: no delayed auto-hide).
| **插件** | 自动更新 | `plugin_auto_update` | bool |
| | 沙箱严格模式 | `plugin_sandbox_strict` | bool（true 时未签名插件拒绝加载） |
| | 日志级别 | `plugin_log_level` | `debug` / `info` / `warn` / `error` |

设置页使用 shadcn/ui 表单组件（`Switch` / `Select` / `Slider` / 颜色选择器），按上表分组渲染。
