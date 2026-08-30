# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61dafb.svg)](https://react.dev)

一款基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。

---

## ✨ 功能模块

| 模块 | 描述 | 状态 |
| ------ | ----------- | :----: |
| **主面板** | 导航侧边栏（首页 / 插件 / 商店 / 工作流 / 日志 / 设置）+ 内容区域。基于 React 19 + shadcn/ui 构建。 | ✅ 已完成 |
| **Dock 侧边栏** | 屏幕右缘弧形启动栏，热区轮询触发。**仅负责唤醒插件**（分发逻辑集中在 Rust）。 | ✅ 已完成 |
| **系统托盘** | 驻留运行：关闭即收入托盘，提供主面板 / 设置 / 退出菜单。 | ✅ 已完成 |
| **插件系统** | WASM 沙箱逻辑（wasmtime 48 + WASI p1：ABI v1 / fuel + 5s 超时 + 内存上限）+ Web UI。`kapi-plugin://` 协议、安装/卸载/启停/模式切换、内嵌与独立窗口宿主、postMessage 桥接（UI 与 WASM 共用默认拒绝的权限模型）、Tauri 式窗口定制（透明/无边框等）、headless 启动即执行默认动作。插件 SDK（`@kapi/plugin-sdk`、`kapi-plugin-sdk`）待做。 | 🔨 开发中 |
| **工作流系统** | 基于触发器的 DAG 步骤图，支持跨插件数据流（例如：剪贴板监听 → 美化并保存 → 截图）。**Phase 6 已交付**：Kahn 拓扑调度（`workflow_engine.rs`）、两级日志（`workflow_runs` + `workflow_step_logs`）、手动触发 + plugin 节点（经 `WasmRuntime::invoke_action`）、失败即终止。**Phase 7 已交付**：React Flow 可视化编辑器（`@xyflow/react`），路由 `/workflow/new` 与 `/workflow/:id/edit`，含 palette / canvas / inspector / bindings 四区域；专用历史页 `/workflow/:id/runs`。待做：clipboard / hotkey / schedule / plugin_event 触发器、transform 节点。 | ✅ 已完成 |
| **本地数据库** | 通过 `tauri-plugin-sql` 使用 SQLite。Rust 侧统一管理迁移入口。 | ✅ 已完成 |

---

## 🧰 技术栈

- **框架**：Tauri v2 + Rust
- **前端**：React 19 + TypeScript + Vite
- **样式**：Tailwind CSS v4 + shadcn/ui
- **状态管理**：Zustand
- **路由**：React Router v7
- **动画**：motion
- **国际化**：react-i18next（zh-CN / en-US，TypeScript 语言包模块）
- **测试**：Vitest
- **数据库**：SQLite（tauri-plugin-sql，迁移文件位于 `src-tauri/migrations/`）

---

## 🚀 快速开始

```bash
# 安装依赖
pnpm install

# 开发模式运行（完整 Tauri 应用）
pnpm tauri dev

# 仅运行前端（浏览器预览，无 Tauri IPC）
pnpm dev

# 运行单元测试
pnpm test

# 构建前端（仅检查）
pnpm build

# 构建发布版应用
pnpm tauri build
```

### 环境要求

- Node.js 18+
- pnpm
- Rust（稳定版）
- Windows / macOS / Linux（参见 [Tauri 前置要求](https://tauri.app/start/prerequisites/)）

---

## 📁 项目结构

```text
├── src/                        # 前端源码
│   ├── components/
│   │   ├── ui/                 # shadcn/ui 基础组件
│   │   ├── navigation/         # AppSidebar、TopBar
│   │   └── plugin/             # PluginHost（iframe 插件宿主）
│   ├── dock/                   # Dock 窗口 UI（弧形布局）
│   ├── pages/                  # 仪表盘 / 插件 / 插件内嵌视图 /
│   │                           # 插件独立窗口壳 / 商店 / 工作流 / 日志 / 设置
│   ├── stores/                 # Zustand 状态仓库（settings、plugins）
│   ├── i18n/                   # 语言包（zh-CN.ts、en-US.ts）
│   ├── lib/                    # db.ts、plugin-bridge.ts、plugin-url.ts、settings.ts、dock-arc.ts、tauri.ts
│   └── types/                  # 全局 TypeScript 类型定义
│
├── src-tauri/                  # 后端（Rust）
│   ├── migrations/             # SQLite 迁移文件（001_init.sql、002_defaults.sql）
│   ├── src/
│   │   ├── lib.rs              # Builder 装配（插件注册、协议、命令）
│   │   ├── db.rs               # 迁移组装
│   │   ├── dock.rs             # 热区轮询 + 窗口定位
│   │   ├── tray.rs             # 系统托盘
│   │   ├── plugin_protocol.rs  # kapi-plugin:// 协议（路径安全静态服务）
│   │   ├── plugin_bridge.rs    # 桥接命令 + 权限守卫（默认拒绝）
│   │   └── plugin_manager.rs   # 安装 / 卸载 / 启动分发
│   └── tauri.conf.json5        # Tauri 配置（JSON5，内联 capabilities）
│
├── plugins/                    # 示例插件（pluginA / pluginB / pluginC / pluginD）
└── docs/                       # 设计文档（中文）
```

---

## 📚 文档

设计文档（中文）位于 [`docs/`](docs/README.md) 文件夹：

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) —— 系统架构、窗口模型、插件启动流程、技术实现
- [DATABASE.md](docs/DATABASE.md) —— SQLite 表结构（8 张表）、迁移、数据访问层
- [PANEL.md](docs/PANEL.md) · [DOCK.md](docs/DOCK.md) · [PLUGINS.md](docs/PLUGINS.md) · [WORKFLOW.md](docs/WORKFLOW.md) —— 各模块详细设计
- [ROADMAP.md](docs/ROADMAP.md) —— 里程碑、任务清单、风险评估

开发规范请参阅 [CLAUDE.md](CLAUDE.md)。

---

## 📄 许可证

本项目基于 [MIT 许可证](LICENSE) 开源 · 版权所有 (c) 2026 Kapi Development Team
