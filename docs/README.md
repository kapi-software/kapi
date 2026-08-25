# Kapi 设计文档

设计文档按主题拆分，本页为索引。

| 文档 | 内容 |
| ---- | ---- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 项目概述、应用架构、窗口模型、统一插件启动流程、技术实现（项目结构 / Tauri 命令 / 自定义协议 / 窗口配置） |
| [DATABASE.md](DATABASE.md) | SQLite 设计：8 张表 DDL、索引、默认数据、迁移策略、前端访问层 |
| [PANEL.md](PANEL.md) | 主面板：布局、路由、PluginHost 统一插件宿主；设置系统 |
| [DOCK.md](DOCK.md) | Dock 侧边栏：窗口参数、弧线几何、三态状态机、热区轮询算法（与 Electron 版源码逐项对齐） |
| [PLUGINS.md](PLUGINS.md) | 插件系统：包结构、manifest、权限模型、桥接 API、WASM 运行时（wasmtime） |
| [WORKFLOW.md](WORKFLOW.md) | 工作流系统：DAG 数据模型、执行引擎、触发器 |
| [ROADMAP.md](ROADMAP.md) | 开发计划：里程碑、任务清单、风险表 |

阅读顺序建议：ARCHITECTURE → DATABASE → 其余按需。
