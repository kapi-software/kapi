# Kapi

基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。
A Tauri-based plugin-oriented desktop application with unified plugin management and workflow orchestration.

## 功能概览 / Features

| 模块 | 说明 | 阶段 |
| ---- | ---- | ---- |
| **主面板** | 左侧导航（首页/插件/市场/工作流/日志/设置）+ 右侧内容区，React 19 + shadcn/ui | Phase 1 ✅ |
| **Dock 侧边栏** | 屏幕右缘弧形快速启动栏，**仅负责唤醒插件**（对齐 Electron 版行为） | Phase 3 |
| **插件系统** | WASM 沙箱逻辑（wasmtime）+ Web UI，embedded / independent / headless 三种运行模式 | Phase 4 |
| **工作流系统** | 触发器 + DAG 步骤图，插件间数据联动（如剪贴板监听 → 代码美化保存 → 截图生成） | Phase 6 |
| **本地数据库** | SQLite（tauri-plugin-sql），Rust 侧唯一迁移入口 | Phase 1 ✅ |

## 技术栈 / Tech Stack

- **框架**：Tauri v2 + Rust
- **前端**：React 19 + TypeScript + Vite
- **样式**：Tailwind CSS v4 + shadcn/ui
- **状态**：Zustand；**路由**：React Router v7；**动画**：motion
- **i18n**：react-i18next（zh-CN / en-US，语言包为 TS 模块）
- **测试**：Vitest
- **数据库**：SQLite（tauri-plugin-sql，迁移见 `src-tauri/migrations/`）

## 快速开始 / Getting Started

```bash
# 安装依赖 / Install dependencies
pnpm install

# 开发模式（启动完整 Tauri 应用）/ Dev mode (full Tauri app)
pnpm tauri dev

# 仅前端（浏览器预览，无 Tauri IPC）/ Frontend only (browser, no Tauri IPC)
pnpm dev

# 单元测试 / Unit tests
pnpm test

# 前端构建检查 / Frontend build check
pnpm build

# 发布构建 / Release build
pnpm tauri build
```

环境要求 / Prerequisites：Node 18+、pnpm、Rust stable、Windows/macOS/Linux（详见 [Tauri 环境准备](https://tauri.app/start/prerequisites/)）。

## 项目结构 / Project Structure

```
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
│   └── tauri.conf.json5        # Tauri 配置（JSON5，见下方说明）
└── docs/plan.MD                # 设计文档（架构 / 数据库 / Dock 规格 / 插件系统 / 开发计划）
```

## ⚠️ 本机环境说明 / Environment Note

本开发机的透明加密软件（DLP）会加密受信进程写入的 `.json` / `.txt` 文件，导致 cargo 构建失败。因此本项目：

- Tauri 配置使用 **`tauri.conf.json5`**（`config-json5` feature 已启用），capability 内联于 `app.security.capabilities`
- i18n 语言包使用 **`.ts` 模块**而非 `.json`
- 新增配置/数据文件请避免 `.json` / `.txt` 扩展名（详见 `CLAUDE.md` §0.1 与 `docs/plan.MD` §10.4）

## 文档 / Docs

- [docs/plan.MD](docs/plan.MD) —— 完整设计文档：应用架构、数据库设计（8 表）、Dock 规格（与 Electron 版源码逐项对齐）、WASM 插件系统、工作流 DAG 引擎、开发计划与风险表
- [CLAUDE.md](CLAUDE.md) —— 开发规范：注释（两行制双语）、测试、Git、文档更新流程

## 许可 / License

私有项目 / Private project
