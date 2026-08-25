# Dock 侧边栏

Kapi 技术文档：Dock 窗口参数、弧线几何、状态机与热区轮询。

> 本文档已与 Electron 版源码逐项对齐：`kapi-main/src/main/dock.ts`（窗口与热区轮询）、
> `window-manager.ts`（状态机与穿透策略）、`renderer/src/dock/Dock.tsx` + `dock.css`（前端）、
> `renderer/src/lib/arc-math.ts`（弧线数学）、`shared/types.ts`（DockState）。
> 其余模块（插件系统/数据库/工作流）为 Tauri 版全新设计，不移植 Electron 实现。

## 1. 窗口参数

| 参数 | 值 | 说明 |
| ---- | -- | ---- |
| 尺寸 | 320 × 560 | 固定，`resizable: false`、`movable: false` |
| 位置 | 光标所在显示器 workArea 右缘、垂直居中 | **创建时一次计算**（Rust 启动时定位），之后不动 |
| 透明 | `transparent: true` | — |
| 无边框 | `decorations: false` | — |
| 阴影 | `shadow: false` | — |
| 置顶 | `alwaysOnTop: true` | Electron 用 'screen-saver' 级别 |
| 任务栏 | `skipTaskbar: true` | — |
| 初始可见 | `visible: false` | 加载完成后按 `dock_enabled` 设置决定是否 show |

**核心原则：展开/收起永远不做 setBounds / resize**——透明窗口 resize 会整窗闪烁（Electron 踩坑结论）。窗口大小与位置终身固定；收起态的"消失"靠**鼠标穿透**实现：整窗 `set_ignore_cursor_events(true)`，物理上只剩右缘 12px 箭头条带可交互，其余区域全部穿透。

## 2. 弧线几何与视觉规格

### 2.1 弧线数学

```text
圆心: C = (320, 280)          # 容器右缘 x + 垂直中心
半径: R = 260
角度: θ ∈ [π/2 + 0.1π, 3π/2 − 0.1π]   # 左半弧，两端各留 10% 边距

插件位置:
  x = 320 + 260 × cos(θ)
  y = 280 + 260 × sin(θ)

可见数: VISIBLE_COUNT = 9（centerIndex = ⌊9/2⌋ = 4 为选中态）
```

纯函数位于 `src/lib/dock-arc.ts`，签名 `getPositionOnArc(index, total)`，配套单元测试。

### 2.2 滚轮循环滚动

```text
可见槽位 i ∈ [0, 9)，实际插件索引:
  actualIndex = ((scrollOffset + i) % total + total) % total
滚轮 deltaY > 0 → offset + 1，反之 −1（仅展开态响应，preventDefault）
```

### 2.3 视觉规格

| 元素 | 规格 |
| ---- | ---- |
| 箭头触发器 | 12 × 150 px，贴右缘垂直居中，左圆角 `12px 0 0 12px`；hover 展宽至 16px |
| 箭头图标 | chevron-left SVG，展开态旋转 180°（0.2s ease） |
| 弧形轨道 | SVG path `M 320 0 A 320 280 0 0 0 320 560`，虚线 `4 6`，`rgba(255,255,255,0.03)` |
| 插件项 | 64×64 圆形；选中放大 1.12 倍并显示 label（8px 胶囊）；hover 1.1 倍 |
| 选中态配色 | 蓝色系光晕 `rgba(0,122,255,*)`（与主题强调色联动可后置） |
| 动画 | **全部用 motion (framer-motion)，禁用 CSS transition/transform**：展开/收起 0.18s `cubic-bezier(0.22,1,0.36,1)`；箭头 0.12s 弹性 `[0.34,1.56,0.64,1]`（展宽 0.16s easeOut） |

## 3. 状态机

```text
状态: hidden | peeking | expanded
      # peeking 为过渡中间态，渲染层按收起处理（Dock.tsx toUiState）

状态归属: Rust 主进程持有唯一状态，渲染层被动接收 dock:state 事件，
          不维护本地状态副本（与 Electron window.kapi.onStateChange 同构）

转换:
  hidden → expanded    光标进入右缘 12px 热区（轮询上升沿，见 §4）
  expanded → hidden    光标离开窗口（轮询下降沿）或 点击窗口外部（渲染层通知）
  expanded → hidden    延迟自动收起 scheduleCollapse（默认 3000ms = dock_auto_hide_delay）
  Alt+Space 切换       Tauri 版新增（global-shortcut）；Electron 版无此入口
```

**鼠标穿透策略（移植重点，Electron 踩坑结论）**：

1. 收起态：整窗穿透（`set_ignore_cursor_events(true)`），热区展开完全依赖 Rust 轮询；
2. **收起瞬间不能立即恢复穿透**——光标可能还停在箭头上，立即穿透会吞掉下一次"再点一下展开"；由轮询确认光标离开 12px 条带后**幂等恢复**穿透；
3. 展开第一动作 = 关闭穿透；
4. 每次状态变更后进入 **120ms 冷却期**，期间跳过热区检测（防边界抖动）；
5. `dock_enabled = false` → 先收起再 `hide()` 整窗；轮询遇窗口不可见直接跳过并重置热区观察值；重新启用 → `show()`。

> **Tauri 移植差异**：Electron `setIgnoreMouseEvents(ignore, { forward: true })` 可向渲染层转发 mousemove（渲染层 hover 兜底展开）；Tauri `set_ignore_cursor_events` 无转发能力。**Rust 轮询是展开的唯一权威路径**——Electron 版穿透态下渲染层本就收不到事件（其注释原话："热区展开完全依赖这里的轮询兜底"），行为一致，无功能损失。

## 4. Rust 热区轮询（`dock_service.rs`）

100ms 轮询，**边沿触发**（非电平触发），对齐 `main/dock.ts startMouseCheck`：

```rust
// 伪代码
loop {
    sleep(100ms);
    if 距上次状态变更 < 120ms { continue; }            // 冷却期
    if dock 窗口隐藏（dock_enabled=false）{ was_in_hotzone = false; continue; }

    let cursor = 全局光标位置();                         // 跨平台 API 见下表
    let in_window_y = cursor.y ∈ [win.y, win.y + win.h];

    // 热区几何随状态变化：收起态只有右缘 12px 条带；展开态是整个窗口
    let in_hotzone = match state {
        Hidden   => in_window_y && cursor.x ∈ [win.right - 12, win.right],
        Expanded | Peeking => in_window_y && cursor.x ∈ [win.x, win.right],
    };

    match (state, in_hotzone, was_in_hotzone) {
        (Hidden, true, false)   => { expand(); 记录状态变更时间; }    // 上升沿
        (Hidden, false, _)      => { 恢复鼠标穿透(); }               // 幂等
        (Expanded, false, true) => { collapse(); 记录状态变更时间; }  // 下降沿
        _ => {}
    }
    was_in_hotzone = in_hotzone;   // 每轮无条件刷新（防外部改状态后卡死）
}
```

Electron `screen.getCursorScreenPoint()` 的 Rust 等价物：

| 平台 | 光标位置 | 显示器归属 |
| ---- | -------- | ---------- |
| Windows | `GetCursorPos` (Win32) | `MonitorFromPoint` → `GetMonitorInfo` 取 workArea |
| macOS | CGEventTap | NSScreen |
| Linux X11 | `XQueryPointer` | RandR |
| Linux Wayland | 不可用 | 降级为仅 Alt+Space 快捷键唤醒（见 ROADMAP.md） |

## 5. Dock 只负责唤醒

对齐需求与 Electron 现状的差异：

- **Electron 现状**：点击插件 = 滚动到中心位（`handlePluginClick` 仅 `setScrollOffset`），点击"设置"临时打开主面板；
- **Tauri 版目标**：点击 = 唤醒分发，一行核心调用：

```typescript
// Dock 点击处理（唯一职责：唤醒）
const handleDockClick = async (pluginId: string) => {
  await invoke('launch_plugin', { pluginId })   // 分发逻辑全部在 Rust（ARCHITECTURE.md §2.3）
}
```

Dock 前端职责收敛为：弧形布局渲染（motion 动画）、滚轮循环切换、点击唤醒、点击外部收起通知、接收 `dock:state` 事件驱动动画。**不做任何模式判断、不渲染插件内容。**
