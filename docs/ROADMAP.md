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

## 3. 待开发清单

- [x] 项目初始化（Tauri + React 19 + Tailwind + shadcn/ui）
- [x] 数据库迁移（tauri-plugin-sql Migration，Rust 侧唯一入口）
- [x] i18n（react-i18next，zh-CN / en-US，语言切换持久化）
- [x] 主面板布局（分组侧边栏 + 内容区 + 路由，2026-08-25）
- [x] 设置页面（统一 settings 表全量设置项；主题/强调色实时生效，Dock 开关实时联动随 Phase 3 dock_service）
- [x] 主题系统（light/dark/system + accent CSS 变量，2026-08-25）
- [x] 日志页（级别过滤 + 自动刷新，2026-08-25）
- [x] Dock 窗口（边沿触发热区轮询 + motion 弧形前端 + 仅唤醒，2026-08-25；Windows 轮询已实现，macOS/Linux 见 DOCK.md §4 平台表）
- [x] kapi-plugin:// 自定义协议（静态资源服务 + 路径安全，2026-08-25）
- [ ] PluginHost（iframe 宿主已就绪：/plugin/:id 内嵌 + /plugin-window/:id 独立壳共用；postMessage 桥接待接）
- [x] 插件独立窗口（manifest 自定义窗口参数，重复点击聚焦，2026-08-25）
- [x] 插件管理器（本地导入安装/卸载/启停/模式切换/排序，2026-08-25；市场安装属 Phase 5）
- [ ] wasmtime 运行时（kapi_invoke ABI + 宿主函数 + fuel 限制）
- [ ] 桥接 API 与权限模型（PermissionGuard，默认拒绝）
- [ ] @kapi/plugin-sdk（插件前端 SDK）与 kapi-plugin-sdk（WASM Rust SDK）
- [ ] 插件市场（GitHub API + 安装流程）
- [ ] 工作流引擎（触发器 + DAG 调度 + 两级日志）
- [ ] 工作流编辑器（React Flow）
- [ ] 全局快捷键（Alt+Space 唤醒 Dock、hotkey 触发器）
- [ ] 打包配置与签名

## 4. 风险与待定项

| 项 | 说明 | 缓解 |
| -- | ---- | ---- |
| Electron Dock 移植精度 | 已与源码逐项对齐（DOCK.md §1–§5：窗口参数/几何/状态机/轮询算法/穿透策略） | 遗留差异一项：Tauri 无 mousemove 转发，见 DOCK.md §3 注 |
| 插件签名验证 | 严格模式依赖签名，签名体系（算法/密钥分发）未设计 | Phase 7 收尾；开发期严格模式默认关 |
| Wayland 下 Dock 热区 | X11 轮询在 Wayland 不可用 | 降级为快捷键唤醒 |
| WASM ABI 稳定性 | JSON 字符串 ABI 简单但弱类型 | kapi_version 字段 + 后续可迁移 WIT 组件模型 |
