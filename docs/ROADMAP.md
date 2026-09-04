# 开发计划

Kapi 技术文档：里程碑、任务清单与风险表。

## 1. 里程碑

| 阶段 | 周期 | 目标 |
| ---- | ---- | ---- |
| **Phase 1** | 1 周 | 项目初始化（Tailwind + shadcn/ui + Router + Zustand + Tauri 插件注册）、数据库迁移与访问层 |
| **Phase 2** | 2 周 | 主面板框架：侧边栏导航、页面路由、主题系统、设置页（含 Dock 开关） |
| **Phase 3** | 2 周 | Dock 侧边栏：Rust 光标轮询（跨平台）、透明窗口、弧形 UI、唤醒链路（launch_plugin） |
| **Phase 4** | 3 周 | 插件系统：kapi-plugin 协议、PluginHost（内嵌+独立窗口）、wasmtime 运行时、桥接 API 与权限模型 |
| **Phase 5** | 1 周 | 插件市场：GitHub 来源浏览、下载安装、卸载更新 |
| **Phase 6** | 2 周 | 工作流：触发器管理、DAG 引擎与两级日志、React Flow 编辑器 |
| **Phase 7** | 1 周 | 打磨、插件签名验证（严格模式收尾）、测试、打包 |

## 2. 当前任务

- [x] 确定技术方案
- [x] 完善设计文档（含数据库结构与插件系统细化）
- [x] 数据库设计（8 表 + 索引 + 迁移规划）
- [x] 初始化项目（Tailwind v4 + shadcn/ui + Router + Zustand + vitest，2026-08-25）
- [x] 实现数据库层（Rust 迁移 + 前端 db.ts，2026-08-25）
- [x] i18n（react-i18next，zh-CN / en-US）
- [x] 实现主面板框架（分组导航 + 设置页全量设置项 + 强调色主题 + 日志过滤/自动刷新，2026-08-25）
- [x] 移植 Dock 侧边栏（弧形 UI + Windows 热区轮询 + 唤醒链路，2026-08-25；macOS/Linux 轮询待移植）
- [x] kapi-plugin:// 自定义协议（静态资源服务 + 路径安全，22 项单元测试，2026-08-25）
- [x] 插件管理器（本地导入 / 卸载 / 启停 / 模式切换 / 排序 + launch_plugin 完整分发 + 最小 PluginHost 链路，2026-08-25）
- [x] PluginHost postMessage 桥接（协议处理纯函数化 + 来源校验 + Tauri 门控，2026-08-25）
- [x] 桥接 API 与权限模型（plugin_bridge 命令：PermissionGuard 默认拒绝 + storage/clipboard/http/events/log/window 通道；plugin.invoke 待 wasmtime，events.on 待 SDK，2026-08-25）
- [x] 独立窗口定制（manifest.window 对齐 Tauri 窗口选项：transparent/decorations/skipTaskbar/shadow/center/fullscreen + 透明壳加载门控，2026-08-25）
- [x] wasmtime 运行时（wasmtime 48 + WASI p1：ABI v1 / 单宿主导入 kapi_host_call 复用桥接分发 / fuel 10 亿 + 5s epoch + 64MiB 内存限制 / 模块缓存与 evict；pluginD 全链路示例 + fixture 单测，2026-08-25）
- [x] headless 启动（立即执行默认动作：run 优先 → 首个 action，成败写 system_logs；Phase 6 工作流接管编排，2026-08-25）
- [x] @kapi/plugin-sdk 前端 SDK（保留路径 `/__kapi__/sdk.js` 分发 + kapi.events.on/off 订阅推送链路 + pluginA 示范，2026-08-30）
- [x] 插件市场（索引源 + 独立插件仓库：store_list 缓存优先 / store_install 防护提取与更新语义，Cloudflare Worker 契约预留，2026-08-30）
- [x] **工作流引擎**（触发器 + DAG 调度 + 两级日志，2026-08-30）
- [x] **工作流编辑器**（React Flow + NodePalette + NodeInspector + BindingsEditor，2026-08-30）
- [x] **Transform 节点**（handlebars 模板渲染，2026-08-30）
- [x] **触发器类型**：schedule（tokio time）/ plugin_event（订阅进程内事件总线）/ clipboard（tauri-plugin-clipboard-manager）/ hotkey（tauri-plugin-global-shortcut）
- [x] **触发器前端**：TriggerDialog（Combobox 搜索下拉框，合并 manifest + 历史事件来源）、TriggerListPanel、BindingsDrawer
- [x] **shadcn 组件**：Select / Switch / Combobox（搜索下拉）/ Drawer / InputGroup / Textarea

## 3. 已完成清单

- [x] 项目初始化（Tauri v2 + React 19 + Tailwind v4 + shadcn/ui）
- [x] 数据库迁移（tauri-plugin-sql Migration，Rust 侧唯一入口）
- [x] i18n（react-i18next，zh-CN / en-US，语言切换持久化）
- [x] 主面板布局（分组侧边栏 + 内容区 + 路由，2026-08-25）
- [x] 设置页面（统一 settings 表全量设置项；主题/强调色实时生效，Dock 开关实时联动，2026-08-25）
- [x] 主题系统（light/dark/system + accent CSS 变量，2026-08-25）
- [x] 日志页（级别过滤 + 自动刷新，2026-08-25）
- [x] Dock 窗口（边沿触发热区轮询 + motion 弧形前端 + 仅唤醒，2026-08-25；Windows 轮询已实现，macOS/Linux 见 DOCK.md §4 平台表）
- [x] kapi-plugin:// 自定义协议（静态资源服务 + 路径安全，2026-08-25）
- [x] PluginHost（iframe 宿主 + postMessage 桥接已接通：/plugin/:id 内嵌 + /plugin-window/:id 独立壳共用，2026-08-25）
- [x] 插件独立窗口（manifest 自定义窗口参数，重复点击聚焦，2026-08-25；窗口选项扩展对齐 Tauri：transparent/decorations/skipTaskbar/shadow/center/fullscreen，2026-08-25）
- [x] 插件管理器（本地导入安装/卸载/启停/模式切换/排序，2026-08-25；市场安装属 Phase 5）
- [x] wasmtime 运行时（kapi_invoke ABI + 宿主导入 + fuel/epoch/内存限制，2026-08-25）
- [x] 桥接 API 与权限模型（plugin_bridge + PermissionGuard 默认拒绝；kapi:plugin.invoke 待 wasmtime、kapi:events.on 待 SDK，2026-08-25）
- [x] kapi-plugin-sdk（WASM Rust SDK crate 抽取，现内联于 plugins/pluginD/wasm-src；前端 @kapi/plugin-sdk 已就绪）
- [x] 插件市场（GitHub API + 安装流程，2026-08-30；GitHub 鉴权 / Release 资产源属后续演进）
- [x] 工作流引擎（触发器 + DAG 调度 + 两级日志，2026-08-30）
- [x] 工作流编辑器（React Flow + NodePalette + NodeInspector + BindingsEditor，2026-08-30）
- [x] Transform 节点（handlebars 模板渲染，2026-08-30）
- [x] Schedule 触发器（tokio time::interval，cron 表达式解析，2026-08-30）
- [x] PluginEvent 触发器（轮询 plugin_events 表，manifest 声明 + 历史事件合并，2026-08-30）
- [x] Clipboard 触发器（tauri-plugin-clipboard-manager，2026-08-30）
- [x] Hotkey 触发器（tauri-plugin-global-shortcut，2026-08-30）
- [x] 触发器 UI（TriggerDialog + TriggerListPanel + BindingsDrawer，2026-08-30）

## 4. 待开发清单

- [ ] kapi-plugin-sdk（WASM Rust SDK crate 抽取）
- [ ] 全局快捷键（Alt+Space 唤醒 Dock，现由设置项控制）
- [ ] 打包配置与签名
- [ ] 插件签名验证（严格模式）
- [ ] macOS / Linux Dock 热区轮询

## 5. 风险与待定项

| 项 | 说明 | 缓解 |
| -- | ---- | ---- |
| Electron Dock 移植精度 | 已与源码逐项对齐（DOCK.md §1–§5：窗口参数/几何/状态机/轮询算法/穿透策略） | 遗留差异一项：Tauri 无 mousemove 转发，见 DOCK.md §3 注 |
| 插件签名验证 | 严格模式依赖签名，签名体系（算法/密钥分发）未设计 | Phase 7 收尾；开发期严格模式默认关 |
| Wayland 下 Dock 热区 | X11 轮询在 Wayland 不可用 | 降级为快捷键唤醒 |
| WASM ABI 稳定性 | JSON 字符串 ABI 简单但弱类型 | kapi_version 字段 + 后续可迁移 WIT 组件模型 |
