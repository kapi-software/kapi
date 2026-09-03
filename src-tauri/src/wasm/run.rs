// 单次 guest 执行：硬化检查 → 受限 Store → ABI 往返 → 信封解析 + 宿主导入
// One guest run: hardening checks, a restricted Store, the ABI roundtrip, envelope parsing + host import
use std::time::Duration;

use serde_json::{json, Value};
use wasmtime::{Caller, Engine, Linker, Memory, Module, Store, Trap};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::WasiCtxBuilder;

use crate::bridge::dispatch::{dispatch_channel, PermissionGuard};
use crate::wasm::abi::{pack_result, unpack_result};
use crate::wasm::limits::{MemLimiter, InvokeLimits, MAX_CAPTURE_BYTES, MAX_ENVELOPE_BYTES, MAX_MEMORY_BYTES};

// Store 数据：每次调用新建；宿主导入在此取权限快照 / 连接池 / tokio 句柄
// Store data: fresh per invoke; host imports read the guard snapshot, pool and tokio handle here
// stdout/stderr 管道经 Arc 共享，读端由 run_guest 持有，无需存入 ctx
// stdout/stderr pipes are Arc-shared; run_guest keeps the readers, so ctx holds none
pub struct WasmCallCtx {
    pub plugin_id: String,
    pub guard: PermissionGuard,
    pub pool: sqlx::SqlitePool,
    pub rt: tokio::runtime::Handle,
    pub wasi: WasiP1Ctx,
    pub limiter: MemLimiter,
}

fn stderr_excerpt(pipe: &MemoryOutputPipe) -> String {
    String::from_utf8_lossy(&pipe.contents()).into_owned()
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

// 宿主导入实现：读通道/payload → block_on 共享分发 → 结果信封写回 guest
// Host import: read the channel/payload, dispatch via block_on, write the envelope back
pub fn host_call(caller: &mut Caller<'_, WasmCallCtx>, chan_ptr: i32, chan_len: i32, payload_ptr: i32, payload_len: i32) -> i64 {
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

// 单次 guest 执行：硬化检查 → 受限 Store → ABI 往返 → 信封解析
// One guest run: hardening checks, a restricted Store, the ABI roundtrip, envelope parsing
pub fn run_guest(
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

// epoch ticker：周期递增引擎时钟；deadline 到点即 trap（Interrupt）
// epoch ticker: bumps the engine clock periodically; deadlines trap with Interrupt
pub fn spawn_epoch_ticker(engine: Engine, period: Duration) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || loop {
        std::thread::sleep(period);
        engine.increment_epoch();
    })
}
