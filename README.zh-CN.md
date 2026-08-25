# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

一款基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。

---

## ✨ 功能模块

| 模块 | 描述 | 状态 |
| ------ | ----------- | :----: |
| **主面板** | 导航侧边栏（首页 / 插件 / 商店 / 工作流 / 日志 / 设置）+ 内容区域。基于 React 19 + shadcn/ui 构建。 | ✅ 第一阶段 |
| **停靠侧边栏** | 位于屏幕右侧边缘的弧形启动器。**仅用于唤醒插件**（与 Electron 版本一致）。 | 第三阶段 |
| **插件系统** | WASM 沙箱化逻辑（wasmtime）+ Web UI。支持嵌入、独立和无头三种运行模式。 | 第四阶段 |
| **工作流系统** | 基于触发器的 DAG 步骤图，支持跨插件数据流（例如：剪贴板监听 → 美化并保存 → 截图）。 | 第六阶段 |
| **本地数据库** | 通过 `tauri-plugin-sql` 使用 SQLite。Rust 侧统一管理迁移入口。 | ✅ 第一阶段 |

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

```
├── src/                        # 前端源码
│   ├── components/             # UI 组件（导航、shadcn/ui）
│   ├── pages/                  # 页面（仪表盘 / 插件 / 商店 / 工作流 / 日志 / 设置）
│   ├── i18n/                   # 国际化（locales/zh-CN.ts、en-US.ts）
│   ├── lib/                    # db.ts（数据访问）、settings.ts（逻辑）、tauri.ts（桥接）
│   ├── stores/                 # Zustand 状态仓库
│   └── types/                  # 全局 TypeScript 类型定义
│
├── src-tauri/                  # 后端（Rust）
│   ├── migrations/             # SQLite 迁移文件（001_schema.sql、002_defaults.sql）
│   ├── src/                    # Rust 源码（db.rs、迁移组装等）
│   └── tauri.conf.json5        # Tauri 配置（JSON5，内联 capabilities）
│
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
