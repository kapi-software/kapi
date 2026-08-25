# Kapi 项目开发规范

## 项目概述 / Project Overview

**Kapi** 是一款基于 Tauri 的插件化桌面应用，提供统一的插件管理和工作流编排能力。应用由主面板和 Dock 侧边栏两部分组成，支持插件在主面板内嵌显示或独立窗口运行，并通过工作流系统实现插件间的数据联动。所有数据持久化存储在本地 SQLite 数据库中。

**Kapi** is a Tauri-based plugin-oriented desktop application that provides unified plugin management and workflow orchestration capabilities. The application consists of a main panel and a Dock sidebar, supporting plugins to run either embedded in the main panel or in independent windows, with data linkage between plugins through the workflow system. All data is persistently stored in a local SQLite database.

---

## 〇、项目技术约束 / Project Technical Constraints

### 0.1 本机透明加密软件（TSD）/ Transparent Encryption on This Machine

本开发机的 DLP 透明加密软件会加密**受信进程（Node / VS Code / Claude Code）写入的 `.json` / `.txt` 文件**（密文头 `%TSD-Header-###%`），非受信进程（cargo、Git Bash 工具）读到密文导致构建失败。实测 `.ts` / `.tsx` / `.toml` / `.json5` / `.md` 不受影响；bash 写入的 `.json` 为明文。

This machine's DLP transparent-encryption software encrypts `.json` / `.txt` files written by trusted processes (Node / VS Code / Claude Code), causing cargo builds to fail when reading ciphertext. `.ts` / `.tsx` / `.toml` / `.json5` / `.md` are unaffected.

**规则 / Rules**：

- Tauri 配置使用 `tauri.conf.json5`（`config-json5` feature 已启用），capability 内联于 `app.security.capabilities`
- 新增配置 / 数据文件避免 `.json` / `.txt` 扩展名——语言包用 `.ts`，配置用 `.json5` 或 `.toml`
- 若必须生成 `.json` 且需被 Rust 读取，用 bash 写入（明文）

### 0.2 i18n 约定 / i18n Conventions

- 使用 `react-i18next`，语言包为 **TypeScript 模块**：`src/i18n/locales/zh-CN.ts` 与 `en-US.ts`（不用 `.json`，见 §0.1）
- 所有 UI 文案必须通过 `t()` 输出，禁止在组件内硬编码中文字符串
- 语言设置持久化于 `settings.language`（`zh-CN` / `en-US`），切换时同步 `i18n.changeLanguage` 与 `document.documentElement.lang`
- 两个语言包的 key 结构必须一致（有单元测试校验）

---

## 一、代码注释规范 / Code Comment Standards

**核心规则 / Core Rule**：注释正文一律**两行制**——第一行中文、第二行英文；仅单行元数据标签（`@file`、`@author`、表格表头等）可用「中文 / English」一行制。

Comment bodies MUST use the two-line style: Chinese on the first line, English on the second. Only single-line metadata tags (`@file`, `@author`, table headers) may use the one-line `Chinese / English` style.

### 1.1 文件头注释 / File Header Comment

每个文件必须包含文件头注释，包含文件说明、作者、创建日期和更新历史。

```typescript
/**
 * @file 文件名 / File Name
 * @description 文件功能描述
 * File function description
 * @author 作者名 / Author Name
 * @created YYYY-MM-DD
 * @updated YYYY-MM-DD
 *
 * @changes
 * - YYYY-MM-DD: 更新说明 / Update description
 * - YYYY-MM-DD: 添加功能 / Added feature
 */
```

### 1.2 函数注释 / Function Comments

每个函数必须包含详细的 JSDoc 注释，包含功能说明、参数、返回值和示例。

```typescript
/**
 * 计算 Dock 弧线位置
 * Calculate Dock arc positions
 *
 * @description 根据插件数量和偏移量计算每个插件在弧线上的位置
 * Calculate each plugin's position on the arc based on count and offset
 *
 * @param count - 插件数量
 * @param offset - 滚动偏移量
 * @param centerIndex - 中心位置索引（默认 4）
 * @param centerIndex - Center position index (default: 4)
 *
 * @returns 插件位置数组
 * @returns Array of plugin positions
 *
 * @example
 * const positions = calculateDockPositions(9, 0)
 * // returns [{ x: 60, y: 20, isActive: false }, ...]
 */
export function calculateDockPositions(
  count: number,
  offset: number,
  centerIndex: number = 4
): DockPosition[] {
  // 函数实现
  // Function implementation
}
```

### 1.3 行内注释 / Inline Comments

复杂的逻辑必须添加行内注释，解释代码意图而非代码本身。**一律两行制：第一行中文、第二行英文。**

```typescript
// 中文注释
// English comment

// 计算圆心位置：窗口右缘 (x=320) 和垂直中心 (y=280)
// Calculate center position: window right edge (x=320) and vertical center (y=280)
const centerX = 320
const centerY = 280

// 计算角度范围：上下各留 10% 边距
// Calculate angle range: leave 10% margin at top and bottom
const thetaStart = Math.PI / 2 + 0.1 * Math.PI
const thetaEnd = 3 * Math.PI / 2 - 0.1 * Math.PI

// 防止溢出：实际索引 = ((偏移 + i) % 总数 + 总数) % 总数
// Prevent overflow: actualIndex = ((offset + i) % total + total) % total
const actualIndex = ((offset + i) % total + total) % total
```

### 1.4 TODO 注释 / TODO Comments

未完成或待优化的代码必须使用 TODO 注释标记（同样两行制）。

```typescript
// TODO: 优化弧形动画性能
// TODO: Optimize arc animation performance
// FIXME: 修复 Windows 平台透明窗口黑屏问题
// FIXME: Fix transparent window black screen on Windows
// HACK: 临时方案，后续需要重构
// HACK: Temporary workaround, needs refactoring later
// NOTE: 此处依赖 wasmtime 最新稳定版
// NOTE: Depends on the latest stable wasmtime version
```

### 1.5 Rust 注释规范 / Rust Comment Standards

```rust
/// 插件管理器
/// Plugin Manager
///
/// 负责管理所有插件的生命周期，包括安装、卸载、启动和停止。
/// Responsible for managing the lifecycle of all plugins, including installation,
/// uninstallation, startup, and shutdown.
///
/// # 示例 / Example
///
/// ```
/// let manager = PluginManager::new();
/// manager.install_plugin("com.example.plugin").await?;
/// ```
pub struct PluginManager {
    /// 已安装插件列表
    /// List of installed plugins
    plugins: HashMap<String, PluginInstance>,
    /// 数据库连接
    /// Database connection
    db: Arc<Mutex<Database>>,
}

impl PluginManager {
    /// 启动插件
    /// Launch a plugin
    ///
    /// 根据插件配置的窗口模式，决定在主面板内嵌显示、独立窗口运行或作为无界面工作流执行。
    /// Based on the plugin's window mode configuration, decide whether to display embedded
    /// in the main panel, run in an independent window, or execute as a headless workflow.
    ///
    /// # 参数 / Arguments
    /// `plugin_id` - 插件唯一标识符
    /// `plugin_id` - Plugin unique identifier
    ///
    /// # 返回值 / Returns
    /// `Result<(), String>` - 成功返回 Ok，失败返回错误信息
    /// `Result<(), String>` - Ok on success, error message on failure
    pub async fn launch_plugin(&mut self, plugin_id: &str) -> Result<(), String> {
        // 1. 从数据库获取插件信息
        // 1. Get plugin info from database
        let plugin = self.get_plugin(plugin_id).await?;

        // 2. 根据窗口模式分发
        // 2. Dispatch based on window mode
        match plugin.window_mode.as_str() {
            "embedded" => self.launch_embedded(plugin).await,
            "independent" => self.launch_independent(plugin).await,
            _ => self.launch_headless(plugin).await,
        }
    }
}
```

---

## 二、开发流程规范 / Development Process Standards

### 2.1 开发前检查 / Pre-development Checklist

每次开始新功能开发前，必须完成以下检查：

```markdown
## 开发前检查清单 / Pre-development Checklist

- [ ] 我已阅读相关文档 / I have read the relevant documentation
- [ ] 我了解当前分支的代码状态 / I understand the current branch code status
- [ ] 我已创建功能分支 / I have created a feature branch
- [ ] 我已在本地运行测试并通过 / I have run tests locally and they passed
- [ ] 我确认开发环境配置正确 / I confirm the development environment is configured correctly
```

### 2.2 功能开发流程 / Feature Development Process

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        功能开发流程 / Feature Development Process          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. 需求分析 / Requirements Analysis                                       │
│     ↓                                                                       │
│  2. 设计文档 / Design Document                                              │
│     ↓                                                                       │
│  3. 创建分支 / Create Branch (feature/xxx)                                 │
│     ↓                                                                       │
│  4. 编码实现 / Implementation                                               │
│     ↓                                                                       │
│  5. 单元测试 / Unit Testing                                                 │
│     ↓                                                                       │
│  6. 集成测试 / Integration Testing                                          │
│     ↓                                                                       │
│  7. 代码审查 / Code Review                                                  │
│     ↓                                                                       │
│  8. 合并分支 / Merge Branch                                                 │
│     ↓                                                                       │
│  9. 更新文档 / Update Documentation                                         │
│     ↓                                                                       │
│  10. 部署发布 / Deploy Release                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 2.3 测试规范 / Testing Standards

每次完成项目开发后，必须执行完整的测试流程并记录测试结果。

#### 2.3.1 单元测试 / Unit Testing

```typescript
// src/__tests__/lib/dock-arc.test.ts
/**
 * @file dock-arc.test.ts
 * @description Dock 弧线计算单元测试 / Dock arc calculation unit tests
 * @author Developer
 * @created 2026-08-25
 */

import { describe, it, expect } from 'vitest'
import { calculateDockPositions } from '@/lib/dock-arc'

describe('Dock 弧线计算 / Dock Arc Calculation', () => {
  // 测试基本功能 / Test basic functionality
  it('应正确计算 9 个插件的位置 / Should correctly calculate positions for 9 plugins', () => {
    const positions = calculateDockPositions(9, 0)
    
    // 验证返回数组长度 / Verify array length
    expect(positions).toHaveLength(9)
    
    // 验证中心位置激活 / Verify center position is active
    expect(positions[4].isActive).toBe(true)
    
    // 验证坐标范围 / Verify coordinate range
    positions.forEach(pos => {
      expect(pos.x).toBeGreaterThan(0)
      expect(pos.x).toBeLessThan(320)
      expect(pos.y).toBeGreaterThan(0)
      expect(pos.y).toBeLessThan(560)
    })
  })
  
  // 测试边界情况 / Test edge cases
  it('应处理偏移量超出范围的情况 / Should handle offset out of range', () => {
    const positions = calculateDockPositions(9, 100)
    expect(positions[0].actualIndex).toBe(1) // 100 % 9 = 1
  })
  
  // 测试性能 / Test performance
  it('应在 16ms 内完成计算 / Should complete calculation within 16ms', () => {
    const start = performance.now()
    calculateDockPositions(9, 0)
    const duration = performance.now() - start
    expect(duration).toBeLessThan(16) // 60fps 要求 / 60fps requirement
  })
})
```

#### 2.3.2 集成测试 / Integration Testing

```typescript
// src/__tests__/integration/plugin-manager.test.ts
/**
 * @file plugin-manager.test.ts
 * @description 插件管理器集成测试 / Plugin manager integration tests
 * @author Developer
 * @created 2026-08-25
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest'
import { initDb, getDb } from '@/lib/db'
import { pluginDb } from '@/lib/db'

describe('插件管理器集成测试 / Plugin Manager Integration Tests', () => {
  // 测试前初始化 / Setup before tests
  beforeAll(async () => {
    await initDb()
  })
  
  // 测试后清理 / Cleanup after tests
  afterAll(async () => {
    const db = getDb()
    await db.execute('DELETE FROM plugins WHERE id LIKE "test-%"')
  })
  
  // 测试插件安装和查询 / Test plugin installation and query
  it('应正确安装并查询插件 / Should install and query plugin correctly', async () => {
    const testPlugin = {
      id: 'test-plugin-1',
      name: '测试插件 / Test Plugin',
      version: '1.0.0',
      author: '测试作者 / Test Author',
      window_mode: 'embedded'
    }
    
    // 安装 / Install
    await pluginDb.save(testPlugin)
    
    // 查询 / Query
    const retrieved = await pluginDb.getById('test-plugin-1')
    expect(retrieved).toBeDefined()
    expect(retrieved?.name).toBe('测试插件 / Test Plugin')
    expect(retrieved?.window_mode).toBe('embedded')
  })
  
  // 测试插件删除 / Test plugin deletion
  it('应正确删除插件 / Should delete plugin correctly', async () => {
    await pluginDb.delete('test-plugin-1')
    const retrieved = await pluginDb.getById('test-plugin-1')
    expect(retrieved).toBeNull()
  })
})
```

#### 2.3.3 端到端测试 / End-to-End Testing

```typescript
// src/__tests__/e2e/dock-flow.test.ts
/**
 * @file dock-flow.test.ts
 * @description Dock 用户交互端到端测试 / Dock user interaction E2E tests
 * @author Developer
 * @created 2026-08-25
 */

import { describe, it, expect, beforeAll } from 'vitest'
import { app } from '@tauri-apps/api'

describe('Dock 用户交互流程 / Dock User Interaction Flow', () => {
  // 测试前准备 / Setup
  beforeAll(async () => {
    // 启动应用 / Launch application
    await app.launch()
    // 等待窗口加载 / Wait for window to load
    await new Promise(resolve => setTimeout(resolve, 1000))
  })
  
  // 测试 Dock 展开和收起 / Test Dock expand and collapse
  it('应正确展开和收起 Dock / Should expand and collapse Dock correctly', async () => {
    // 模拟鼠标进入热区 / Simulate mouse entering hotzone
    // 验证 Dock 展开 / Verify Dock is expanded
    // 模拟鼠标离开 / Simulate mouse leaving
    // 验证 Dock 收起 / Verify Dock is collapsed
  })
  
  // 测试插件启动 / Test plugin launch
  it('应正确启动插件 / Should launch plugin correctly', async () => {
    // 点击插件图标 / Click plugin icon
    // 验证插件窗口打开 / Verify plugin window is opened
    // 验证插件状态 / Verify plugin state
  })
})
```

### 2.4 测试报告模板 / Test Report Template

每次测试完成后，必须生成以下格式的测试报告：

```markdown
# Kapi 测试报告 / Kapi Test Report

**测试日期 / Test Date**: YYYY-MM-DD
**测试人员 / Tester**: 姓名 / Name
**测试版本 / Test Version**: vX.X.X
**测试环境 / Test Environment**: 
- OS: Windows 11 / macOS 14 / Ubuntu 22.04
- Node: v18.x
- Rust: 1.75+

## 测试结果汇总 / Test Results Summary

| 测试类型 / Test Type | 总用例 / Total | 通过 / Passed | 失败 / Failed | 通过率 / Pass Rate |
|---------------------|---------------|--------------|--------------|-------------------|
| 单元测试 / Unit Tests | 45 | 45 | 0 | 100% |
| 集成测试 / Integration Tests | 23 | 22 | 1 | 95.6% |
| 端到端测试 / E2E Tests | 12 | 12 | 0 | 100% |
| **总计 / Total** | **80** | **79** | **1** | **98.75%** |

## 详细测试结果 / Detailed Test Results

### 单元测试 / Unit Tests ✅

| 测试模块 / Module | 用例数 | 状态 / Status |
|------------------|--------|--------------|
| dock-arc.ts | 12 | ✅ 全部通过 / All passed |
| db.ts | 8 | ✅ 全部通过 / All passed |
| workflow-engine.ts | 15 | ✅ 全部通过 / All passed |
| plugin-manager.ts | 10 | ✅ 全部通过 / All passed |

### 集成测试 / Integration Tests ⚠️

| 测试模块 / Module | 用例数 | 状态 / Status |
|------------------|--------|--------------|
| plugin-db | 8 | ✅ 全部通过 / All passed |
| workflow-db | 7 | ✅ 全部通过 / All passed |
| dock-service | 8 | ⚠️ 1 个失败 / 1 failed |

**失败详情 / Failure Details**:
- `dock-service` > `should handle concurrent mouse events`
  - 错误 / Error: 并发事件处理超时 / Concurrent event handling timeout
  - 原因 / Cause: 事件锁竞争导致死锁 / Event lock contention causing deadlock
  - 修复方案 / Fix: 优化锁粒度，使用异步互斥锁 / Optimize lock granularity, use async mutex

### 端到端测试 / E2E Tests ✅

| 测试场景 / Scenario | 状态 / Status |
|--------------------|--------------|
| Dock 展开/收起 / Dock expand/collapse | ✅ 通过 / Passed |
| 插件启动 / Plugin launch | ✅ 通过 / Passed |
| 工作流执行 / Workflow execution | ✅ 通过 / Passed |
| 窗口切换 / Window switching | ✅ 通过 / Passed |

## 性能测试 / Performance Tests

| 指标 / Metric | 目标值 / Target | 实际值 / Actual | 状态 / Status |
|--------------|----------------|----------------|--------------|
| 应用启动时间 / App Launch Time | < 3s | 2.1s | ✅ |
| Dock 展开动画 / Dock Expand Animation | 60fps | 60fps | ✅ |
| 插件加载时间 / Plugin Load Time | < 500ms | 320ms | ✅ |
| 工作流执行时间 / Workflow Execution | < 1s | 650ms | ✅ |
| 数据库查询时间 / DB Query Time | < 100ms | 45ms | ✅ |

## 问题列表 / Issue List

| ID | 严重程度 / Severity | 描述 / Description | 状态 / Status | 负责人 / Assignee |
|----|--------------------|-------------------|--------------|------------------|
| KAPI-001 | 中等 / Medium | 并发事件处理超时 / Concurrent event timeout | 修复中 / Fixing | @dev |
| KAPI-002 | 低 / Low | 日志记录不完整 / Incomplete logging | 待修复 / Pending | @dev |

## 建议 / Recommendations

1. 优化事件锁机制，避免死锁 / Optimize event locking mechanism to avoid deadlock
2. 增加日志记录，便于调试 / Add more logging for debugging
3. 考虑添加性能监控 / Consider adding performance monitoring

## 结论 / Conclusion

**测试状态**: ⚠️ 有条件通过 / Conditional Pass
**批准人**: 项目负责人 / Project Lead
**签名**: _____________
**日期**: YYYY-MM-DD
```

---

## 三、Git 规范 / Git Standards

### 3.1 分支策略 / Branch Strategy

```
main (生产环境 / Production)
  └── develop (开发环境 / Development)
       ├── feature/xxx (功能分支 / Feature branch)
       ├── fix/xxx (修复分支 / Fix branch)
       ├── release/xxx (发布分支 / Release branch)
       └── hotfix/xxx (紧急修复 / Hotfix)
```

### 3.2 提交规范 / Commit Standards

```
<type>(<scope>): <subject> / <subject-en>

类型 / Type:
- feat: 新功能 / New feature
- fix: Bug 修复 / Bug fix
- docs: 文档更新 / Documentation
- style: 代码格式 / Code style
- refactor: 重构 / Refactor
- perf: 性能优化 / Performance
- test: 测试 / Testing
- chore: 构建/工具 / Build/Tools

示例 / Example:
feat(dock): 实现弧形布局 / Implement arc layout
fix(plugin): 修复窗口关闭后状态未更新 / Fix state not updated after window close
docs(readme): 更新安装说明 / Update installation guide
test(workflow): 添加工作流执行测试 / Add workflow execution tests
refactor(db): 优化数据库连接池 / Optimize database connection pool
perf(dock): 优化弧线计算性能 / Optimize arc calculation performance
chore(deps): 更新依赖版本 / Update dependencies
```

### 3.3 PR 模板 / PR Template

```markdown
## PR 描述 / PR Description

### 变更类型 / Change Type
- [ ] 新功能 / New feature
- [ ] Bug 修复 / Bug fix
- [ ] 代码重构 / Code refactor
- [ ] 性能优化 / Performance optimization
- [ ] 文档更新 / Documentation update
- [ ] 测试 / Testing

### 变更内容 / Changes
<!-- 简要描述变更内容 / Briefly describe the changes -->

### 测试 / Testing
- [ ] 单元测试通过 / Unit tests passed
- [ ] 集成测试通过 / Integration tests passed
- [ ] 手动测试通过 / Manual testing passed

### 相关 Issue / Related Issues
- Closes #ISSUE_ID
- Related #ISSUE_ID

### 截图 / Screenshots
<!-- 如有 UI 变更，请提供截图 / If UI changes, provide screenshots -->

### 检查清单 / Checklist
- [ ] 代码符合规范 / Code follows style guidelines
- [ ] 添加了必要的测试 / Added necessary tests
- [ ] 更新了相关文档 / Updated relevant documentation
- [ ] 自检通过 / Self-review completed
- [ ] 无性能问题 / No performance issues

### 备注 / Notes
<!-- 其他需要说明的内容 / Other information -->
```

---

## 四、项目完成检查清单 / Project Completion Checklist

每次完成项目阶段或发布版本时，必须完成以下检查：

```markdown
## 项目完成检查清单 / Project Completion Checklist

### 代码质量 / Code Quality
- [ ] 所有代码已通过 Lint 检查 / All code passed lint checks
- [ ] 所有代码已通过格式化检查 / All code passed formatting checks
- [ ] 无 TypeScript/Rust 类型错误 / No TypeScript/Rust type errors
- [ ] 代码覆盖率 ≥ 80% / Code coverage ≥ 80%
- [ ] 无 TODO 注释遗留 / No TODO comments left
- [ ] 无 console.log/debug 遗留 / No console.log/debug left

### 测试 / Testing
- [ ] 单元测试全部通过 / All unit tests passed
- [ ] 集成测试全部通过 / All integration tests passed
- [ ] 端到端测试全部通过 / All E2E tests passed
- [ ] 性能测试达标 / Performance tests met targets
- [ ] 跨平台测试通过 / Cross-platform tests passed
  - [ ] Windows
  - [ ] macOS
  - [ ] Linux

### 文档 / Documentation
- [ ] API 文档已更新 / API documentation updated
- [ ] 用户手册已更新 / User manual updated
- [ ] README 已更新 / README updated
- [ ] CHANGELOG 已更新 / CHANGELOG updated
- [ ] 测试报告已生成 / Test report generated

### 部署 / Deployment
- [ ] 构建成功 / Build successful
- [ ] 安装包测试通过 / Installer package tested
- [ ] 签名已完成 / Signature completed
- [ ] 发布说明已准备 / Release notes prepared

### 安全 / Security
- [ ] 无已知安全漏洞 / No known security vulnerabilities
- [ ] 依赖已更新到安全版本 / Dependencies updated to secure versions
- [ ] 敏感信息未硬编码 / No hardcoded sensitive information
- [ ] 权限配置正确 / Permission configuration correct

### 性能 / Performance
- [ ] 应用启动时间 < 3s / App launch time < 3s
- [ ] 内存使用 < 200MB / Memory usage < 200MB
- [ ] CPU 使用率 < 20% (空闲) / CPU usage < 20% (idle)
- [ ] 数据库查询 < 100ms / Database queries < 100ms
- [ ] 动画帧率 ≥ 60fps / Animation frame rate ≥ 60fps
```

---

## 五、文档更新规范 / Documentation Update Standards

### 5.1 更新时机 / Update Timing

| 变更类型 / Change Type | 文档更新要求 / Documentation Update Required |
|----------------------|---------------------------------------------|
| 新功能 / New feature | ✅ 必须更新 / Must update |
| Bug 修复 / Bug fix | ⚠️ 影响 API 时更新 / Update if API affected |
| API 变更 / API change | ✅ 必须更新 / Must update |
| 配置变更 / Config change | ✅ 必须更新 / Must update |
| 性能优化 / Performance | ⚠️ 大幅变化时更新 / Update if significant |
| 文档错误 / Doc error | ✅ 立即修复 / Fix immediately |

### 5.2 文档结构 / Document Structure

```
docs/
├── README.md                    # 项目总览 / Project overview
├── CHANGELOG.md                 # 版本变更记录 / Version changelog
├── CONTRIBUTING.md              # 贡献指南 / Contributing guide
├── ARCHITECTURE.md              # 架构设计 / Architecture design
├── API.md                       # API 文档 / API documentation
├── DEVELOPER.md                 # 开发者指南 / Developer guide
├── USER_GUIDE.md                # 用户手册 / User manual
├── DEPLOYMENT.md                # 部署指南 / Deployment guide
├── tests/
│   ├── TEST_PLAN.md             # 测试计划 / Test plan
│   ├── TEST_CASES.md            # 测试用例 / Test cases
│   └── TEST_REPORTS/            # 测试报告 / Test reports
│       ├── YYYY-MM-DD_report.md
│       └── YYYY-MM-DD_report.md
└── migrations/
    ├── 001_init.sql
    ├── 002_workflow.sql
    └── 003_plugin_data.sql
```

### 5.3 CHANGELOG 模板 / CHANGELOG Template

```markdown
# Kapi 更新日志 / Kapi Changelog

## [v1.0.0] - YYYY-MM-DD

### 新增功能 / Added
- **Dock 侧边栏**: 实现弧形布局，支持鼠标悬停展开/收起 / Implement arc layout with hover expand/collapse
- **插件系统**: 支持 WASM 插件加载，三种窗口模式 / Support WASM plugin loading, three window modes
- **工作流引擎**: 实现插件间数据联动和自动化编排 / Implement data linkage and automation orchestration between plugins
- **本地数据库**: SQLite 存储所有应用数据 / SQLite for all application data storage

### 修复 / Fixed
- 修复 Windows 透明窗口黑屏问题 / Fix transparent window black screen on Windows
- 修复 Dock 展开动画闪烁问题 / Fix Dock expand animation flickering

### 性能优化 / Performance
- 优化弧线计算性能，提升至 60fps / Optimize arc calculation performance to 60fps
- 优化数据库查询，添加索引 / Optimize database queries with indexes

### 安全 / Security
- 添加插件签名验证 / Add plugin signature verification
- 添加 WASM 沙箱权限控制 / Add WASM sandbox permission control

### 已知问题 / Known Issues
- 并发事件处理偶发超时 / Occasional timeout in concurrent event handling (#KAPI-001)
- 日志记录不完整 / Incomplete logging (#KAPI-002)

### 贡献者 / Contributors
- @username (贡献内容 / Contribution)
- @username (贡献内容 / Contribution)
```

---

## 六、测试记录文档模板 / Test Record Document Template

每次测试完成后，必须将测试结果记录到 `docs/tests/TEST_REPORTS/` 目录。

```markdown
# Kapi 测试记录 / Kapi Test Record

**记录编号 / Record ID**: TR-YYYYMMDD-XXX
**测试日期 / Test Date**: YYYY-MM-DD
**测试人员 / Tester**: 姓名 / Name
**测试版本 / Test Version**: vX.X.X
**测试范围 / Test Scope**: 功能/性能/安全/集成 / Feature/Performance/Security/Integration

---

## 一、测试环境 / Test Environment

| 项目 / Item | 详情 / Details |
|------------|---------------|
| 操作系统 / OS | Windows 11 Pro 22H2 |
| CPU | Intel Core i7-12700K |
| 内存 / RAM | 32GB DDR4 |
| 显卡 / GPU | NVIDIA RTX 3060 |
| Rust 版本 | 1.75.0 |
| Node 版本 | 18.17.0 |
| 数据库 / Database | SQLite 3.43.0 |

---

## 二、测试用例 / Test Cases

### 2.1 功能测试 / Functional Tests

| 用例 ID | 测试描述 / Description | 预期结果 / Expected | 实际结果 / Actual | 状态 / Status |
|---------|----------------------|-------------------|------------------|--------------|
| TC-001 | Dock 悬停展开 / Dock hover expand | Dock 展开显示插件 / Dock expanded with plugins | Dock 正常展开 / Dock expanded normally | ✅ PASS |
| TC-002 | Dock 离开收起 / Dock leave collapse | Dock 收起到触发器 / Dock collapsed to trigger | Dock 正常收起 / Dock collapsed normally | ✅ PASS |
| TC-003 | 插件内嵌启动 / Embedded plugin launch | 主面板显示插件 / Plugin shown in main panel | 插件正常显示 / Plugin displayed normally | ✅ PASS |
| TC-004 | 插件独立窗口 / Independent plugin window | 创建独立窗口 / Create independent window | 窗口正常创建 / Window created normally | ✅ PASS |
| TC-005 | 工作流执行 / Workflow execution | 插件按顺序执行 / Plugins executed in order | 工作流正常执行 / Workflow executed normally | ✅ PASS |
| TC-006 | 数据库读写 / Database read/write | 数据正确存储和读取 / Data stored and read correctly | 数据操作正常 / Data operations normal | ✅ PASS |

### 2.2 性能测试 / Performance Tests

| 用例 ID | 测试描述 / Description | 目标值 / Target | 实际值 / Actual | 状态 / Status |
|---------|----------------------|----------------|----------------|--------------|
| PT-001 | 应用启动时间 / App launch time | < 3s | 2.1s | ✅ PASS |
| PT-002 | Dock 展开动画帧率 / Dock expand FPS | ≥ 60fps | 60fps | ✅ PASS |
| PT-003 | 插件加载时间 / Plugin load time | < 500ms | 320ms | ✅ PASS |
| PT-004 | 工作流执行时间 / Workflow execution | < 1s | 650ms | ✅ PASS |
| PT-005 | 数据库查询时间 / DB query time | < 100ms | 45ms | ✅ PASS |
| PT-006 | 内存使用 / Memory usage | < 200MB | 156MB | ✅ PASS |

### 2.3 安全测试 / Security Tests

| 用例 ID | 测试描述 / Description | 预期结果 / Expected | 实际结果 / Actual | 状态 / Status |
|---------|----------------------|-------------------|------------------|--------------|
| ST-001 | 插件权限验证 / Plugin permission validation | 未授权操作被拒绝 / Unauthorized operations rejected | 权限验证正常 / Permission validation normal | ✅ PASS |
| ST-002 | WASM 沙箱隔离 / WASM sandbox isolation | 插件无法访问系统资源 / Plugin cannot access system resources | 隔离正常 / Isolation normal | ✅ PASS |
| ST-003 | 签名验证 / Signature verification | 未签名插件拒绝加载 / Unsigned plugins rejected | 验证正常 / Verification normal | ✅ PASS |

---

## 三、问题记录 / Issue Records

### 问题 1 / Issue 1

**ID**: KAPI-001  
**严重程度 / Severity**: 中等 / Medium  
**状态 / Status**: 修复中 / Fixing  
**发现时间 / Found**: 2026-08-25 14:30  
**发现人 / Found by**: @developer  

**描述 / Description**:
并发事件处理时出现超时，导致 Dock 状态更新延迟。
Timeout occurs during concurrent event processing, causing Dock state update delay.

**复现步骤 / Steps to Reproduce**:
1. 快速移动鼠标进入和离开 Dock 热区
2. 观察 Dock 状态切换
3. 出现状态不同步

**期望行为 / Expected Behavior**:
Dock 状态应立即切换，无延迟。
Dock state should switch immediately without delay.

**实际行为 / Actual Behavior**:
Dock 状态切换延迟 200-500ms。
Dock state switching delayed by 200-500ms.

**解决方案 / Solution**:
优化事件锁粒度，使用异步互斥锁代替同步锁。
Optimize event lock granularity, use async mutex instead of sync mutex.

---

## 四、测试结论 / Test Conclusion

**测试状态**: ✅ 通过 / PASS  
**测试覆盖率**: 87%  
**通过率**: 97.5%  
**批准人**: 项目负责人 / Project Lead  

**备注 / Notes**:
- 1 个问题正在修复，不影响核心功能
- 1 issue is being fixed, does not affect core functionality
- 建议继续监控性能指标
- Suggest continuing to monitor performance metrics

---

## 五、附件 / Attachments

- [ ] 测试日志 / Test logs
- [ ] 性能监控截图 / Performance monitoring screenshots
- [ ] 错误堆栈 / Error stacks
- [ ] 覆盖率报告 / Coverage report
```

---

## 七、开发工具配置 / Development Tool Configuration

### 7.1 ESLint 配置 / ESLint Configuration

```javascript
// .eslintrc.cjs
module.exports = {
  root: true,
  env: { browser: true, es2020: true },
  extends: [
    'eslint:recommended',
    'plugin:@typescript-eslint/recommended',
    'plugin:react-hooks/recommended',
  ],
  ignorePatterns: ['dist', '.eslintrc.cjs'],
  parser: '@typescript-eslint/parser',
  plugins: ['react-refresh'],
  rules: {
    'react-refresh/only-export-components': [
      'warn',
      { allowConstantExport: true },
    ],
    '@typescript-eslint/no-unused-vars': ['warn', { 
      argsIgnorePattern: '^_',
      varsIgnorePattern: '^_' 
    }],
    'no-console': ['warn', { allow: ['warn', 'error'] }],
    'no-debugger': 'warn',
  },
}
```

### 7.2 Prettier 配置 / Prettier Configuration

```json
// .prettierrc
{
  "semi": false,
  "singleQuote": true,
  "tabWidth": 2,
  "trailingComma": "es5",
  "printWidth": 100,
  "endOfLine": "auto",
  "arrowParens": "always",
  "bracketSpacing": true
}
```

### 7.3 配置文件 / Configuration Files

```json
// tsconfig.json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": {
      "@/*": ["src/*"]
    }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

---

## 八、紧急情况处理 / Emergency Handling

### 8.1 紧急修复流程 / Hotfix Process

1. 从 `main` 分支创建 `hotfix/xxx` 分支
2. 修复问题 / Fix the issue
3. 添加测试 / Add tests
4. 更新文档 / Update documentation
5. 代码审查 / Code review
6. 合并到 `main` 和 `develop`
7. 发布新版本 / Release new version

### 8.2 回滚策略 / Rollback Strategy

- **小问题 / Minor Issue**: 修复后发布补丁版本 / Fix and release patch version
- **严重问题 / Major Issue**: 立即回滚到上一个稳定版本 / Immediately rollback to previous stable version
- **数据问题 / Data Issue**: 从备份恢复数据 / Restore from backup

---

**文档版本**: v1.0.0  
**最后更新**: 2026-08-25  
**维护者**: Kapi 开发团队 / Kapi Development Team