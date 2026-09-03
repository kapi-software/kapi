// 资源限制常量与 MemLimiter：单次调用的内存/fuel/epoch 上限
// Resource limit constants and MemLimiter: per-invoke memory/fuel/epoch caps
use wasmtime::ResourceLimiter;

// 单次调用的资源上限 / per-invoke resource caps
pub(crate) const MAX_MEMORY_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const MAX_ENVELOPE_BYTES: usize = 1024 * 1024;
pub(crate) const MAX_CAPTURE_BYTES: usize = 64 * 1024;
// 5s 硬超时 = 50 tick × 100ms ticker / the 5s hard timeout = 50 ticks at the 100ms ticker
pub(crate) const EPOCH_TICKER_PERIOD: std::time::Duration = std::time::Duration::from_millis(100);

// 单次调用的 fuel 与 epoch 预算（可注入便于单测）
// Per-invoke fuel and epoch budgets (injectable for tests)
#[derive(Debug, Clone, Copy)]
pub struct InvokeLimits {
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
pub struct MemLimiter {
    pub remaining: i64,
}

impl MemLimiter {
    pub(crate) fn new(max: usize) -> Self {
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
