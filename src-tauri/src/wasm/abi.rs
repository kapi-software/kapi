// ABI v1 打包：ret = ptr << 32 | len，纯函数无 wasmtime 依赖
// ABI v1 packing: ret = ptr << 32 | len, pure functions without wasmtime deps
pub fn pack_result(ptr: u32, len: u32) -> i64 {
    ((ptr as i64) << 32) | (len as i64 & 0xFFFF_FFFF)
}

pub fn unpack_result(ret: i64) -> (u32, u32) {
    (((ret >> 32) as u32), (ret as u32))
}
