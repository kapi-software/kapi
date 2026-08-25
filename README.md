# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-orange.svg)](https://tauri.app)
[![React](https://img.shields.io/badge/React-19-61dafb.svg)](https://react.dev)

A Tauri-based plugin-oriented desktop application with unified plugin management and workflow orchestration.

---

## ✨ Features

| Module | Description | Status |
| ------ | ----------- | :----: |
| **Main Panel** | Navigation sidebar (Home / Plugins / Store / Workflow / Logs / Settings) + content area. Built with React 19 + shadcn/ui. | ✅ Done |
| **Dock Sidebar** | Arc-shaped launcher on the right screen edge with hotzone polling. **Wakes plugins only** (dispatch lives in Rust). | ✅ Done |
| **System Tray** | Resident app: close-to-tray, panel / settings / quit menu. | ✅ Done |
| **Plugin System** | WASM-sandboxed logic (wasmtime) + Web UI. `kapi-plugin://` protocol, install/uninstall/enable/mode switch, embedded & independent window hosts, postMessage bridge with a default-deny permission model, Tauri-aligned window customization (transparent / frameless / ...). WASM runtime & plugin SDKs pending. | 🔨 In progress |
| **Workflow System** | Trigger-based DAG step graphs for cross-plugin data flow (e.g., clipboard watch → beautify & save → screenshot). | Phase 6 |
| **Local Database** | SQLite via `tauri-plugin-sql`. Single migration entry point on the Rust side. | ✅ Done |

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

```text
├── src/                        # Frontend source
│   ├── components/
│   │   ├── ui/                 # shadcn/ui primitives
│   │   ├── navigation/         # AppSidebar, TopBar
│   │   └── plugin/             # PluginHost (iframe host)
│   ├── dock/                   # Dock window UI (arc layout)
│   ├── pages/                  # Dashboard / Plugins / PluginEmbedView /
│   │                           # PluginWindowShell / Store / Workflow / Logs / Settings
│   ├── stores/                 # Zustand stores (settings, plugins)
│   ├── i18n/                   # Locales (zh-CN.ts, en-US.ts)
│   ├── lib/                    # db.ts, plugin-bridge.ts, plugin-url.ts, settings.ts, dock-arc.ts, tauri.ts
│   └── types/                  # Global TypeScript types
│
├── src-tauri/                  # Backend (Rust)
│   ├── migrations/             # SQLite migrations (001_init.sql, 002_defaults.sql)
│   ├── src/
│   │   ├── lib.rs              # Builder assembly (plugins, protocol, commands)
│   │   ├── db.rs               # Migration assembly
│   │   ├── dock.rs             # Hotzone polling + window positioning
│   │   ├── tray.rs             # System tray
│   │   ├── plugin_protocol.rs  # kapi-plugin:// protocol (path-safe static serving)
│   │   ├── plugin_bridge.rs    # Bridge command + PermissionGuard (default-deny)
│   │   └── plugin_manager.rs   # Install / uninstall / launch dispatch
│   └── tauri.conf.json5        # Tauri configuration (JSON5, inline capabilities)
│
├── plugins/                    # Sample plugin fixtures (pluginA / pluginB / pluginC)
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
