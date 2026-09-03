// WASM 运行时主体：Engine + 跨 Store 复用的 Linker + 模块缓存 + invoke_action 入口
// WASM runtime body: Engine, a Store-reusable Linker, module cache and the invoke_action entry
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use serde_json::{json, Value};
use sqlx::Row;
use wasmtime::{Engine, Linker, Module};

use crate::bridge::dispatch::PermissionGuard;
use crate::bridge::log::write_system_log;
use crate::wasm::limits::InvokeLimits;
use crate::wasm::linker::build_engine_linker;
use crate::wasm::run::{run_guest, spawn_epoch_ticker, WasmCallCtx};
use crate::wasm::limits::EPOCH_TICKER_PERIOD;

// 编译产物缓存条目：fingerprint（文件长度 + mtime）未变则复用
// Compiled-module cache entry: reused while the (len, mtime) fingerprint is unchanged
pub struct CachedModule {
    module: Module,
    fingerprint: (u64, SystemTime),
}

// WASM 运行时：Engine + 跨 Store 复用的 Linker + 模块缓存
// WASM runtime: the Engine, a Store-reusable Linker and the module cache
#[derive(Clone)]
pub struct WasmRuntime {
    pub engine: Engine,
    pub linker: Arc<Linker<WasmCallCtx>>,
    pub modules: Arc<Mutex<HashMap<String, CachedModule>>>,
}

impl Default for WasmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl WasmRuntime {
    pub fn new() -> Self {
        let (engine, linker) = build_engine_linker().expect("failed to build the wasm runtime");
        // epoch ticker：进程生命周期常驻；配合 set_epoch_deadline 实现 5s 硬超时
        // epoch ticker: process-lifetime thread; pairs with set_epoch_deadline for the 5s timeout
        spawn_epoch_ticker(engine.clone(), EPOCH_TICKER_PERIOD);
        Self { engine, linker: Arc::new(linker), modules: Arc::new(Mutex::new(HashMap::new())) }
    }

    // Phase 6 工作流引擎的正式入口（亦服务 kapi:plugin.invoke 与 headless 启动）
    // The official entry for the Phase 6 workflow engine (also serves plugin.invoke and headless)
    pub async fn invoke_action(
        &self,
        pool: &sqlx::SqlitePool,
        plugin_id: &str,
        action: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        let row =
            sqlx::query("SELECT manifest, install_path, wasm_path, is_enabled, is_installed FROM plugins WHERE id = ?")
                .bind(plugin_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| format!("StorageError: {e}"))?
                .ok_or_else(|| format!("PluginNotFound: {plugin_id}"))?;

        if row.try_get::<i64, _>("is_installed").map_err(|e| format!("StorageError: {e}"))? == 0 {
            return Err(format!("PluginNotFound: {plugin_id} (uninstalled)"));
        }
        if row.try_get::<i64, _>("is_enabled").map_err(|e| format!("StorageError: {e}"))? == 0 {
            return Err(format!("PluginDisabled: {plugin_id}"));
        }
        let manifest: String = row
            .try_get::<String, _>("manifest")
            .map_err(|e| format!("StorageError: {e}"))?;
        let guard = PermissionGuard::from_manifest_json(&manifest)?;
        let wasm_rel: String = row
            .try_get::<Option<String>, _>("wasm_path")
            .map_err(|e| format!("StorageError: {e}"))?
            .ok_or_else(|| "WasmError: plugin has no wasm entry".to_string())?;
        let install_path: String = row
            .try_get::<String, _>("install_path")
            .map_err(|e| format!("StorageError: {e}"))?;
        let wasm_file = std::path::PathBuf::from(install_path).join(wasm_rel);

        // 移入闭包的所有权件 / owned pieces moved into the closure
        let engine = self.engine.clone();
        let linker = self.linker.clone();
        let modules = self.modules.clone();
        let pool2 = pool.clone();
        let rt = tokio::runtime::Handle::current();
        let plugin_id2 = plugin_id.to_string();
        let action2 = action.to_string();
        let payload2 = payload.clone();

        let started = std::time::Instant::now();
        let outcome = tauri::async_runtime::spawn_blocking(move || {
            let module = load_module(&engine, &modules, &plugin_id2, &wasm_file)?;
            Ok(run_guest(
                &engine,
                &linker,
                &module,
                guard,
                pool2,
                rt,
                &plugin_id2,
                &action2,
                &payload2,
                &InvokeLimits::default(),
            ))
        })
        .await
        .map_err(|e| format!("WasmError: {e}"));

        let (result, stderr) = match outcome {
            Ok(Ok(pair)) => pair,
            Ok(Err(e)) | Err(e) => (Err(e), String::new()),
        };

        // 失败且 guest 有 stderr 输出：摘录写系统日志，便于排查
        // On failure with guest stderr output, log an excerpt for debugging
        if let Err(err) = &result {
            if !stderr.is_empty() {
                let excerpt: String = stderr.chars().take(2048).collect();
                let _ = write_system_log(
                    pool,
                    "error",
                    err,
                    &format!("plugin:{plugin_id}"),
                    Some(json!({ "stderr": excerpt, "elapsedMs": started.elapsed().as_millis() as u64 })),
                )
                .await;
            }
        }
        result
    }

    // 卸载 / 重装时清掉编译缓存（公开给 plugin_manager 调用）
    // Drop the compiled cache on uninstall / reinstall (called by plugin_manager)
    pub fn evict(&self, plugin_id: &str) {
        self.lock_modules().remove(plugin_id);
    }

    // 仅供测试：缓存条目数 / test-only: cached module count
    #[doc(hidden)]
    pub fn cached_module_count(&self) -> usize {
        self.lock_modules().len()
    }

    fn lock_modules(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedModule>> {
        self.modules.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

// 模块缓存：fingerprint 命中直接复用；未命中读文件编译（锁外）后回填
// Module cache: reuse on fingerprint hit; otherwise compile outside the lock and backfill
pub fn load_module(
    engine: &Engine,
    modules: &Arc<Mutex<HashMap<String, CachedModule>>>,
    plugin_id: &str,
    wasm_file: &Path,
) -> Result<Module, String> {
    let meta = std::fs::metadata(wasm_file)
        .map_err(|e| format!("WasmError: cannot read wasm entry ({e})"))?;
    let fingerprint = (
        meta.len(),
        meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
    );

    let cache = || {
        modules
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    };
    if let Some(cached) = cache().get(plugin_id) {
        if cached.fingerprint == fingerprint {
            return Ok(cached.module.clone());
        }
    }

    let bytes = std::fs::read(wasm_file)
        .map_err(|e| format!("WasmError: cannot read wasm entry ({e})"))?;
    // 48 版经 Module::new 编译（wat feature 下也接受 WAT 文本）
    // Compiling via Module::new in 48 (accepts WAT text too with the wat feature)
    let module = Module::new(engine, &bytes).map_err(|e| format!("WasmError: {e}"))?;
    cache().insert(plugin_id.to_string(), CachedModule { module: module.clone(), fingerprint });
    Ok(module)
}
