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
    "alwaysOnTop": false
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

## 3. 权限模型

- **默认拒绝**：manifest 未声明的权限，桥接调用一律返回 `PermissionDenied`。
- **检查点唯一**：所有权限检查集中在 Rust `plugin_bridge` 命令与 wasmtime 宿主函数两处（共用同一个 `PermissionGuard`）。
- **网络权限带域名白名单**：`network:host:<domain>`，宿主侧代理请求并校验目标域名。
- **存储天然隔离**：`storage:*` 只能访问 `plugin_data` 中自身 `plugin_id` 命名空间。
- **严格模式**：`plugin_sandbox_strict = true` 时，未签名插件拒绝加载（签名验证见 ROADMAP.md 风险表）。

## 4. 桥接 API（插件 SDK 通道）

插件 UI 通过 `@kapi/plugin-sdk` 调用，SDK 内部走 `postMessage('kapi:*')` → `PluginHost` → `plugin_bridge`（权限检查）→ 执行：

| 通道 | 权限 | 说明 |
| ---- | ---- | ---- |
| `kapi:storage.get/set/remove` | `storage:*` | 插件隔离 KV |
| `kapi:clipboard.read/write` | `clipboard:read/write` | 剪贴板 |
| `kapi:http.fetch` | `network:host:<domain>` | 宿主代理 HTTP |
| `kapi:events.emit` / `kapi:events.on` | `events:emit/subscribe` | 事件总线（可触发工作流） |
| `kapi:plugin.invoke(action, payload)` | 无需声明（调用自身） | 调用本插件 WASM 入口 |
| `kapi:window.setTitle/close/minimize` | — | independent 模式窗口控制 |
| `kapi:log.debug/info/warn/error` | — | 写 system_logs |

## 5. WASM 运行时（wasmtime）

```rust
// src-tauri/src/wasm_runtime.rs（示意）
pub struct WasmRuntime {
    // plugin_id → 已实例化 engine，懒加载 + 复用
    instances: Mutex<HashMap<String, PluginInstance>>,
}

impl WasmRuntime {
    // 调用插件 WASM 入口：JSON 进 / JSON 出
    pub async fn invoke_action(
        &self,
        plugin_id: &str,
        action: &str,
        payload: &Value,
    ) -> Result<Value, String> {
        // 1. 取插件实例（懒加载 install_path/main.wasm，wasmtime 编译缓存）
        // 2. 序列化 { action, payload } 写入线性内存
        // 3. 调用导出函数 kapi_invoke(ptr, len) -> i64（结果 JSON 回读）
        // 4. 反序列化返回
    }
}
```

- **ABI（v1，刻意保持简单）**：单一导出函数 `kapi_invoke`，JSON 字符串经线性内存传递。提供 `kapi-plugin-sdk`（Rust crate，编译目标 wasm32-wasip1）封装序列化与宿主函数导入。后续如需更强类型再引入 WIT 组件模型。
- **宿主函数**：WASM 内可导入与 §4 相同能力的 `kapi_host_*` 函数（storage/clipboard/http/events/log），同样经 `PermissionGuard` 检查。
- **资源限制**：fuel metering 限制单次执行指令数（防死循环），执行超时 5s 强制中断。
- **UI ↔ WASM 通路**：插件 UI `kapi:plugin.invoke` → `plugin_bridge` → `WasmRuntime::invoke_action` → JSON 结果原路返回。

## 6. 插件生命周期

```text
安装(市场/本地导入) → 校验 manifest → 解压到 plugins/{id}/ → 写 plugins 表 → 就绪
    ↓
运行: launch_plugin 按模式分发（ARCHITECTURE.md §2.3）
    ↓
禁用: is_enabled = 0，Dock 与侧边栏隐藏，工作流节点标记不可用
    ↓
卸载: 关闭其独立窗口 → DELETE plugins 行（CASCADE 清 plugin_data）→ 删除安装目录
```
