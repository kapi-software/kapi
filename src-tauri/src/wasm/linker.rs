// Engine + Linker 装配：consume_fuel + epoch_interruption；WASI p1 + kapi 宿主导入
// Engine + Linker assembly: consume_fuel + epoch_interruption; WASI p1 + the kapi host import
use wasmtime::{Caller, Config, Engine, Linker};
use wasmtime_wasi::p1::add_to_linker_sync;

use crate::wasm::run::{host_call, WasmCallCtx};

pub fn build_engine_linker() -> Result<(Engine, Linker<WasmCallCtx>), String> {
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
