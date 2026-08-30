// WASM 运行时：wasmtime 沙箱 + Kapi ABI v1 + 资源限制（docs/PLUGINS.md §5）
// WASM runtime: the wasmtime sandbox, Kapi ABI v1 and resource limits
// 执行模型：同步 Store 放 spawn_blocking；宿主函数经 Handle::block_on 复用桥接分发与权限守卫
// Execution: a sync Store inside spawn_blocking; host functions reuse the bridge
// dispatch and permission guard via Handle::block_on
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};

use serde_json::{json, Value};
use sqlx::Row;
use wasmtime::{Caller, Config, Engine, Linker, Memory, Module, ResourceLimiter, Store, Trap};
use wasmtime_wasi::p1::{add_to_linker_sync, WasiP1Ctx};
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::WasiCtxBuilder;

use crate::plugin_bridge::{dispatch_channel, write_system_log, PermissionGuard};

// 单次调用的资源上限 / per-invoke resource caps
const MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;
const MAX_CAPTURE_BYTES: usize = 64 * 1024;
// 5s 硬超时 = 50 tick × 100ms ticker / the 5s hard timeout = 50 ticks at the 100ms ticker
const EPOCH_TICKER_PERIOD: Duration = Duration::from_millis(100);

// 单次调用的 fuel 与 epoch 预算（可注入便于单测）
// Per-invoke fuel and epoch budgets (injectable for tests)
#[derive(Debug, Clone, Copy)]
pub(crate) struct InvokeLimits {
    pub fuel: u64,
    pub deadline_ticks: u64,
}

impl Default for InvokeLimits {
    fn default() -> Self {
        Self { fuel: 1_000_000_000, deadline_ticks: 50 }
    }
}

// 内存增长限制：线性内存累计不超过 64 MiB
// Memory growth cap: linear memory never exceeds 64 MiB total
struct MemLimiter {
    remaining: i64,
}

impl MemLimiter {
    fn new(max: usize) -> Self {
        Self { remaining: max as i64 }
    }
}

impl ResourceLimiter for MemLimiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let delta = desired as i64 - current as i64;
        if delta > self.remaining {
            wasmtime::bail!("WasmError: memory limit exceeded (64 MiB)");
        }
        self.remaining -= delta;
        Ok(true)
    }

    // 表增长不设限（guest 侧几乎不建表）/ tables are unlimited (guests barely use them)
    fn table_growing(
        &mut self,
        _current: usize,
        _desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        Ok(true)
    }
}

// Store 数据：每次调用新建；宿主导入在此取权限快照 / 连接池 / tokio 句柄
// Store data: fresh per invoke; host imports read the guard snapshot, pool and tokio handle here
// stdout/stderr 管道经 Arc 共享，读端由 run_guest 持有，无需存入 ctx
// stdout/stderr pipes are Arc-shared; run_guest keeps the readers, so ctx holds none
pub struct WasmCallCtx {
    plugin_id: String,
    guard: PermissionGuard,
    pool: sqlx::SqlitePool,
    rt: tokio::runtime::Handle,
    wasi: WasiP1Ctx,
    limiter: MemLimiter,
}

// 编译产物缓存条目：fingerprint（文件长度 + mtime）未变则复用
// Compiled-module cache entry: reused while the (len, mtime) fingerprint is unchanged
struct CachedModule {
    module: Module,
    fingerprint: (u64, SystemTime),
}

// WASM 运行时：Engine + 跨 Store 复用的 Linker + 模块缓存
// WASM runtime: the Engine, a Store-reusable Linker and the module cache
#[derive(Clone)]
pub struct WasmRuntime {
    engine: Engine,
    linker: Arc<Linker<WasmCallCtx>>,
    modules: Arc<Mutex<HashMap<String, CachedModule>>>,
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
    #[cfg(test)]
    pub(crate) fn cached_module_count(&self) -> usize {
        self.lock_modules().len()
    }

    fn lock_modules(&self) -> std::sync::MutexGuard<'_, HashMap<String, CachedModule>> {
        self.modules.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

// Engine + Linker 装配：consume_fuel + epoch_interruption；WASI p1 + kapi 宿主导入
// Engine + Linker assembly: consume_fuel + epoch_interruption; WASI p1 + the kapi host import
fn build_engine_linker() -> Result<(Engine, Linker<WasmCallCtx>), String> {
    let mut config = Config::new();
    // builder 风格配置（48 版返回 &mut Self）/ builder-style config (returns &mut Self in 48)
    config.consume_fuel(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|e| format!("WasmError: {e}"))?;

    let mut linker: Linker<WasmCallCtx> = Linker::new(&engine);
    // WASI preview1：无 preopen / 无网络 / 空环境，stdout/stderr 捕获为内存管道
    // WASI preview1: no preopens, no network, empty env; stdout/stderr captured in memory
    add_to_linker_sync(&mut linker, |ctx: &mut WasmCallCtx| &mut ctx.wasi)
        .map_err(|e| format!("WasmError: {e}"))?;
    // 唯一宿主导入：通道分发与权限守卫与 UI 桥完全共用（dispatch_channel）
    // The single host import: shares dispatch and permissions with the UI bridge
    linker
        .func_wrap(
            "kapi",
            "kapi_host_call",
            |mut caller: Caller<'_, WasmCallCtx>,
             chan_ptr: i32,
             chan_len: i32,
             payload_ptr: i32,
             payload_len: i32|
             -> i64 {
                host_call(&mut caller, chan_ptr, chan_len, payload_ptr, payload_len)
            },
        )
        .map_err(|e| format!("WasmError: {e}"))?;
    Ok((engine, linker))
}

// epoch ticker：周期递增引擎时钟；deadline 到点即 trap（Interrupt）
// epoch ticker: bumps the engine clock periodically; deadlines trap with Interrupt
fn spawn_epoch_ticker(engine: Engine, period: Duration) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || loop {
        std::thread::sleep(period);
        engine.increment_epoch();
    })
}

// ABI v1 打包：ret = ptr << 32 | len / ABI v1 packing: ret = ptr << 32 | len
fn pack_result(ptr: u32, len: u32) -> i64 {
    ((ptr as i64) << 32) | (len as i64 & 0xFFFF_FFFF)
}

fn unpack_result(ret: i64) -> (u32, u32) {
    (((ret >> 32) as u32), (ret as u32))
}

// trap → 稳定错误码（fuel / epoch 特判，其余原样带出）
// trap -> stable error codes (fuel and epoch special-cased)
fn map_trap(e: wasmtime::Error) -> String {
    if let Some(trap) = e.downcast_ref::<Trap>() {
        match trap {
            Trap::OutOfFuel => "WasmError: fuel exhausted".into(),
            Trap::Interrupt => "WasmError: timeout".into(),
            other => format!("WasmError: {other}"),
        }
    } else {
        format!("WasmError: {e}")
    }
}

// 模块缓存：fingerprint 命中直接复用；未命中读文件编译（锁外）后回填
// Module cache: reuse on fingerprint hit; otherwise compile outside the lock and backfill
fn load_module(
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

// 宿主导入实现：读通道/payload → block_on 共享分发 → 结果信封写回 guest
// Host import: read the channel/payload, dispatch via block_on, write the envelope back
fn host_call(caller: &mut Caller<'_, WasmCallCtx>, chan_ptr: i32, chan_len: i32, payload_ptr: i32, payload_len: i32) -> i64 {
    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return 0,
    };
    let mut read_at = |mem: &Memory, ptr: i32, len: i32| -> Option<Vec<u8>> {
        if ptr <= 0 || len < 0 {
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        mem.read(&mut *caller, ptr as usize, &mut buf).ok()?;
        Some(buf)
    };
    let (chan_buf, payload_buf) = match (
        read_at(&memory, chan_ptr, chan_len),
        read_at(&memory, payload_ptr, payload_len),
    ) {
        (Some(c), Some(p)) => (c, p),
        _ => {
            return write_envelope_to_guest(
                caller,
                &json!({ "ok": false, "error": "WasmError: invalid host call arguments" }),
            )
        }
    };

    let channel = String::from_utf8_lossy(&chan_buf).into_owned();
    let payload = match serde_json::from_slice::<Value>(&payload_buf) {
        Ok(v) => v,
        Err(e) => {
            return write_envelope_to_guest(
                caller,
                &json!({ "ok": false, "error": format!("InvalidPayload: {e}") }),
            )
        }
    };

    // 克隆所需件，避免跨 block_on 持有 caller 借用
    // Clone what we need so no caller borrow lives across block_on
    let ctx = caller.data();
    let pool = ctx.pool.clone();
    let guard = ctx.guard.clone();
    let plugin_id = ctx.plugin_id.clone();
    let rt = ctx.rt.clone();

    let result = rt.block_on(dispatch_channel(&pool, &guard, &plugin_id, &channel, payload));
    let envelope = match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(e) => json!({ "ok": false, "error": e }),
    };
    write_envelope_to_guest(caller, &envelope)
}

// 结果信封写回 guest：经其 kapi_alloc 取缓冲并按 ABI 打包；失败返回 0
// Write the envelope back into guest memory via its kapi_alloc; 0 on failure
fn write_envelope_to_guest(caller: &mut Caller<'_, WasmCallCtx>, envelope: &Value) -> i64 {
    let bytes = match serde_json::to_vec(envelope) {
        Ok(b) => b,
        Err(_) => return 0,
    };
    if bytes.len() > MAX_ENVELOPE_BYTES {
        let fallback =
            json!({ "ok": false, "error": format!("WasmError: host result exceeds {MAX_ENVELOPE_BYTES} bytes") });
        let fallback = match serde_json::to_vec(&fallback) {
            Ok(b) => b,
            Err(_) => return 0,
        };
        return write_bytes_to_guest(caller, &fallback);
    }
    write_bytes_to_guest(caller, &bytes)
}

fn write_bytes_to_guest(caller: &mut Caller<'_, WasmCallCtx>, bytes: &[u8]) -> i64 {
    // 嵌套调用 guest 的 kapi_alloc（wasmtime 支持宿主内回调；同受 fuel/epoch 约束）
    // Nested call into the guest's kapi_alloc (supported by wasmtime; still fuel/epoch bound)
    let alloc = match caller.get_export("kapi_alloc").and_then(|e| e.into_func()) {
        Some(f) => f,
        None => return 0,
    };
    let typed = match alloc.typed::<(i32,), i32>(&mut *caller) {
        Ok(t) => t,
        Err(_) => return 0,
    };
    let ptr = match typed.call(&mut *caller, (bytes.len() as i32,)) {
        Ok(p) => p,
        Err(_) => return 0,
    };
    if ptr <= 0 {
        return 0;
    }
    let memory = match caller.get_export("memory").and_then(|e| e.into_memory()) {
        Some(m) => m,
        None => return 0,
    };
    if memory.write(&mut *caller, ptr as usize, bytes).is_err() {
        return 0;
    }
    pack_result(ptr as u32, bytes.len() as u32)
}

fn stderr_excerpt(pipe: &MemoryOutputPipe) -> String {
    String::from_utf8_lossy(&pipe.contents()).into_owned()
}

// 单次 guest 执行：硬化检查 → 受限 Store → ABI 往返 → 信封解析
// One guest run: hardening checks, a restricted Store, the ABI roundtrip, envelope parsing
fn run_guest(
    engine: &Engine,
    linker: &Linker<WasmCallCtx>,
    module: &Module,
    guard: PermissionGuard,
    pool: sqlx::SqlitePool,
    rt: tokio::runtime::Handle,
    plugin_id: &str,
    action: &str,
    payload: &Value,
    limits: &InvokeLimits,
) -> (Result<Value, String>, String) {
    // 硬化：import 模块白名单 / hardening: the import-module whitelist
    for import in module.imports() {
        let module_name = import.module();
        if module_name != "wasi_snapshot_preview1" && module_name != "kapi" {
            return (
                Err(format!("WasmError: unsupported import {module_name}.{}", import.name())),
                String::new(),
            );
        }
    }
    // 硬化：必需导出 / hardening: required exports
    for name in ["memory", "kapi_alloc", "kapi_invoke"] {
        if module.get_export(name).is_none() {
            return (Err(format!("WasmError: missing export {name}")), String::new());
        }
    }

    let stdout = MemoryOutputPipe::new(MAX_CAPTURE_BYTES);
    let stderr = MemoryOutputPipe::new(MAX_CAPTURE_BYTES);
    // 读端克隆：管道移入 ctx 后仍可读取 / reader clone kept after the pipes move into ctx
    let stderr_reader = stderr.clone();
    let wasi = WasiCtxBuilder::new()
        .stdout(stdout.clone())
        .stderr(stderr.clone())
        .build_p1();

    let mut store: Store<WasmCallCtx> = Store::new(
        engine,
        WasmCallCtx {
            plugin_id: plugin_id.to_string(),
            guard,
            pool,
            rt,
            wasi,
            limiter: MemLimiter::new(MAX_MEMORY_BYTES),
        },
    );
    if let Err(e) = store.set_fuel(limits.fuel) {
        return (Err(format!("WasmError: {e}")), String::new());
    }
    store.set_epoch_deadline(limits.deadline_ticks);
    store.limiter(|ctx| &mut ctx.limiter);

    let instantiate = linker.instantiate(&mut store, module);
    let instance = match instantiate {
        Ok(i) => i,
        Err(e) => return (Err(format!("WasmError: {e}")), stderr_excerpt(&stderr_reader)),
    };
    let excerpt = || stderr_excerpt(&stderr_reader);

    let memory = match instance.get_memory(&mut store, "memory") {
        Some(m) => m,
        None => return (Err("WasmError: missing export memory".into()), excerpt()),
    };
    let alloc = match instance.get_typed_func::<(i32,), i32>(&mut store, "kapi_alloc") {
        Ok(f) => f,
        Err(_) => return (Err("WasmError: missing export kapi_alloc".into()), excerpt()),
    };
    let invoke = match instance.get_typed_func::<(i32, i32), i64>(&mut store, "kapi_invoke") {
        Ok(f) => f,
        Err(_) => return (Err("WasmError: missing export kapi_invoke".into()), excerpt()),
    };

    // 请求写入 guest 内存（先 kapi_alloc 取缓冲）
    // Write the request into guest memory (buffer obtained via kapi_alloc)
    let request = json!({ "action": action, "payload": payload });
    let bytes = match serde_json::to_vec(&request) {
        Ok(b) => b,
        Err(e) => return (Err(format!("WasmError: {e}")), excerpt()),
    };
    if bytes.len() > MAX_ENVELOPE_BYTES {
        return (Err(format!("InvalidPayload: request exceeds {MAX_ENVELOPE_BYTES} bytes")), excerpt());
    }
    let ptr = match alloc.call(&mut store, (bytes.len() as i32,)) {
        Ok(p) => p,
        Err(e) => return (Err(map_trap(e)), excerpt()),
    };
    if ptr <= 0 {
        return (Err("WasmError: guest allocation failed".into()), excerpt());
    }
    if let Err(e) = memory.write(&mut store, ptr as usize, &bytes) {
        return (Err(format!("WasmError: {e}")), excerpt());
    }

    // 执行 kapi_invoke 并解包结果信封 / run kapi_invoke and unpack the result envelope
    let ret = match invoke.call(&mut store, (ptr, bytes.len() as i32)) {
        Ok(r) => r,
        Err(e) => return (Err(map_trap(e)), excerpt()),
    };
    let parsed = if ret == 0 {
        Err("WasmError: guest returned null result".to_string())
    } else {
        let (rptr, rlen) = unpack_result(ret);
        let mut buf = vec![0u8; rlen as usize];
        match memory.read(&mut store, rptr as usize, &mut buf) {
            Err(e) => Err(format!("WasmError: {e}")),
            Ok(()) => match serde_json::from_slice::<Value>(&buf) {
                Ok(v) if matches!(v.get("ok"), Some(Value::Bool(true))) => {
                    Ok(v.get("data").cloned().unwrap_or(Value::Null))
                }
                Ok(v) if matches!(v.get("ok"), Some(Value::Bool(false))) => Err(v
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("WasmError: invalid guest result")
                    .to_string()),
                _ => Err("WasmError: invalid guest result".to_string()),
            },
        }
    };
    (parsed, excerpt())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- 测试件 / test fixtures ----

    async fn test_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        // 复用生产建表语句 / reuse the production schema
        sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
            .execute(&pool)
            .await
            .unwrap();
        pool
    }

    async fn insert_plugin(pool: &sqlx::SqlitePool, id: &str, manifest: &str, install_path: &str) {
        sqlx::query(
            "INSERT INTO plugins (id, name, version, manifest, install_path, wasm_path, web_path)
             VALUES (?, 'Demo', '1.0.0', ?, ?, 'main.wasm', 'web/index.html')",
        )
        .bind(id)
        .bind(manifest)
        .bind(install_path)
        .execute(pool)
        .await
        .unwrap();
    }

    // WAT 字符串转义（引号 / 反斜杠）/ escape quotes and backslashes for WAT data strings
    fn wat_escape(s: &str) -> String {
        s.replace('\\', "\\\\").replace('"', "\\\"")
    }

    // 生成"调用一次宿主导入"的 guest 模块 / a guest module that calls the host import once
    fn host_call_wat(channel: &str, payload: &str) -> String {
        let chan_len = channel.len();
        let payload_len = payload.len();
        format!(
            r#"(module
  (import "kapi" "kapi_host_call" (func $host (param i32 i32 i32 i32) (result i64)))
  (memory (export "memory") 1)
  (data (i32.const 1024) "{chan}")
  (data (i32.const 1056) "{payload}")
  (global $heap (mut i32) (i32.const 2048))
  (func $alloc (export "kapi_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.and (i32.add (i32.add (global.get $heap) (local.get $size)) (i32.const 7)) (i32.const -8)))
    (if (i32.gt_u (global.get $heap) (i32.const 65536)) (then (return (i32.const 0))))
    (local.get $ptr))
  (func (export "kapi_invoke") (param $p i32) (param $l i32) (result i64)
    (call $host (i32.const 1024) (i32.const {chan_len}) (i32.const 1056) (i32.const {payload_len}))))"#,
            chan = wat_escape(channel),
            payload = wat_escape(payload),
        )
    }

    // 回环模块：结果信封 = {"ok":true,"data":<请求原文>}（头 18 字节 + 请求 + '}'）
    // Echo module: the envelope wraps the raw request (18-byte header + request + '}')
    const ECHO_WAT: &str = r#"(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 64))
  (func $alloc (export "kapi_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.and (i32.add (i32.add (global.get $heap) (local.get $size)) (i32.const 7)) (i32.const -8)))
    (if (i32.gt_u (global.get $heap) (i32.const 65536)) (then (return (i32.const 0))))
    (local.get $ptr))
  (data (i32.const 32768) "{\"ok\":true,\"data\":")
  (func (export "kapi_invoke") (param $req_ptr i32) (param $req_len i32) (result i64)
    (local $dst i32) (local $i i32)
    (local.set $dst (call $alloc (i32.add (local.get $req_len) (i32.const 19))))
    (if (i32.eqz (local.get $dst)) (then (return (i64.const 0))))
    (local.set $i (i32.const 0))
    (loop $h
      (if (i32.lt_u (local.get $i) (i32.const 18)) (then
        (i32.store8 (i32.add (local.get $dst) (local.get $i))
          (i32.load8_u (i32.add (i32.const 32768) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $h))))
    (local.set $i (i32.const 0))
    (loop $c
      (if (i32.lt_u (local.get $i) (local.get $req_len)) (then
        (i32.store8 (i32.add (i32.add (local.get $dst) (i32.const 18)) (local.get $i))
          (i32.load8_u (i32.add (local.get $req_ptr) (local.get $i))))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $c))))
    (i32.store8 (i32.add (i32.add (local.get $dst) (i32.const 18)) (local.get $req_len)) (i32.const 125))
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $dst)) (i64.const 32))
      (i64.extend_i32_u (i32.add (local.get $req_len) (i32.const 19))))))"#;

    fn parts() -> (Engine, Arc<Linker<WasmCallCtx>>) {
        let (engine, linker) = build_engine_linker().unwrap();
        (engine, Arc::new(linker))
    }

    // 与生产同路径：spawn_blocking 内执行（宿主函数的 block_on 只在 blocking 线程合法）
    // Same path as production: run inside spawn_blocking (host-fn block_on is legal only there)
    #[allow(clippy::too_many_arguments)]
    async fn run_on_blocking(
        engine: &Engine,
        linker: &Arc<Linker<WasmCallCtx>>,
        module: Module,
        guard: PermissionGuard,
        pool: sqlx::SqlitePool,
        plugin_id: &'static str,
        action: &'static str,
        payload: Value,
        limits: InvokeLimits,
    ) -> (Result<Value, String>, String) {
        let engine = engine.clone();
        let linker = linker.clone();
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            run_guest(&engine, &linker, &module, guard, pool, rt, plugin_id, action, &payload, &limits)
        })
        .await
        .unwrap_or_else(|e| (Err(format!("WasmError: {e}")), String::new()))
    }

    // ---- 回环 / echo roundtrip ----

    #[tokio::test]
    async fn echo_roundtrip_through_abi() {
        let (engine, linker) = parts();
        let module = Module::new(&engine, ECHO_WAT).unwrap();
        let pool = test_pool().await;
        let (result, _) = run_on_blocking(
            &engine, &linker, module, PermissionGuard::default(), pool,
            "com.test.echo", "echo", json!({"n": 1}), InvokeLimits::default(),
        )
        .await;
        // data = 完整请求对象 / data equals the whole request object
        assert_eq!(result.unwrap(), json!({"action": "echo", "payload": {"n": 1}}));
    }

    // ---- fuel / epoch 资源限制 / resource limits ----

    #[tokio::test]
    async fn fuel_exhaustion_traps() {
        let (engine, linker) = parts();
        let wat = r#"(module
  (memory (export "memory") 1)
  (func $alloc (export "kapi_alloc") (param $size i32) (result i32) (local.get $size))
  (func (export "kapi_invoke") (param i32 i32) (result i64)
    (loop $l (br $l))
    (i64.const 0)))"#;
        let module = Module::new(&engine, wat).unwrap();
        let pool = test_pool().await;
        let (result, _) = run_on_blocking(
            &engine, &linker, module, PermissionGuard::default(), pool,
            "com.test.fuel", "x", Value::Null,
            InvokeLimits { fuel: 10_000, deadline_ticks: u64::MAX },
        )
        .await;
        assert_eq!(result.unwrap_err(), "WasmError: fuel exhausted");
    }

    #[tokio::test]
    async fn epoch_deadline_times_out() {
        let (engine, linker) = parts();
        // 循环调用宿主（未知通道快速失败不落库；分配耗尽后 host 返 0，循环继续），靠 epoch 中断
        // Loop over the host import (unknown channel fails fast, no DB; once the bump heap
        // is exhausted the host returns 0 and the loop continues) until the epoch interrupt
        let wat = r#"(module
  (import "kapi" "kapi_host_call" (func $host (param i32 i32 i32 i32) (result i64)))
  (memory (export "memory") 1)
  (data (i32.const 1024) "kapi:x")
  (data (i32.const 1056) "{}")
  (global $heap (mut i32) (i32.const 2048))
  (func $alloc (export "kapi_alloc") (param $size i32) (result i32)
    (local $ptr i32)
    (local.set $ptr (global.get $heap))
    (global.set $heap (i32.and (i32.add (i32.add (global.get $heap) (local.get $size)) (i32.const 7)) (i32.const -8)))
    (if (i32.gt_u (global.get $heap) (i32.const 65536)) (then (return (i32.const 0))))
    (local.get $ptr))
  (func (export "kapi_invoke") (param i32 i32) (result i64)
    (loop $l
      (drop (call $host (i32.const 1024) (i32.const 7) (i32.const 1056) (i32.const 2)))
      (br $l))
    (i64.const 0)))"#;
        let module = Module::new(&engine, wat).unwrap();
        let _ticker = spawn_epoch_ticker(engine.clone(), Duration::from_millis(10));
        let pool = test_pool().await;
        let (result, _) = run_on_blocking(
            &engine, &linker, module, PermissionGuard::default(), pool,
            "com.test.timeout", "x", Value::Null,
            InvokeLimits { fuel: 1_000_000_000, deadline_ticks: 1 },
        )
        .await;
        assert_eq!(result.unwrap_err(), "WasmError: timeout");
    }

    // ---- 宿主导入：权限闸与落库 / host import: permission gate and persistence ----

    #[tokio::test]
    async fn host_call_enforces_permissions_and_persists() {
        let pool = test_pool().await;
        let (engine, linker) = parts();
        let module = Module::new(
            &engine,
            &host_call_wat("kapi:storage.set", r#"{"key":"k","value":1}"#),
        )
        .unwrap();

        // 未声明权限 → PermissionDenied / no permission declared -> denied
        insert_plugin(&pool, "com.test.host", r#"{"id":"com.test.host"}"#, "/tmp/wasm-test").await;
        let (result, _) = run_on_blocking(
            &engine, &linker, module.clone(), PermissionGuard::default(), pool.clone(),
            "com.test.host", "x", Value::Null, InvokeLimits::default(),
        )
        .await;
        assert_eq!(result.unwrap_err(), "PermissionDenied: storage:write");

        // 声明权限 → 成功且 plugin_data 落行 / declared -> succeeds and persists
        let guard = PermissionGuard::from_manifest_json(
            r#"{"permissions":["storage:write"]}"#,
        )
        .unwrap();
        let (result, _) = run_on_blocking(
            &engine, &linker, module, guard, pool.clone(),
            "com.test.host", "x", Value::Null, InvokeLimits::default(),
        )
        .await;
        assert_eq!(result.unwrap(), Value::Null);
        let row: Option<String> = sqlx::query_scalar(
            "SELECT value FROM plugin_data WHERE plugin_id = 'com.test.host' AND key = 'k'",
        )
        .fetch_optional(&pool)
        .await
        .unwrap();
        assert_eq!(row.as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn host_call_logs_to_system_logs() {
        let pool = test_pool().await;
        let (engine, linker) = parts();
        insert_plugin(&pool, "com.test.log", r#"{"id":"com.test.log"}"#, "/tmp/wasm-test").await;
        let module = Module::new(
            &engine,
            &host_call_wat("kapi:log.info", r#"{"message":"from wasm"}"#),
        )
        .unwrap();
        let (result, _) = run_on_blocking(
            &engine, &linker, module, PermissionGuard::default(), pool.clone(),
            "com.test.log", "x", Value::Null, InvokeLimits::default(),
        )
        .await;
        assert_eq!(result.unwrap(), Value::Null);
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM system_logs WHERE source = 'plugin:com.test.log' AND message = 'from wasm'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn host_call_denies_ui_only_and_nested_channels() {
        let pool = test_pool().await;
        let (engine, linker) = parts();
        for (channel, expected_prefix) in [
            ("kapi:window.close", "WindowNotAllowed:"),
            ("kapi:plugin.invoke", "WasmError:"),
        ] {
            let module = Module::new(&engine, &host_call_wat(channel, "{}")).unwrap();
            let (result, _) = run_on_blocking(
                &engine, &linker, module, PermissionGuard::default(), pool.clone(),
                "com.test.deny", "x", Value::Null, InvokeLimits::default(),
            )
            .await;
            assert!(
                result.unwrap_err().starts_with(expected_prefix),
                "{channel} should be denied with {expected_prefix}"
            );
        }
    }

    // ---- guest 异常形态 / guest failure shapes ----

    #[tokio::test]
    async fn guest_failure_shapes() {
        let (engine, linker) = parts();
        let pool = test_pool().await;
        let limits = InvokeLimits::default();

        // 返回 0 = guest 致命错误 / returning 0 means a fatal guest error
        let null_ret = Module::new(
            &engine,
            r#"(module (memory (export "memory") 1)
                (func (export "kapi_alloc") (param i32) (result i32) (i32.const 1024))
                (func (export "kapi_invoke") (param i32 i32) (result i64) (i64.const 0)))"#,
        )
        .unwrap();
        let (r, _) = run_on_blocking(&engine, &linker, null_ret, PermissionGuard::default(), pool.clone(), "t", "x", Value::Null, limits).await;
        assert_eq!(r.unwrap_err(), "WasmError: guest returned null result");

        // 缺 kapi_alloc 导出 / missing kapi_alloc export
        let no_alloc = Module::new(
            &engine,
            r#"(module (memory (export "memory") 1)
                (func (export "kapi_invoke") (param i32 i32) (result i64) (i64.const 0)))"#,
        )
        .unwrap();
        let (r, _) = run_on_blocking(&engine, &linker, no_alloc, PermissionGuard::default(), pool.clone(), "t", "x", Value::Null, limits).await;
        assert_eq!(r.unwrap_err(), "WasmError: missing export kapi_alloc");

        // 非法结果 JSON / invalid result JSON
        let garbage = Module::new(
            &engine,
            r#"(module (memory (export "memory") 1)
                (global $heap (mut i32) (i32.const 64))
                (func $alloc (export "kapi_alloc") (param $size i32) (result i32)
                  (local $ptr i32)
                  (local.set $ptr (global.get $heap))
                  (global.set $heap (i32.add (global.get $heap) (local.get $size)))
                  (local.get $ptr))
                (func (export "kapi_invoke") (param i32 i32) (result i64)
                  (local $dst i32)
                  (local.set $dst (call $alloc (i32.const 4)))
                  (i32.store8 (local.get $dst) (i32.const 97))
                  (i32.store8 (i32.add (local.get $dst) (i32.const 1)) (i32.const 98))
                  (i32.store8 (i32.add (local.get $dst) (i32.const 2)) (i32.const 99))
                  (i32.store8 (i32.add (local.get $dst) (i32.const 3)) (i32.const 100))
                  (i64.or (i64.shl (i64.extend_i32_u (local.get $dst)) (i64.const 32)) (i64.const 4))))"#,
        )
        .unwrap();
        let (r, _) = run_on_blocking(&engine, &linker, garbage, PermissionGuard::default(), pool, "t", "x", Value::Null, limits).await;
        assert_eq!(r.unwrap_err(), "WasmError: invalid guest result");
    }

    // ---- ABI 打包纯函数 / ABI packing pure functions ----

    #[test]
    fn pack_unpack_roundtrip_and_bounds() {
        assert_eq!(unpack_result(pack_result(0x1234_5678, 0x9ABC)), (0x1234_5678, 0x9ABC));
        assert_eq!(unpack_result(pack_result(0, 0)), (0, 0));
        // u32 上界往返无损（注：ptr = u32::MAX 时 i64 符号位被置位，属 ABI 固有边界，
        // 真实 guest 指针远小于该值）/ u32 bounds round-trip losslessly (note: ptr = u32::MAX
        // sets the i64 sign bit — an inherent ABI edge, far beyond real guest pointers)
        assert_eq!(unpack_result(pack_result(u32::MAX, u32::MAX)), (u32::MAX, u32::MAX));
        assert_eq!(pack_result(0x1234, 0x5678), (0x1234i64) << 32 | 0x5678);
    }

    // ---- 提交 fixture 验证 / committed-fixture verification ----

    #[tokio::test]
    async fn plugin_d_fixture_invokes_reverse_and_log() {
        // 直接跑仓库内提交的 main.wasm：防 fixture 与 wasm-src / ABI 漂移
        // Runs the committed main.wasm directly: guards drift between the fixture, wasm-src and the ABI
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/pluginD");
        if !fixture.join("main.wasm").is_file() {
            // fixture 未构建时跳过（构建步骤见 plugins/pluginD/wasm-src/README.md）
            // Skipped until the fixture is built (see plugins/pluginD/wasm-src/README.md)
            return;
        }
        let pool = test_pool().await;
        let runtime = WasmRuntime::new();
        insert_plugin(
            &pool,
            "com.kapi.sample.plugin-d",
            r#"{"id":"com.kapi.sample.plugin-d","permissions":[]}"#,
            fixture.to_str().unwrap(),
        )
        .await;

        // reverse：文本反转 / reverse flips the text
        let out = runtime
            .invoke_action(&pool, "com.kapi.sample.plugin-d", "reverse", &json!({"text": "Hello Kapi"}))
            .await
            .unwrap();
        assert_eq!(out, json!({"text": "ipaK olleH"}));

        // log：经宿主导入写系统日志（UI/WASM 共用分发的端到端证明）
        // log writes through the host import (end-to-end proof of the shared dispatch)
        let out = runtime
            .invoke_action(&pool, "com.kapi.sample.plugin-d", "log", &json!({"text": "fixture"}))
            .await
            .unwrap();
        assert_eq!(out, json!({"logged": true}));
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM system_logs WHERE source = 'plugin:com.kapi.sample.plugin-d' AND message = 'plugin-d: fixture'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count, 1);

        // 未知动作 / unknown action
        let err = runtime
            .invoke_action(&pool, "com.kapi.sample.plugin-d", "nope", &Value::Null)
            .await
            .unwrap_err();
        assert!(err.starts_with("UnknownAction: nope"), "{err}");
    }

    // ---- 模块缓存与 evict / module cache and eviction ----

    #[tokio::test]
    async fn module_cache_reuses_and_evicts() {
        let runtime = WasmRuntime::new();
        let dir = std::env::temp_dir().join(format!("kapi-wasm-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("main.wasm");
        std::fs::write(&file, ECHO_WAT).unwrap();

        let modules = runtime.modules.clone();
        let engine = runtime.engine.clone();
        let m1 = load_module(&engine, &modules, "cache.test", &file).unwrap();
        let m2 = load_module(&engine, &modules, "cache.test", &file).unwrap();
        // fingerprint 命中复用同一缓存 / fingerprint hit reuses the same cached module
        assert_eq!(runtime.cached_module_count(), 1);
        drop((m1, m2));

        runtime.evict("cache.test");
        assert_eq!(runtime.cached_module_count(), 0);

        // 文件变化（长度变）触发重编译 / a file change (length) forces recompilation
        let _ = load_module(&engine, &modules, "cache.test", &file).unwrap();
        std::fs::write(&file, format!("{ECHO_WAT}\n")).unwrap();
        load_module(&engine, &modules, "cache.test", &file).unwrap();
        assert_eq!(runtime.cached_module_count(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
