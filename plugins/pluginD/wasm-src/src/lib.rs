// pluginD WASM 入口：Kapi ABI v1 的内联实现（未来抽为 kapi-plugin-sdk crate）
// pluginD WASM entry: an inline Kapi ABI v1 implementation (future kapi-plugin-sdk crate)
// ABI 见 docs/PLUGINS.md §5：导出 memory / kapi_alloc / kapi_invoke；导入 kapi.kapi_host_call
// ABI per docs/PLUGINS.md §5: exports memory / kapi_alloc / kapi_invoke; imports kapi.kapi_host_call
use std::panic::{AssertUnwindSafe, catch_unwind};

use serde_json::{Value, json};

// 静态 bump 堆：256 KiB，8 字节对齐；宿主与 guest 共用同一线性内存地址空间
// A static bump heap: 256 KiB, 8-byte aligned; the host and guest share linear addresses
 const HEAP_SIZE: usize = 256 * 1024;
 static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
 static HEAP_CURSOR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// 堆基址（线性内存绝对地址）；addr_of! 规避对 static mut 的直接引用
// The heap base as an absolute linear address; addr_of! avoids a direct static-mut reference
fn heap_base() -> usize {
    std::ptr::addr_of!(HEAP) as usize
}

// bump 分配：绝对地址返回；溢出或非法 size 返回 0（ABI 约定）
// Bump allocation returning absolute addresses; 0 on overflow or a bad size (per the ABI)
#[no_mangle]
pub extern "C" fn kapi_alloc(size: i32) -> i32 {
    if size <= 0 {
        return 0;
    }
    let size = size as usize;
    let mut old = HEAP_CURSOR.load(std::sync::atomic::Ordering::Relaxed);
    loop {
        let aligned = old.div_ceil(8) * 8;
        let next = aligned.saturating_add(size);
        if next > HEAP_SIZE {
            return 0;
        }
        match HEAP_CURSOR.compare_exchange_weak(
            old,
            next,
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
        ) {
            Ok(_) => return (heap_base() + aligned) as i32,
            Err(cur) => old = cur,
        }
    }
}

// guest 堆内切片：地址必须落在已分配范围内 / slice within the allocated heap range
unsafe fn heap_slice(ptr: i32, len: i32) -> Option<&'static [u8]> {
    if ptr <= 0 || len < 0 {
        return None;
    }
    let ptr = ptr as usize;
    let len = len as usize;
    let base = heap_base();
    let cursor = HEAP_CURSOR.load(std::sync::atomic::Ordering::Relaxed);
    if ptr < base || ptr.saturating_add(len) > base + cursor {
        return None;
    }
    Some(unsafe { std::slice::from_raw_parts(ptr as *const u8, len) })
}

// 唯一宿主导入：通道分发（kapi:log.info 等），返回打包的 (ptr, len)
// The single host import: channel dispatch (e.g. kapi:log.info), returning packed (ptr, len)
#[link(wasm_import_module = "kapi")]
extern "C" {
    fn kapi_host_call(chan_ptr: i32, chan_len: i32, payload_ptr: i32, payload_len: i32) -> i64;
}

// 调宿主通道：payload 为 JSON 值；结果解包为 data 或错误串
// Call a host channel with a JSON payload; unpack into data or an error string
fn host_call(channel: &str, payload: &Value) -> Result<Value, String> {
    let payload_bytes = serde_json::to_vec(payload).unwrap_or_default();
    let Some((cp, cl)) = write_to_heap(channel.as_bytes()) else {
        return Err("WasmError: guest allocation failed".into());
    };
    let Some((pp, pl)) = write_to_heap(&payload_bytes) else {
        return Err("WasmError: guest allocation failed".into());
    };
    let ret = unsafe { kapi_host_call(cp, cl as i32, pp, pl as i32) };
    if ret == 0 {
        return Err("WasmError: host returned null".into());
    }
    let ptr = (ret >> 32) as u32 as usize;
    let len = (ret & 0xFFFF_FFFF) as u32 as usize;
    let bytes = unsafe { heap_slice(ptr as i32, len as i32) }
        .ok_or_else(|| "WasmError: invalid host result".to_string())?;
    let envelope: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("WasmError: {e}"))?;
    if envelope.get("ok") == Some(&Value::Bool(true)) {
        Ok(envelope.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(envelope
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("WasmError: invalid host result")
            .to_string())
    }
}

// 拷贝进堆并返回 (绝对地址, 长度) / copy into the heap, returning (absolute ptr, len)
fn write_to_heap(bytes: &[u8]) -> Option<(i32, usize)> {
    let ptr = kapi_alloc(bytes.len() as i32);
    if ptr <= 0 {
        return None;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
    }
    Some((ptr, bytes.len()))
}

// 结果信封序列化进堆并按 ABI 打包 / serialize the envelope into the heap and pack per the ABI
fn pack_out(envelope: &Value) -> i64 {
    let bytes = serde_json::to_vec(envelope).unwrap_or_default();
    let Some((ptr, len)) = write_to_heap(&bytes) else {
        return 0;
    };
    ((ptr as i64) << 32) | (len as i64 & 0xFFFF_FFFF)
}

fn ok_envelope(data: Value) -> i64 {
    pack_out(&json!({ "ok": true, "data": data }))
}

fn err_envelope(error: String) -> i64 {
    pack_out(&json!({ "ok": false, "error": error }))
}

// 业务分发：reverse 反转文本；log 经宿主导入写系统日志
// Action dispatch: reverse flips the text; log writes through the host import
fn dispatch(action: &str, payload: &Value) -> i64 {
    match action {
        "reverse" => {
            let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
            let reversed: String = text.chars().rev().collect();
            ok_envelope(json!({ "text": reversed }))
        }
        "log" => {
            let text = payload.get("text").and_then(Value::as_str).unwrap_or("");
            match host_call("kapi:log.info", &json!({ "message": format!("plugin-d: {text}") })) {
                Ok(_) => ok_envelope(json!({ "logged": true })),
                Err(e) => err_envelope(e),
            }
        }
        other => err_envelope(format!("UnknownAction: {other}")),
    }
}

// ABI 入口：解析请求 JSON → 分发 → 打包结果；panic 捕获为错误信封
// The ABI entry: parse the request JSON, dispatch, pack the result; panics become errors
#[no_mangle]
pub extern "C" fn kapi_invoke(req_ptr: i32, req_len: i32) -> i64 {
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let bytes = unsafe { heap_slice(req_ptr, req_len) }.unwrap_or(&[]);
        let request: Value = match serde_json::from_slice(bytes) {
            Ok(v) => v,
            Err(e) => return err_envelope(format!("InvalidPayload: {e}")),
        };
        let action = request.get("action").and_then(Value::as_str).unwrap_or("");
        let payload = request.get("payload").cloned().unwrap_or(Value::Null);
        dispatch(action, &payload)
    }));
    match outcome {
        Ok(ret) => ret,
        Err(e) => err_envelope(format!("WasmPanic: {e:?}")),
    }
}
