// WASM ABI 测试：pack/unpack 纯函数 + 回环 / fuel / epoch / 宿主导入 / guest 异常 / 缓存
// WASM ABI tests: pack/unpack pure fns + echo / fuel / epoch / host import / guest failures / cache
// 运行：cargo test -p kapi --test wasm_abi
use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use sqlx::SqlitePool;
use tauri_app_lib::wasm::abi::{pack_result, unpack_result};
use tauri_app_lib::wasm::engine::{load_module, WasmRuntime};
use tauri_app_lib::wasm::limits::InvokeLimits;
use tauri_app_lib::wasm::linker::build_engine_linker;
use tauri_app_lib::wasm::run::{run_guest, spawn_epoch_ticker, WasmCallCtx};
use tauri_app_lib::bridge::dispatch::PermissionGuard;

// ---- 测试件 / test fixtures ----

async fn test_pool() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    // 复用生产建表语句 / reuse the production schema
    sqlx::raw_sql(include_str!("../migrations/001_init.sql"))
        .execute(&pool)
        .await
        .unwrap();
    pool
}

async fn insert_plugin(pool: &SqlitePool, id: &str, manifest: &str, install_path: &str) {
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

fn parts() -> (wasmtime::Engine, Arc<wasmtime::Linker<WasmCallCtx>>) {
    let (engine, linker) = build_engine_linker().unwrap();
    (engine, Arc::new(linker))
}

// 与生产同路径：spawn_blocking 内执行（宿主函数的 block_on 只在 blocking 线程合法）
// Same path as production: run inside spawn_blocking (host-fn block_on is legal only there)
#[allow(clippy::too_many_arguments)]
async fn run_on_blocking(
    engine: &wasmtime::Engine,
    linker: &Arc<wasmtime::Linker<WasmCallCtx>>,
    module: wasmtime::Module,
    guard: PermissionGuard,
    pool: SqlitePool,
    plugin_id: &'static str,
    action: &'static str,
    payload: serde_json::Value,
    limits: InvokeLimits,
) -> (Result<serde_json::Value, String>, String) {
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
    let module = wasmtime::Module::new(&engine, ECHO_WAT).unwrap();
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
    let module = wasmtime::Module::new(&engine, wat).unwrap();
    let pool = test_pool().await;
    let (result, _) = run_on_blocking(
        &engine, &linker, module, PermissionGuard::default(), pool,
        "com.test.fuel", "x", serde_json::Value::Null,
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
    let module = wasmtime::Module::new(&engine, wat).unwrap();
    let _ticker = spawn_epoch_ticker(engine.clone(), Duration::from_millis(10));
    let pool = test_pool().await;
    let (result, _) = run_on_blocking(
        &engine, &linker, module, PermissionGuard::default(), pool,
        "com.test.timeout", "x", serde_json::Value::Null,
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
    let module = wasmtime::Module::new(
        &engine,
        &host_call_wat("kapi:storage.set", r#"{"key":"k","value":1}"#),
    )
    .unwrap();

    // 未声明权限 → PermissionDenied / no permission declared -> denied
    insert_plugin(&pool, "com.test.host", r#"{"id":"com.test.host"}"#, "/tmp/wasm-test").await;
    let (result, _) = run_on_blocking(
        &engine, &linker, module.clone(), PermissionGuard::default(), pool.clone(),
        "com.test.host", "x", serde_json::Value::Null, InvokeLimits::default(),
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
        "com.test.host", "x", serde_json::Value::Null, InvokeLimits::default(),
    )
    .await;
    assert_eq!(result.unwrap(), serde_json::Value::Null);
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
    let module = wasmtime::Module::new(
        &engine,
        &host_call_wat("kapi:log.info", r#"{"message":"from wasm"}"#),
    )
    .unwrap();
    let (result, _) = run_on_blocking(
        &engine, &linker, module, PermissionGuard::default(), pool.clone(),
        "com.test.log", "x", serde_json::Value::Null, InvokeLimits::default(),
    )
    .await;
    assert_eq!(result.unwrap(), serde_json::Value::Null);
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
        let module = wasmtime::Module::new(&engine, &host_call_wat(channel, "{}")).unwrap();
        let (result, _) = run_on_blocking(
            &engine, &linker, module, PermissionGuard::default(), pool.clone(),
            "com.test.deny", "x", serde_json::Value::Null, InvokeLimits::default(),
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
    let null_ret = wasmtime::Module::new(
        &engine,
        r#"(module (memory (export "memory") 1)
                (func (export "kapi_alloc") (param i32) (result i32) (i32.const 1024))
                (func (export "kapi_invoke") (param i32 i32) (result i64) (i64.const 0)))"#,
    )
    .unwrap();
    let (r, _) = run_on_blocking(&engine, &linker, null_ret, PermissionGuard::default(), pool.clone(), "t", "x", serde_json::Value::Null, limits).await;
    assert_eq!(r.unwrap_err(), "WasmError: guest returned null result");

    // 缺 kapi_alloc 导出 / missing kapi_alloc export
    let no_alloc = wasmtime::Module::new(
        &engine,
        r#"(module (memory (export "memory") 1)
                (func (export "kapi_invoke") (param i32 i32) (result i64) (i64.const 0)))"#,
    )
    .unwrap();
    let (r, _) = run_on_blocking(&engine, &linker, no_alloc, PermissionGuard::default(), pool.clone(), "t", "x", serde_json::Value::Null, limits).await;
    assert_eq!(r.unwrap_err(), "WasmError: missing export kapi_alloc");

    // 非法结果 JSON / invalid result JSON
    let garbage = wasmtime::Module::new(
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
    let (r, _) = run_on_blocking(&engine, &linker, garbage, PermissionGuard::default(), pool, "t", "x", serde_json::Value::Null, limits).await;
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
        .invoke_action(&pool, "com.kapi.sample.plugin-d", "nope", &serde_json::Value::Null)
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
