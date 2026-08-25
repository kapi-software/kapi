# pluginD wasm-src

`main.wasm` fixture 的源码：Kapi ABI v1（docs/PLUGINS.md §5）的内联实现，未来抽为 `kapi-plugin-sdk` crate。

Source of the `main.wasm` fixture: an inline Kapi ABI v1 (docs/PLUGINS.md §5) implementation, to be extracted into the future `kapi-plugin-sdk` crate.

## 构建 / Build

```bash
# 一次性：安装编译目标 / one-off: install the target
rustup target add wasm32-wasip1

# 构建 / build
cargo build --release --target wasm32-wasip1

# 拷贝产物（改源码后必须重建并提交，fixture 单测会校验行为）/ copy the
# artifact (rebuild and commit after any change; a fixture unit test guards drift)
cp target/wasm32-wasip1/release/plugin_d_wasm.wasm ../main.wasm
```

## 动作 / Actions

| action | 输入 | 输出 | 说明 |
|---|---|---|---|
| `reverse` | `{text}` | `{text}` | 反转文本；缺失 text 视为 `""`（headless 空跑可成功） |
| `log` | `{text}` | `{logged: true}` | 经 `kapi_host_call` 调 `kapi:log.info` 写系统日志 |
| 其它 | — | `UnknownAction: <name>` | |

`kapi_alloc`：静态 256 KiB bump 堆（8 字节对齐，绝对线性地址返回，溢出返 0）。
`kapi_host_call`：宿主提供的唯一导入，与 UI 桥共用通道分发与权限守卫。
