# Kapi

[English](README.md) | [简体中文](README.zh-CN.md)

A Tauri-based plugin-oriented desktop application providing unified plugin management and workflow orchestration.

## Features

| Module | Description | Status |
| ------ | ----------- | ------ |
| **Main Panel** | Nav sidebar (Home / Plugins / Store / Workflow / Logs / Settings) + content area, React 19 + shadcn/ui | Phase 1 ✅ |
| **Dock Sidebar** | Arc-shaped launcher at the right screen edge; **wakes plugins only** (aligned with the Electron version) | Phase 3 |
| **Plugin System** | WASM sandboxed logic (wasmtime) + web UI; embedded / independent / headless run modes | Phase 4 |
| **Workflow System** | Triggers + DAG step graphs linking data across plugins (e.g. clipboard watch → beautify & save → screenshot) | Phase 6 |
| **Local Database** | SQLite (tauri-plugin-sql) with a single Rust-side migration entry point | Phase 1 ✅ |

## Tech Stack

- **Framework**: Tauri v2 + Rust
- **Frontend**: React 19 + TypeScript + Vite
- **Styling**: Tailwind CSS v4 + shadcn/ui
- **State**: Zustand · **Routing**: React Router v7 · **Animation**: motion
- **i18n**: react-i18next (zh-CN / en-US, TS locale modules)
- **Testing**: Vitest
- **Database**: SQLite (tauri-plugin-sql, migrations in `src-tauri/migrations/`)

## Getting Started

```bash
# Install dependencies
pnpm install

# Dev mode (full Tauri app)
pnpm tauri dev

# Frontend only (browser preview, no Tauri IPC)
pnpm dev

# Unit tests
pnpm test

# Frontend build check
pnpm build

# Release build
pnpm tauri build
```

Prerequisites: Node 18+, pnpm, Rust stable, Windows/macOS/Linux (see [Tauri prerequisites](https://tauri.app/start/prerequisites/)).

## Project Structure

```text
├── src/                        # Frontend
│   ├── components/             #   Components (navigation / ui(shadcn))
│   ├── pages/                  #   Pages (Dashboard / Plugins / Store / Workflow / Logs / Settings)
│   ├── i18n/                   #   i18n (locales/zh-CN.ts · en-US.ts)
│   ├── lib/                    #   db.ts access layer · settings.ts pure logic · tauri.ts bridge
│   ├── stores/                 #   Zustand stores
│   └── types/                  #   Global types
├── src-tauri/
│   ├── migrations/             # SQLite migrations (001 schema · 002 defaults)
│   ├── src/                    # Rust (db.rs migration assembly, etc.)
│   └── tauri.conf.json5        # Tauri config (JSON5, inline capabilities)
└── docs/                       # Design docs (Chinese)
```

## Documentation

Design docs (in Chinese) live under [`docs/`](docs/README.md):

- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — architecture, window model, plugin launch flow, technical implementation
- [docs/DATABASE.md](docs/DATABASE.md) — SQLite schema (8 tables), migrations, access layer
- [docs/PANEL.md](docs/PANEL.md) · [docs/DOCK.md](docs/DOCK.md) · [docs/PLUGINS.md](docs/PLUGINS.md) · [docs/WORKFLOW.md](docs/WORKFLOW.md) — module designs
- [docs/ROADMAP.md](docs/ROADMAP.md) — milestones, task checklist, risks

Development standards: [CLAUDE.md](CLAUDE.md)

## License

Private project
