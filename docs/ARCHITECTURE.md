# 架构设计

Kapi 技术文档：项目概述、应用架构、技术实现。

## 1. 项目概述

### 1.1 项目简介

**Kapi** 是一款基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。

应用由两类界面组成：

- **主面板**：应用主界面，左侧为功能导航（设置、插件市场、工作流等），右侧为页面内容。插件默认在主面板内嵌显示。
- **Dock 侧边栏**：屏幕右缘的弧形快速启动栏，鼠标悬停展开。**Dock 只负责唤醒插件**（按插件配置分发到主面板内嵌 / 独立窗口 / 无界面执行），本身不承载任何插件逻辑。

插件支持三种运行模式（内嵌 / 独立窗口 / 无界面），每个插件都可以拥有自定义的独立运行窗口；插件之间通过工作流系统实现数据联动（如剪贴板监听 → 代码美化保存 → 生成截图）。所有数据持久化存储在本地 SQLite 数据库中。

### 1.2 核心功能

| 功能模块 | 说明 |
| ------ | ------ |
| **主面板** | 左侧导航（首页/插件/插件市场/工作流/日志/设置）+ 右侧内容区 |
| **Dock 侧边栏** | 弧形布局的插件快速启动栏，仅负责唤醒，鼠标悬停展开 |
| **插件系统** | WASM 沙箱逻辑 + Web UI，支持 embedded / independent / headless 三种模式 |
| **工作流系统** | 插件间数据联动与自动化编排（触发器 + DAG 步骤图） |
| **本地数据库** | SQLite 存储所有应用数据 |

### 1.3 技术栈

| 层级 | 技术选型 |
| ------ | -------- |
| 应用框架 | Tauri v2 |
| 前端框架 | React 19 + TypeScript |
| 样式方案 | Tailwind CSS + shadcn/ui |
| 状态管理 | Zustand |
| 路由 | React Router v7（v6 API 兼容） |
| Dock 动画 | motion (framer-motion)，对齐 Electron 版 Dock 实现 |
| i18n | react-i18next（zh-CN / en-US） |
| 数据库 | SQLite（tauri-plugin-sql） |
| WASM 运行时 | wasmtime（Rust 侧，最新稳定版） |
| 构建工具 | Vite |

**Tauri 插件清单**：

| 插件 | 用途 |
| ------ | ------ |
| `tauri-plugin-sql` | SQLite 访问与迁移 |
| `tauri-plugin-clipboard-manager` | 剪贴板监听（工作流触发器）+ 插件剪贴板权限 |
| `tauri-plugin-global-shortcut` | 全局快捷键（Alt+Space 唤醒 Dock、工作流 hotkey 触发器） |
| `tauri-plugin-autostart` | 开机自启动设置项 |
| `tauri-plugin-opener` | 打开外部链接（插件市场仓库页等） |

## 2. 应用架构

### 2.1 整体架构图

```text
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Kapi 应用                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                 主窗口 main (Main Window)                            │   │
│  │  ┌──────────────┐  ┌────────────────────────────────────────────┐ │   │
│  │  │  侧边栏导航   │  │           内容区域 (React Router)          │ │   │
│  │  │              │  │                                            │ │   │
│  │  │  🏠 首页     │  │   - 仪表盘 / 插件管理 / 插件市场          │ │   │
│  │  │  🧩 插件     │  │   - 工作流列表 / 工作流编辑器              │ │   │
│  │  │  📦 插件市场  │  │   - 日志 / 设置                          │ │   │
│  │  │  🔄 工作流   │  │   - 插件内嵌视图 (PluginHost + iframe)    │ │   │
│  │  │  📊 日志     │  │     路由 /plugin/:id                      │ │   │
│  │  │  ⚙️ 设置     │  │                                            │ │   │
│  │  └──────────────┘  └────────────────────────────────────────────┘ │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │              Dock 窗口 dock (独立透明窗口, 仅唤醒)                   │   │
│  │   弧形布局 ← 点击图标 → invoke launch_plugin → Rust 按模式分发      │   │
│  │   不渲染任何插件内容，不含插件逻辑                                   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │              插件窗口 plugin-<id> (可选独立实例)                    │   │
│  │   加载应用路由 /plugin-window/:id (裸 PluginHost 壳)                │   │
│  │   窗口尺寸/标题/可缩放等由插件 manifest 自定义                      │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                     Rust 核心 (src-tauri)                           │   │
│  │   dock_service │ plugin_manager │ wasm_runtime │ workflow_engine    │   │
│  │   github_client │ plugin_protocol (kapi-plugin://) │ plugin_bridge  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    SQLite 数据库 (本地存储)                          │   │
│  │  plugins │ plugin_data │ workflows │ workflow_runs                  │   │
│  │  workflow_step_logs │ settings │ plugin_events │ system_logs       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 窗口架构

| 窗口 | Label | 说明 | 特性 |
| ---- | ----- | ---- | ---- |
| **主窗口** | `main` | 应用主界面 | 左侧导航 + 右侧内容区，承载设置、市场、工作流、内嵌插件 |
| **Dock 窗口** | `dock` | 快速启动栏 | 弧形布局，透明无边框，始终置顶，不进任务栏，仅负责唤醒 |
| **插件窗口** | `plugin-<id>` | 插件独立实例 | 仅当插件 window_mode = `independent` 时创建，窗口参数取自插件 manifest，可由用户在插件管理页切换模式 |

### 2.3 统一插件启动流程（核心链路）

无论从 Dock 点击、主面板侧边栏点击还是工作流调用，都走同一条链路：

```text
点击图标 (Dock / 主面板)
  └─> invoke('launch_plugin', { pluginId })
        └─> Rust: 读取 plugins 表 window_mode
              ├─ 'embedded'    → emit('plugin:navigate', id) → 主窗口路由 /plugin/:id
              │                  (主窗口若隐藏/最小化则同时 show + set_focus)
              ├─ 'independent' → 创建或聚焦 WebviewWindow('plugin-<id>')
              │                  窗口参数来自 manifest.window (标题/尺寸/可缩放)
              └─ 'headless'    → 直接调用 WASM 入口执行一次动作
                                 (工作流插件，仅数据处理，无 UI)
```

要点：

- **Dock 不做模式判断**，只发 `launch_plugin`，分发逻辑集中在 Rust 一处。
- 同一插件重复点击 `independent` 模式时**聚焦已有窗口**而非重复创建。
- 用户可在插件管理页修改某插件的 `window_mode`，下次启动即生效。

## 3. 技术实现

### 3.1 项目结构

```text
kapi/
├── src/
│   ├── components/
│   │   ├── ui/                          # shadcn/ui
│   │   ├── navigation/
│   │   │   ├── AppSidebar.tsx           # 主面板侧边栏（shadcn Sidebar）
│   │   │   └── TopBar.tsx
│   │   ├── dock/
│   │   │   ├── DockWindow.tsx           # Dock 前端（弧形布局渲染）
│   │   │   ├── DockItem.tsx
│   │   │   └── useDock.ts
│   │   ├── plugin/
│   │   │   ├── PluginHost.tsx           # 统一插件宿主（iframe + 桥接）
│   │   │   └── PluginCard.tsx
│   │   └── workflow/
│   │       ├── WorkflowCanvas.tsx       # React Flow 画布
│   │       ├── WorkflowNode.tsx
│   │       └── WorkflowRunPanel.tsx     # 运行历史/步骤日志
│   ├── pages/
│   │   ├── Dashboard.tsx
│   │   ├── Plugins.tsx
│   │   ├── Store.tsx
│   │   ├── Workflow.tsx
│   │   ├── PluginEmbedView.tsx          # /plugin/:id（带侧边栏外壳）
│   │   ├── PluginWindowShell.tsx        # /plugin-window/:id（独立窗口裸壳）
│   │   ├── Logs.tsx
│   │   └── Settings.tsx
│   ├── stores/
│   │   ├── settings.ts
│   │   ├── plugins.ts
│   │   └── workflow.ts
│   ├── i18n/                            # react-i18next + TS 语言包
│   ├── lib/
│   │   ├── db.ts                        # 数据库访问层（见 DATABASE.md）
│   │   ├── tauri.ts                     # Tauri 桥接
│   │   └── dock-arc.ts                  # 弧线纯函数计算
│   ├── types/
│   │   └── index.ts
│   ├── App.tsx
│   ├── main.tsx
│   └── routes.tsx
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs                       # Builder 装配：插件注册/迁移/协议/命令
│   │   ├── db.rs                        # 连接与迁移装配
│   │   ├── dock_service.rs              # 热区轮询（边沿触发，见 DOCK.md）
│   │   ├── plugin_manager.rs            # 安装/卸载/窗口创建
│   │   ├── plugin_protocol.rs           # kapi-plugin:// 自定义协议
│   │   ├── plugin_bridge.rs             # 统一权限检查 + 桥接分发
│   │   ├── wasm_runtime.rs              # wasmtime 运行时（见 PLUGINS.md）
│   │   ├── workflow_engine.rs           # DAG 调度引擎（见 WORKFLOW.md）
│   │   └── github_client.rs             # 插件市场来源
│   ├── migrations/
│   │   ├── 001_init.sql
│   │   └── 002_defaults.sql
│   ├── Cargo.toml
│   └── tauri.conf.json5
└── package.json
```

### 3.2 数据库初始化（Rust 侧迁移，唯一入口）

```rust
// src-tauri/src/db.rs
use tauri_plugin_sql::{Migration, MigrationKind};

pub fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "init tables",
            sql: include_str!("../migrations/001_init.sql"),
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "seed default settings",
            sql: include_str!("../migrations/002_defaults.sql"),
            kind: MigrationKind::Up,
        },
    ]
}

// lib.rs 装配（迁移随插件加载自动执行且只执行一次）
// tauri::Builder::default()
//     .plugin(
//         tauri_plugin_sql::Builder::default()
//             .add_migrations("sqlite:kapi.db", db::migrations())
//             .build(),
//     )
```

### 3.3 Tauri 命令（v2 API）

```rust
// src-tauri/src/lib.rs（示意）
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

// 统一插件启动入口（Dock / 主面板 / 快捷键共用）
#[tauri::command]
async fn launch_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
    plugin_id: String,
) -> Result<(), String> {
    let plugin = state.plugin_manager.get(&plugin_id).await?;

    match plugin.window_mode.as_str() {
        // 内嵌：通知主窗口路由切换；主窗口隐藏时先唤起
        "embedded" => {
            if let Some(win) = app.get_webview_window("main") {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
                win.emit("plugin:navigate", &plugin_id).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        // 独立窗口：存在则聚焦，不存在按 manifest.window 创建
        "independent" => {
            let label = format!("plugin-{}", plugin_id);
            if let Some(win) = app.get_webview_window(&label) {
                win.show().map_err(|e| e.to_string())?;
                win.set_focus().map_err(|e| e.to_string())?;
            } else {
                state.plugin_manager.create_window(&app, &plugin).await?;
            }
            Ok(())
        }
        // headless：直接执行一次默认动作（工作流插件）
        _ => state.workflow_engine.execute_plugin_once(&plugin_id).await,
    }
}

// 插件桥接统一入口：权限检查 + 分发（见 PLUGINS.md）
#[tauri::command]
async fn plugin_bridge(
    state: State<'_, AppState>,
    plugin_id: String,
    channel: String,          // 'kapi:storage.get' / 'kapi:plugin.invoke' / ...
    payload: serde_json::Value,
) -> Result<serde_json::Value, String> {
    state.bridge.dispatch(&plugin_id, &channel, payload).await
}
```

### 3.4 自定义协议（插件 UI 资源服务）——已实现

`src-tauri/src/plugin_protocol.rs` 注册 `kapi-plugin` 协议，将插件静态资源映射到安装目录：

```text
kapi-plugin://localhost/<plugin_id>/<path>        (macOS/Linux)
http://kapi-plugin.localhost/<plugin_id>/<path>   (Windows / WebView2)
    →  {app_data}/plugins/<plugin_id>/web/<path>
```

前端统一用 `src/lib/plugin-url.ts` 的 `pluginAssetUrl()` 构造 URL（自动适配平台形式）。插件根路径回退 `index.html`；仅接受 GET 请求。

安全规则（均有单元测试覆盖，`cargo test --lib`）：

1. 百分号解码后再做词法校验：拒绝 `..`（含 `%2e%2e` 编码形式）、反斜杠、非法 plugin_id 字符集（`[A-Za-z0-9._-]`）与非 UTF-8 输入
2. 仅允许 `web/` 子目录，`manifest.json` / `main.wasm` 等逻辑文件不可经协议访问
3. canonicalize 双保险：解析符号链接后必须仍位于该插件 `web/` 根内，防符号链接逃逸
4. 响应带正确 Content-Type；`Cache-Control: no-store` 避免插件更新后缓存旧资源；错误响应固定文案，不回显路径

### 3.5 窗口配置（tauri.conf.json5）

```jsonc
{
  "app": {
    "windows": [
      // 主面板：对齐 Electron panel（1200×1000，无边框自绘标题栏）
      { "label": "main", "title": "Kapi", "width": 1200, "height": 1000, "decorations": false },
      // Dock：完整参数见 DOCK.md；位置由 Rust 启动时按光标所在显示器计算
      {
        "label": "dock",
        "width": 320, "height": 560,
        "transparent": true, "decorations": false, "shadow": false,
        "alwaysOnTop": true, "skipTaskbar": true,
        "visible": false, "resizable": false
      }
    ]
  }
}
```

主面板**关闭 = 隐藏复用**（Rust 拦截 `CloseRequested` → prevent + hide，对齐 Electron 的 close 拦截模式），应用退出流程中放行。
