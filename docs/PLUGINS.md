# 插件系统

Kapi 技术文档：插件包结构、manifest、权限模型、桥接 API 与 WASM 运行时。

> **架构定位**：Tauri 版插件系统**基于 WASM**（逻辑运行在 wasmtime 沙箱），为全新设计；
> Electron 版的插件宿主/沙箱实现（web worker 形态）**不移植**，仅插件产品概念（安装源、
> 独立窗口、KV 存储、事件）在本设计中等价重现。

## 1. 插件包结构

```text
com.example.code-beautifier/        # 目录名 = 插件 id
├── manifest.json                   # 元数据与权限声明（§2）
├── icon.png                        # 128×128 图标
├── main.wasm                       # 逻辑入口（可选，纯 UI 插件可省略）
└── web/                            # UI 资源（可选，headless 插件可省略）
    ├── index.html
    ├── main.js                     # 引入 @kapi/plugin-sdk（桥接 RPC 封装）
    └── style.css
```

安装后解压至 `{app_data}/plugins/{id}/`，`plugins` 表记录 `install_path` 及相对入口。

## 2. manifest.json

```jsonc
{
  "id": "com.example.code-beautifier",
  "name": "代码美化",
  "version": "1.0.0",
  "kapi_version": "^1.0.0",          // 兼容的宿主 API 版本
  "author": "Your Name",
  "description": "代码格式化与收藏",

  // 运行模式与独立窗口自定义（三选一；用户可在插件管理页覆盖 mode）
  "window": {
    "mode": "embedded",              // 'embedded' | 'independent' | 'headless'
    "title": "代码美化",
    "width": 420,
    "height": 640,
    "minWidth": 320,
    "minHeight": 400,
    "resizable": true,
    "alwaysOnTop": false,
    "transparent": false,            // 透明背景（窗口与页面双透明）
    "decorations": true,             // false = 无边框（隐藏标题栏）
    "skipTaskbar": false,            // 不在任务栏显示
    "shadow": true,                  // 窗口投影（仅 Windows/Linux）
    "center": true,                  // 居中创建
    "fullscreen": false
  },

  // 工作流能力声明（headless 插件的主要形态）
  "workflow": {
    "triggers": ["clipboard_changed"],
    "actions": [
      { "name": "format", "inputs": { "text": "string" }, "outputs": { "formatted": "string" } },
      { "name": "save",   "inputs": { "formatted": "string" }, "outputs": { "savedId": "string" } }
    ],
    "events": ["processed", "saved"]
  },

  // 权限声明（默认全部拒绝，逐项显式申请）
  "permissions": [
    "storage:read", "storage:write",
    "clipboard:read",
    "network:host:api.github.com"
  ]
}
```

### 2.1 运行模式

| 模式 | UI | 逻辑 | 典型场景 |
| ---- | -- | ---- | -------- |
| **embedded** | 主面板 `/plugin/:id` 内嵌 iframe | WASM / 桥接 API | 工具类插件，与主界面紧密集成 |
| **independent** | 独立 `WebviewWindow`，窗口参数取自 manifest.window | 同上 | 需要独立空间、复杂 UI、常驻的插件 |
| **headless** | 无 UI | 仅 WASM | 工作流数据处理插件、后台服务 |

三种模式共用同一套桥接 API 与 WASM 入口；`window_mode` 用户可随时在插件管理页切换。

### 2.2 独立窗口选项（对齐 Tauri 窗口设置）

`window` 内除 `mode` 外的字段仅在 **independent** 模式下生效，安装时快照进 `plugins.window_config`（改 manifest 窗口字段需重装生效；更新流程属 Phase 5）：

| 字段 | 类型 | 默认 | 说明 |
| ---- | ---- | ---- | ---- |
| `title` | string | 插件 id | 窗口标题 |
| `width` / `height` | number | 800 / 600 | 初始尺寸 |
| `minWidth` / `minHeight` | number | — | 最小尺寸（两者齐备才生效） |
| `resizable` | bool | `true` | 可缩放 |
| `alwaysOnTop` | bool | `false` | 置顶 |
| `transparent` | bool | `false` | 透明背景：窗口与页面 html/body 双透明 |
| `decorations` | bool | `true` | `false` = 无边框（隐藏标题栏），配合 `kapi:window.startDragging` 自绘拖拽区 |
| `skipTaskbar` | bool | `false` | 不在任务栏显示（macOS 同时从 Cmd-Tab 隐藏） |
| `shadow` | bool | `true` | 窗口投影（仅 Windows/Linux，macOS 忽略） |
| `center` | bool | `true` | 居中创建 |
| `fullscreen` | bool | `false` | 全屏（与 `resizable:false` 的组合行为因 OS 而异，插件作者自行验证） |

> **平台差异**：透明在 macOS 需 `macos-private-api`（已启用）；Linux X11 无合成器时透明退化为黑底（Wayland 多数合成器可用）；Windows 无边框 + 投影在部分版本有细边伪影，可将 `shadow` 设 `false` 回退。窗口选项当前**直接生效不经权限门控**，Phase 7 硬化时再评估收紧。

## 3. 权限模型（已实现：`src-tauri/src/plugin_bridge.rs`）

- **默认拒绝**：manifest 未声明的权限，桥接调用一律返回 `PermissionDenied`。
- **检查点唯一**：所有权限检查集中在 Rust `dispatch_channel`（UI 桥 `plugin_bridge` 命令与 WASM 宿主导入 `kapi_host_call` 共用同一分发与 `PermissionGuard`）；前端不做任何权限判断。
- **按次加载**：每次桥接调用从 `plugins.manifest` 重新解析权限快照——禁用/卸载/manifest 变更即时生效，无缓存失效问题。
- **上下文闸**：未安装（`PluginNotFound`）或已禁用（`PluginDisabled`）的插件连免权限通道（window/log）也拿不到。
- **网络权限带域名白名单**：`network:host:<domain>` 精确匹配（子域不隐式放行）或 `network:host:*` 通配；宿主侧先校验域名再代理请求，且禁跟随重定向（防 3xx 绕过白名单）。
- **存储天然隔离**：`storage:*` 只能访问 `plugin_data` 中自身 `plugin_id` 命名空间。
- **严格模式**：`plugin_sandbox_strict = true` 时，未签名插件拒绝加载（签名验证见 ROADMAP.md 风险表）。

**错误码**（`错误码: 细节` 格式，供 SDK 机器解析）：`PermissionDenied` / `PluginNotFound` / `PluginDisabled` / `InvalidPayload` / `UnknownChannel` / `WindowNotAllowed` / `NotImplemented` / `StorageError` / `ClipboardError` / `HttpError` / `EventError` / `WasmError`（fuel/timeout/内存超限/缺导出/非法结果等沙箱错误）/ `UnknownAction`（guest 侧未知动作）。

**载荷边界**：key ≤256 字符；storage 值 ≤1 MiB；title ≤256；日志 message ≤2000；事件 type ≤128 且仅 `[A-Za-z0-9._-]`；http 仅 http/https、方法白名单、响应体 ≤1 MiB、超时 10s。

## 4. 桥接 API（已实现：postMessage → plugin_bridge）

插件 UI 通过 `@kapi/plugin-sdk` 调用（SDK 未发布前可按 docs/PANEL.md §3 协议裸调 postMessage，示例见 `plugins/pluginA`）；SDK 内部走 `postMessage('kapi:*')` → `PluginHost` → `plugin_bridge`（权限检查）→ 执行：

| 通道 | 权限 | 请求 payload | 成功 data |
| ---- | ---- | ---- | ---- |
| `kapi:storage.get` | `storage:read` | `{key}` | `{value}`（JSON 或 null） |
| `kapi:storage.set` | `storage:write` | `{key, value}` | `null` |
| `kapi:storage.remove` | `storage:write` | `{key}` | `null` |
| `kapi:clipboard.read` | `clipboard:read` | — | `{text}` |
| `kapi:clipboard.write` | `clipboard:write` | `{text}` | `null` |
| `kapi:http.fetch` | `network:host:<domain>` 或 `network:host:*` | `{url, method?, headers?, body?}` | `{status, headers, body}` |
| `kapi:events.emit` | `events:emit` | `{type, data?}` | `null`（写 plugin_events） |
| `kapi:events.on` | `events:subscribe` | — | **未实现**（订阅需 SDK 推送协议，随 @kapi/plugin-sdk 落地） |
| `kapi:plugin.invoke` | 无需声明（调用自身） | `{action, payload?}` | guest 动作返回的 `data`；未知动作 → `UnknownAction: <name>` |
| `kapi:window.setTitle` | — | `{title}` | `null` |
| `kapi:window.getInfo` | — | — | `{mode}`（`"embedded"` \| `"independent"`；只读环境查询，两种模式均可调用，插件据此隐藏/展示窗口控制按钮） |
| `kapi:window.close` / `minimize` / `startDragging` | — | — | `null` |
| `kapi:log.debug/info/warn/error` | — | `{message, data?}` | `null`（写 system_logs，source=`plugin:<id>`） |

> `kapi:window.*` 仅在插件**自己的独立窗口**内可用（调用方窗口 label 与 `plugin-<id>` 精确匹配）；embedded 模式返回 `WindowNotAllowed`，天然防跨插件控窗。`startDragging` 是无边框窗口的唯一移动方式。

## 5. WASM 运行时（已实现：`src-tauri/src/wasm_runtime.rs`，wasmtime 48 + WASI preview1）

- **执行模型**：同步 Store 放 `spawn_blocking`；宿主函数经 `Handle::block_on` 复用 `dispatch_channel` —— UI 与 WASM 共用同一权限守卫与通道路由。
- **模块缓存**：按插件缓存编译产物（fingerprint = 文件长度 + mtime，重装/改文件自动重编译；卸载/重装时 `evict`）；**每次调用新建 Store/Instance**（fuel 重置、并发互不干扰、trap 不腐蚀后续调用；ABI v1 无状态语义，持久化由 `kapi:storage` 承担）。
- **WASI**：preview1（wasm32-wasip1 的 std 依赖）；无 preopen、无网络、空 args/envs；stdout/stderr 捕获为内存管道，失败时摘录 ≤2KB 写 system_logs。
- **Phase 6 入口**：`WasmRuntime::invoke_action(pool, plugin_id, action, payload)` 为工作流引擎的正式调用接口。

**ABI v1（刻意保持简单；`kapi-plugin-sdk` crate 抽取前的内联实现见 `plugins/pluginD/wasm-src`）**：

| 方向 | 名称 | 签名 | 说明 |
|---|---|---|---|
| guest 导出 | `memory` | 线性内存 | wasm32 单内存，按名导出 |
| guest 导出 | `kapi_alloc` | `(size: i32) -> i32` | 8 字节对齐 bump 分配，返回绝对地址；失败返 0 |
| guest 导出 | `kapi_invoke` | `(req_ptr: i32, req_len: i32) -> i64` | 请求经 `kapi_alloc` 缓冲写入；返回值打包 `(ptr << 32) \| len`，`0` = guest 致命错误 |
| host 导入 | `kapi.kapi_host_call` | `(chan_ptr, chan_len, payload_ptr, payload_len: i32) -> i64` | 唯一宿主导入；通道集 = §4（`kapi:window.*` → `WindowNotAllowed`、`kapi:plugin.invoke` → 禁嵌套）；返回同打包规则 |
| host 导入 | `wasi_snapshot_preview1.*` | — | wasmtime-wasi 提供 |

**JSON 信封**：请求 `{"action","payload"}`；结果 `{"ok":true,"data":...}` | `{"ok":false,"error":"Code: detail"}`；未知动作由 guest 自行返回 `UnknownAction: <name>`；两侧 ≤1 MiB。后续如需更强类型再引入 WIT 组件模型。

**资源限制**：fuel 10 亿条/次（防死循环，`WasmError: fuel exhausted`）；epoch ticker 100ms × deadline 50 = **5s 硬超时**（`WasmError: timeout`）；线性内存 ≤64 MiB（`ResourceLimiter`）；import 模块白名单 `{wasi_snapshot_preview1, kapi}`；必需导出 `memory/kapi_alloc/kapi_invoke`。

**UI ↔ WASM 通路**：插件 UI `kapi:plugin.invoke {action, payload}` → `plugin_bridge` → `WasmRuntime::invoke_action` → 沙箱执行 → JSON 结果原路返回（无需权限声明，调用自身）。完整示例见 `plugins/pluginD`（reverse / log 动作）。

## 6. 插件生命周期

```text
安装(市场/本地导入) → 校验 manifest → 复制到 plugins/{id}/ → 写 plugins 表（+evict wasm 缓存）→ 就绪
    ↓
运行: launch_plugin 按模式分发（ARCHITECTURE.md §2.3）
    ├─ embedded → 主面板 /plugin/:id
    ├─ independent → 独立窗口（manifest.window，含透明/无边框等定制）
    └─ headless → 立即执行一次默认动作（run 优先 → 首个 action → "run"），
       成败写 system_logs（source = plugin:<id>）；Phase 6 工作流引擎接管触发编排
    ↓
禁用: is_enabled = 0，Dock 与侧边栏隐藏，桥接一律 PluginDisabled，工作流节点标记不可用
    ↓
卸载: 关闭其独立窗口 → evict wasm 缓存 → DELETE plugins 行（CASCADE 清 plugin_data）→ 删除安装目录
```
