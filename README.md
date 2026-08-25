
# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A Tauri-based plugin-oriented desktop application with unified plugin management and workflow orchestration.

---

## ✨ Features

| Module | Description | Status |
| ------ | ----------- | :----: |
| **Main Panel** | Navigation sidebar (Home / Plugins / Store / Workflow / Logs / Settings) + content area. Built with React 19 + shadcn/ui. | ✅ Phase 1 |
| **Dock Sidebar** | Arc-shaped launcher on the right screen edge. **Wakes plugins only** (aligned with Electron version). | Phase 3 |
| **Plugin System** | WASM-sandboxed logic (wasmtime) + Web UI. Supports embedded, standalone, and headless run modes. | Phase 4 |
| **Workflow System** | Trigger-based DAG step graphs for cross-plugin data flow (e.g., clipboard watch → beautify & save → screenshot). | Phase 6 |
| **Local Database** | SQLite via `tauri-plugin-sql`. Single migration entry point on the Rust side. | ✅ Phase 1 |

---

## 🧰 Tech Stack

- **Framework**: Tauri v2 + Rust  
- **Frontend**: React 19 + TypeScript + Vite  
- **Styling**: Tailwind CSS v4 + shadcn/ui  
- **State Management**: Zustand  
- **Routing**: React Router v7  
- **Animation**: motion  
- **i18n**: react-i18next (zh-CN / en-US, TypeScript locale modules)  
- **Testing**: Vitest  
- **Database**: SQLite (tauri-plugin-sql, migrations in `src-tauri/migrations/`)

---

## 🚀 Getting Started

```bash
# Install dependencies
pnpm install

# Run in development mode (full Tauri app)
pnpm tauri dev

# Run frontend only (browser preview, no Tauri IPC)
pnpm dev

# Run unit tests
pnpm test

# Build frontend (check only)
pnpm build

# Build release app
pnpm tauri build
```

### Prerequisites

- Node.js 18+  
- pnpm  
- Rust (stable channel)  
- Windows / macOS / Linux (see [Tauri prerequisites](https://tauri.app/start/prerequisites/))

---

## 📁 Project Structure

```
├── src/                        # Frontend source
│   ├── components/             # UI components (navigation, shadcn/ui)
│   ├── pages/                  # Pages (Dashboard / Plugins / Store / Workflow / Logs / Settings)
│   ├── i18n/                   # Internationalization (locales/zh-CN.ts, en-US.ts)
│   ├── lib/                    # db.ts (data access), settings.ts (logic), tauri.ts (bridge)
│   ├── stores/                 # Zustand stores
│   └── types/                  # Global TypeScript types
│
├── src-tauri/                  # Backend (Rust)
│   ├── migrations/             # SQLite migrations (001_schema.sql, 002_defaults.sql)
│   ├── src/                    # Rust source (db.rs, migration assembly, etc.)
│   └── tauri.conf.json5        # Tauri configuration (JSON5, inline capabilities)
│
└── docs/                       # Design documentation (Chinese)
```

---

## 📚 Documentation

Design docs (in Chinese) are available in the [`docs/`](docs/README.md) folder:

- [ARCHITECTURE.md](docs/ARCHITECTURE.md) — System architecture, window model, plugin launch flow, technical implementation  
- [DATABASE.md](docs/DATABASE.md) — SQLite schema (8 tables), migrations, data access layer  
- [PANEL.md](docs/PANEL.md) · [DOCK.md](docs/DOCK.md) · [PLUGINS.md](docs/PLUGINS.md) · [WORKFLOW.md](docs/WORKFLOW.md) — Module-specific designs  
- [ROADMAP.md](docs/ROADMAP.md) — Milestones, task checklist, risk assessment  

For development standards, see [CLAUDE.md](CLAUDE.md).

---

## 📄 License

Released under the [MIT License](LICENSE) · Copyright (c) 2026 Kapi Development Team
