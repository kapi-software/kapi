---
name: kapi-rust-refactor-2026
description: Rust 端重构进度（2026-08-30）— workflow / wasm / plugin / bridge 拆分为子模块
metadata: 
  node_type: memory
  type: project
  originSessionId: 12b646f7-9871-4fd3-9413-2da23e54f0cc
  modified: 2026-08-30T13:30:00.000Z
---

# Kapi Rust 端重构进度（2026-08-30）

## 完成项

### 拆分原则
- 每个新文件 200–400 行；按"职责 + 复用单元"切
- 测试统一拆到 `kapi/src-tauri/tests/`（crate 根目录），与源文件**分离**
- 测试访问 `pub` API，不再依赖 `pub(crate)`（integration test 是独立 crate）
- 注释两行制（中英文），不用文件头注释块

### 完成的拆分

| 原文件 | 行数 | 拆分后位置 | 拆出 tests |
|--------|------|------------|------------|
| `src/workflow_engine.rs` | 1508 | `src/workflow/{mod,model,topo,trigger,node,engine,db,commands}.rs` | `tests/workflow_topo.rs`（5 个）<br>`tests/workflow_node_input.rs`（3 个） |
| `src/wasm_runtime.rs` | 990 | `src/wasm/{mod,abi,limits,engine,linker,run}.rs` | `tests/wasm_abi.rs`（10 个） |
| `src/plugin_manager.rs` | 1123 | `src/plugin/{mod,manifest,resolve,install,launch,pool}.rs` | `tests/plugin_resolve.rs`（15 个）<br>`tests/plugin_install.rs`（3 个）<br>`tests/plugin_pool.rs`（1 个） |
| `src/plugin_bridge.rs` | 981 | `src/bridge/{mod,types,validate,dispatch,event_bus,log}.rs` | `tests/bridge.rs`（14 个） |

### 关键决策

1. **触发器 starter 放在 `workflow::engine` 而非 `workflow::trigger`**：避免 `trigger↔engine` 循环依赖。`trigger.rs` 只剩 `TriggerEntry` + `TriggerHandle`。
2. **`open_pool_with_migrations` 移到 `workflow::db`**：因为 setup 时创建工作流引擎要用到，与 trigger CRUD 同源。
3. **Wasm 模块的 `MemLimiter`/`InvokeLimits`/`CachedModule` 等改为 `pub`**：因为 integration tests 在 `tests/` 目录需要访问。
4. **plugin 的 `plugin_window_label`/`pick_headless_action` 改为 `pub`**：同上。
5. **bridge 测试集中到一个 `tests/bridge.rs`**：14 个测试覆盖 dispatch + validate + event_bus 全场景。

### 修改的 import 路径
- `crate::plugin_manager::*` → `crate::plugin::*`
- `crate::plugin_bridge::*` → `crate::bridge::*`（进一步细分到 `bridge::log::write_system_log`、`bridge::dispatch::PermissionGuard`）
- `crate::wasm_runtime::*` → `crate::wasm::engine::WasmRuntime` 等
- `crate::workflow_engine::*` → `crate::workflow::*`

## 测试结果（全部通过）

| 来源 | 数量 | 状态 |
|------|------|------|
| `cargo test --lib` | 37 个 | ✅（含 store / plugin_protocol / wasm 等其他模块保留的 `#[cfg(test)]`） |
| `tests/workflow_topo.rs` | 5 | ✅ |
| `tests/workflow_node_input.rs` | 3 | ✅ |
| `tests/wasm_abi.rs` | 10 | ✅ |
| `tests/bridge.rs` | 14 | ✅ |
| `tests/plugin_resolve.rs` | 15 | ✅ |
| `tests/plugin_install.rs` | 3 | ✅ |
| `tests/plugin_pool.rs` | 1 | ✅ |
| **合计** | **88** | **0 failed** |

`cargo build` 成功，零编译警告。

## 重要修复

### 工作流触发器启动失败（dev 运行发现）
错误信息：`failed to start triggers: StorageError: error returned from database: (code: 1) no such table: workflow_triggers`

**原因**：`workflow::db::open_pool_with_migrations` 函数名暗示跑迁移，但只 `connect` 了池，没有 `db::migrations()` apply。

**修复**：在 connect 后遍历 `crate::db::migrations()` 并 `sqlx::raw_sql(...).execute(&pool)`。位置：`src/workflow/db.rs:31-37`。

**Why**: integration test 用的是测试数据库（memory），但 production 路径通过 `app_config_dir/kapi.db` 打开另一份连接，必须自己跑迁移。

**How to apply**: 任何需要 direct SQLite 连接但 `plugin-sql` 还未 load 的路径，都需要先 `db::migrations()`。

## 当前 git 状态

- 工作目录: `d:\code\kapi-lab`
- 分支: `feature/workflow-engine`（未提交到 git——当前目录不是 git 仓库）
- 修改的 Rust 源文件：
  - `kapi/src-tauri/src/lib.rs`（装配 4 个新模块）
  - `kapi/src-tauri/src/workflow/{mod,model,topo,trigger,node,engine,db,commands}.rs`（新增 8 个）
  - `kapi/src-tauri/src/wasm/{mod,abi,limits,engine,linker,run}.rs`（新增 6 个）
  - `kapi/src-tauri/src/plugin/{mod,manifest,resolve,install,launch,pool}.rs`（新增 6 个）
  - `kapi/src-tauri/src/bridge/{mod,types,validate,dispatch,event_bus,log}.rs`（新增 6 个）
  - `kapi/src-tauri/tests/{workflow_topo,workflow_node_input,wasm_abi,bridge,plugin_resolve,plugin_install,plugin_pool}.rs`（新增 7 个）
  - 旧文件 `workflow_engine.rs` / `wasm_runtime.rs` / `plugin_manager.rs` / `plugin_bridge.rs` 已删除

## 下次接手注意

1. **重启 dev server 验证触发器启动**：`pnpm tauri dev`，打开 settings 页，添加一个 schedule 触发器，看是否还报"no such table"。
2. **如果还报错**：检查 `cargo clean && cargo build` 一次（因为之前 `cargo test` 全量触发过 rustc 栈溢出 STATUS_STACK_BUFFER_OVERRUN，单个 `--test` 跑没问题）。
3. **可能存在的潜在问题**：
   - `bridge::dispatch` 的 `pub fn` 列表可能需要 `pub fn dispatch_channel` 显式导出（之前 grep 看到 4 行有 `dispatch_channel`）
   - `wasm::linker.rs` 的 `add_plugin_import` 在原文件里叫别的名字（agent 报告里不确定）
   - `open_pool_with_migrations` 现在在每次 `setup` 都跑迁移——可优化为 `if NOT EXISTS` 探测，但性能可接受
4. **测试运行方式**：由于 `cargo test`（全量）会触发 rustc 栈溢出，建议用 `cargo test --test <name>` 逐个跑，或 `cargo test --lib` 跑单元测试。
5. **没动前端**：用户最初提到"前端的一些方法也是，单元测试和代码在一起"，但本次只重构了 Rust 端。前端重构留待下次。

**Why**: 项目正在 feature/workflow-engine 分支进行大重构，今日完成 Rust 端全部分文件，需要为明天接手保存完整进度。

**How to apply**: 明天打开 `d:\code\kapi-lab` 即可看到现状，先 `pnpm tauri dev` 验证触发器修复是否生效；若还有问题，从「下次接手注意」第 1-3 条开始排查。

---

## 对话过程（关键决策节点）

### 1. 起点（用户）

- 用户希望重构 Rust 端：单文件 1000+ 行、单元测试和代码混在一起
- 提到前端也有类似问题，但本次先 Rust 端

### 2. 初次方案

- 提出 4 个大文件 → 各 5-7 个子模块
- 询问两个细节：
  1. 测试是否拆到独立 `tests/` 目录
  2. 拆分粒度

### 3. 用户决定

- "拆成独立 tests/ 目录"——决定了后续所有测试在 `kapi/src-tauri/tests/`（crate 根），与源文件分离

### 4. 执行过程

- **第 1 批**（我手动执行）：`workflow/` 子模块拆分
  - 遇到循环依赖问题：`trigger.rs` 引用 `engine::from_app` → 解决：starter（`start_schedule_trigger` / `start_plugin_event_trigger`）内联回 `engine.rs`，`trigger.rs` 只剩 `TriggerEntry + TriggerHandle`
  - 5 个编译错误：缺 `DataBinding` import / `Manager` trait / `sqlite_pool().await` / `Option<String>` 类型标注 / 未用 import
  - 都修复后 `cargo check --lib` 通过
  - 单独跑 `cargo test --test workflow_topo` 5 个测试通过

- **第 2 批**（3 个 agent 并行）：`wasm/` / `plugin/` / `bridge/`
  - 用户催"加快进度"——所以并行
  - 三个 agent 都完成、修改了导入路径
  - 合并后 `cargo check --lib` 5 个 warning（未用 import/常量）
  - 清理后零 warning

### 5. 测试迁移

- `src/<module>/tests/*.rs` 不会被 Rust 编译为 integration test
- 移到 `kapi/src-tauri/tests/*.rs`（crate 根目录）
- 这样能引用 `tauri_app_lib::xxx::...` 公开 API
- 把 `pub(crate)` 改为 `pub`（integration test 是独立 crate）

### 6. 测试结果

| 批次 | 测试文件 | 数量 |
|------|---------|------|
| lib 单元 | `cargo test --lib` | 37 ✅ |
| workflow | `workflow_topo.rs` / `workflow_node_input.rs` | 5 + 3 ✅ |
| wasm | `wasm_abi.rs` | 10 ✅ |
| plugin | `plugin_resolve.rs` / `plugin_install.rs` / `plugin_pool.rs` | 15 + 3 + 1 ✅ |
| bridge | `bridge.rs` | 14 ✅ |
| **合计** | | **88** |

注意：`cargo test`（全量）会触发 rustc 栈溢出（STATUS_STACK_BUFFER_OVERRUN），需要逐个 `--test` 跑或 `cargo test --lib`。

### 7. Dev 真实运行发现

- 用户启动了 `pnpm tauri dev` 跑起来
- 报错：`failed to start triggers: StorageError: error returned from database: (code: 1) no such table: workflow_triggers`
- 原因：`workflow::db::open_pool_with_migrations` 名字暗示跑迁移但只 connect
- 修复：connect 后遍历 `crate::db::migrations()` 并 `raw_sql().execute()` 跑迁移
- 修复后 `cargo check --lib` 通过

### 8. 保存

- 用户要求保存到 Kapi 根目录：`kapi/REFACTOR_PROGRESS_2026-08-30.md`
- 同步更新 memory 索引文件
- 用户选择离开，本对话结束，明天继续

---

## 与用户的几个关键互动

1. **拆分粒度** → 用户选"独立 tests/ 目录"（更彻底）
2. **前端重构** → 用户本次只要求 Rust 端，前端留到下次
3. **加快进度** → 用 3 个 agent 并行处理 wasm / plugin / bridge
4. **保存对话** → 用户希望保留完整对话过程以便明天继续

**Why**: 完整对话过程记录能让明天直接接续，不需要重读大量代码。

**How to apply**: 明天打开本文档，先看「下次接手注意」开始排查；如需回顾决策，看「对话过程」。

---

## 2026-09-03 续：迁移根因修复 + 顶栏高度修复

### 迁移问题三层排查（最终根治）

1. **第一次尝试**（盲目 raw_sql 重跑）→ `table plugins already exists`
   - 教训：不能用 raw_sql 重放已落库的迁移
2. **第二次尝试**（构造与 plugin-sql 相同的 Migrator 共用 `_sqlx_migrations`）→ `migration 3: Safety level may not be changed inside a transaction`
   - 发现：sqlx-sqlite 的 `apply()` 无条件包事务，忽略 `no_tx` 标志
3. **根因确认**：`%APPDATA%/com.kapi.app/kapi.db` 的 `_sqlx_migrations` 只有 v1、v2；
   `workflow_triggers` 表不存在 → **003_wal.sql 的 PRAGMA 从未成功应用**（前端
   plugin-sql 路径同样一直在 v3 失败，错误被吞）。这是重构前就存在的 bug。

### 最终修复

- `workflow/db.rs::open_pool_with_migrations`：改用 `SqliteConnectOptions` +
  `journal_mode(Wal)` + `synchronous(Normal)`（连接时设置，持久化进 DB 文件）
- `migrations/003_wal.sql`：改为无操作占位（注释说明根因 + `SELECT 1;`）
- `workflow/db.rs::apply_migrations`：与 plugin-sql 完全一致的 Migrator 构造
  （`SqlxMigration::new`，checksum=sha384(sql)），共用版本表互相跳过

### 验证结果（全部通过）

- 应用启动零报错；`_sqlx_migrations` v1–v4 全部 success=1
- `workflow_triggers` 表已创建；`journal_mode: wal` 生效

### 顶栏高度修复

- 根因：PanelLayout 在 SidebarProvider 上设了 `--header-height: 2.25rem`，
  StandaloneLayout 没设 → TopBar 的 `h-(--header-height)` 高度塌陷
- 修复：TopBar.tsx 导出 `HEADER_HEIGHT = "2.25rem"`，两个布局共用常量

### 下次可做

- 视觉确认两种布局顶栏高度一致（dev server 已在跑，Vite 热更新已生效）
- 前端拆分重构（用户最初提过"前端方法+测试在一起"，尚未动）
- 把本次改动提交 git（当前均未提交）


报错

thread 'main' (11284) panicked at C:\Users\Administrator\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\tauri-2.11.5\src\app.rs:1425:11:
Failed to setup app: error encountered during setup hook: MigrationError: error returned from database: (code: 1) table plugins already exists
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
[0830/231515.513:ERROR:ui\gfx\win\window_impl.cc:172] Failed to unregister class Chrome_WidgetWin_0. Error = 1412
error: process didn't exit successfully: `target\debug\tauri-app.exe` (exit code: 101)
[ELIFECYCLE] Command failed with exit code 101.
