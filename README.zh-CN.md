# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。

## 功能概览

| 模块 | 说明 | 阶段 |
| ---- | ---- | ---- |
| **主面板** | 左侧导航（首页/插件/市场/工作流/日志/设置）+ 右侧内容区，React 19 + shadcn/ui | Phase 1 ✅ |
| **Dock 侧边栏** | 屏幕右缘弧形快速启动栏，**仅负责唤醒插件**（对齐 Electron 版行为） | Phase 3 |
| **插件系统** | WASM 沙箱逻辑（wasmtime）+ Web UI，embedded / independent / headless 三种运行模式 | Phase 4 |
| **工作流系统** | 触发器 + DAG 步骤图，插件间数据联动（如剪贴板监听 → 代码美化保存 → 截图生成） | Phase 6 |
| **本地数据库** | SQLite（tauri-plugin-sql），Rust 侧唯一迁移入口 | Phase 1 ✅ |

## 技术栈

- **框架**：Tauri v2 + Rust
- **前端**：React 19 + TypeScript + Vite
- **样式**：Tailwind CSS v4 + shadcn/ui
- **状态**：Zustand；**路由**：React Router v7；**动画**：motion
- **i18n**：react-i18next（zh-CN / en-US，TS 语言包）
- **测试**：Vitest
- **数据库**：SQLite（tauri-plugin-sql，迁移见 `src-tauri/migrations/`）

## 快速开始

```bash
# 安装依赖
pnpm install

# 开发模式（启动完整 Tauri 应用）
pnpm tauri dev

# 仅前端（浏览器预览，无 Tauri IPC）
pnpm dev

# 单元测试
pnpm test

# 前端构建检查
pnpm build

# 发布构建
pnpm tauri build
```

环境要求：Node 18+、pnpm、Rust stable、Windows/macOS/Linux（详见 [Tauri 环境准备](https://tauri.app/start/prerequisites/)）。

## 项目结构

```text
├── src/                        # 前端
│   ├── components/             #   组件（navigation / ui(shadcn)）
│   ├── pages/                  #   页面（Dashboard / Plugins / Store / Workflow / Logs / Settings）
│   ├── i18n/                   #   国际化（locales/zh-CN.ts · en-US.ts）
│   ├── lib/                    #   db.ts 数据访问层 · settings.ts 纯逻辑 · tauri.ts 桥接
│   ├── stores/                 #   Zustand stores
│   └── types/                  #   全局类型
├── src-tauri/
│   ├── migrations/             # SQLite 迁移（001 建表 · 002 默认设置）
│   ├── src/                    # Rust（db.rs 迁移装配等）
│   └── tauri.conf.json5        # Tauri 配置（JSON5，capability 内联）
└── docs/                       # 设计文档
```

## 文档

设计文档位于 [`docs/`](docs/README.md)：

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) —— 架构、窗口模型、插件启动流程、技术实现
- [docs/DATABASE.md](docs/DATABASE.md) —— SQLite 设计（8 表）、迁移、访问层
- [docs/PANEL.md](docs/PANEL.md) · [docs/DOCK.md](docs/DOCK.md) · [docs/PLUGINS.md](docs/PLUGINS.md) · [docs/WORKFLOW.md](docs/WORKFLOW.md) —— 各模块设计
- [docs/ROADMAP.md](docs/ROADMAP.md) —— 里程碑、任务清单、风险表

开发规范：[CLAUDE.md](CLAUDE.md)

## 许可

私有项目
