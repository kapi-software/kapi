// Dock 服务：Windows 光标轮询、状态机与鼠标穿透（docs/DOCK.md §3–§4）
// Dock service: Windows cursor polling, state machine and pass-through (docs/DOCK.md §3–§4)
// 状态权威在此（Rust），渲染层仅被动接收 dock:state 事件
// The state authority lives here (Rust); the renderer only reacts to dock:state events
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

// Dock 窗口固定尺寸（docs/DOCK.md §1：永不 resize）
// Fixed dock window size (docs/DOCK.md §1: never resized)
const DOCK_W: i32 = 320;
const DOCK_H: i32 = 560;

// 轮询与热区参数（docs/DOCK.md §3–§4）
// Polling and hotzone parameters (docs/DOCK.md §3–§4)
const POLL_INTERVAL_MS: u64 = 100;
const COOLDOWN_MS: u64 = 120;
const HOTZONE_WIDTH: i32 = 12;

// 前端推送的 Dock 配置（settings 表镜像，经 dock_set_config 命令进入）
// Dock config pushed by the frontend (mirror of the settings table via dock_set_config)
#[derive(Clone)]
pub struct DockConfig {
    pub enabled: bool,
    pub auto_hide_ms: u64,
}

impl Default for DockConfig {
    fn default() -> Self {
        // 与前端 DEFAULT_SETTINGS.dock_auto_hide_delay 一致
        // Matches DEFAULT_SETTINGS.dock_auto_hide_delay on the frontend
        Self { enabled: false, auto_hide_ms: 3000 }
    }
}

// 状态机（docs/DOCK.md §3）：Peeking 为收起过渡态，光标离开条带后转 Hidden
// State machine (docs/DOCK.md §3): Peeking is the collapse transition; becomes Hidden once the cursor leaves the strip
#[derive(Clone, Copy, PartialEq)]
enum DockState {
    Hidden,
    Peeking,
    Expanded,
}

// 启动入口：定位 Dock 窗口并启动轮询线程
// Entry point: position the dock window and start the polling thread
pub fn start(app: AppHandle) {
    // 定位一次：光标所在显示器 workArea 右缘、垂直居中，之后不再移动（docs/DOCK.md §1）
    // Position once: right edge of the cursor's monitor workArea, vertically centered
    if let Some(dock) = app.get_webview_window("dock") {
        if let Some((x, y)) = initial_position() {
            let _ = dock.set_position(PhysicalPosition::new(x, y));
        }
    }
    std::thread::spawn(move || poll_loop(app));
}

// 轮询主循环：100ms 边沿触发热区检测（docs/DOCK.md §4 伪代码对齐）
// Polling loop: 100ms edge-triggered hotzone detection (aligned with docs/DOCK.md §4)
fn poll_loop(app: AppHandle) {
    let mut state = DockState::Hidden;
    let mut was_in_hotzone = false;
    // 收起态初始即穿透；pass_through 只用于幂等去重，避免每轮调系统 API
    // Collapsed starts pass-through; pass_through only dedupes system calls
    let mut pass_through = true;
    let mut last_change = Instant::now();
    let mut hide_deadline: Option<Instant> = None;

    loop {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

        let Some(dock) = app.get_webview_window("dock") else { continue };
        let config = app.state::<Mutex<DockConfig>>().inner().lock().unwrap().clone();

        // §3.5 启用开关：禁用 → 收起并整窗隐藏；轮询重置热区观察值
        // §3.5 enable switch: disable → collapse and hide; reset the hotzone observation
        if !config.enabled {
            state = DockState::Hidden;
            hide_deadline = None;
            if dock.is_visible().unwrap_or(false) {
                let _ = dock.hide();
            }
            was_in_hotzone = false;
            continue;
        }

        // 重新启用 → show 并回到收起态（初始穿透）
        // Re-enabled → show and return to the collapsed state (pass-through on)
        if !dock.is_visible().unwrap_or(false) {
            let _ = dock.show();
            let _ = dock.set_ignore_cursor_events(true);
            pass_through = true;
            state = DockState::Hidden;
            was_in_hotzone = false;
            last_change = Instant::now();
        }

        // §3.4 冷却期：状态变更后 120ms 内跳过热区检测（防边界抖动）
        // §3.4 cooldown: skip hotzone detection for 120ms after a state change
        if last_change.elapsed() < Duration::from_millis(COOLDOWN_MS) {
            continue;
        }

        let Some((cx, cy)) = cursor_position() else { continue };
        let Ok(pos) = dock.outer_position() else { continue };

        let in_window_y = cy >= pos.y && cy <= pos.y + DOCK_H;
        // 热区几何随状态变化：收起态仅右缘条带，展开态整窗（docs/DOCK.md §4）
        // Hotzone geometry varies by state: collapsed = right strip only, expanded = whole window
        let in_hotzone = match state {
            DockState::Hidden | DockState::Peeking => {
                in_window_y && cx >= pos.x + DOCK_W - HOTZONE_WIDTH && cx <= pos.x + DOCK_W
            }
            DockState::Expanded => in_window_y && cx >= pos.x && cx <= pos.x + DOCK_W,
        };

        match (state, in_hotzone, was_in_hotzone) {
            // 上升沿：展开，第一动作 = 关闭穿透（§3.3）
            // Rising edge: expand; the first action is disabling pass-through (§3.3)
            (DockState::Hidden, true, false) => {
                let _ = dock.set_ignore_cursor_events(false);
                pass_through = false;
                state = DockState::Expanded;
                last_change = Instant::now();
                hide_deadline = Some(Instant::now() + Duration::from_millis(config.auto_hide_ms));
                let _ = app.emit("dock:state", "expanded");
            }
            // 收起态不在热区：幂等恢复穿透（§3.2 收起瞬间不立即恢复）
            // Collapsed and outside the hotzone: idempotently restore pass-through (§3.2)
            (DockState::Hidden, false, _) => {
                if !pass_through {
                    let _ = dock.set_ignore_cursor_events(true);
                    pass_through = true;
                }
            }
            // 下降沿：光标离开整窗 → 收起（过渡态 Peeking，穿透待光标离开条带后恢复）
            // Falling edge: cursor left the window → collapse (Peeking transition)
            (DockState::Expanded, false, true) => {
                state = DockState::Peeking;
                hide_deadline = None;
                last_change = Instant::now();
                let _ = app.emit("dock:state", "hidden");
            }
            _ => {}
        }

        // Peeking：光标离开右缘条带 → 转正式 Hidden（下轮由上面分支恢复穿透）
        // Peeking: cursor left the strip → Hidden (the branch above restores pass-through next tick)
        if state == DockState::Peeking && !in_hotzone {
            state = DockState::Hidden;
            last_change = Instant::now();
        }

        // §3 延迟自动收起：dock_auto_hide_delay 到期（光标可能仍在窗口内）
        // §3 delayed auto-collapse: dock_auto_hide_delay elapsed (cursor may still be inside)
        if state == DockState::Expanded && hide_deadline.is_some_and(|d| Instant::now() >= d) {
            state = DockState::Peeking;
            hide_deadline = None;
            last_change = Instant::now();
            let _ = app.emit("dock:state", "hidden");
        }

        // 每轮无条件刷新，防外部改状态后卡死（docs/DOCK.md §4）
        // Refresh unconditionally every tick (docs/DOCK.md §4)
        was_in_hotzone = in_hotzone;
    }
}

// 前端推送 Dock 配置：设置页变更实时生效（docs/PANEL.md §4.1）
// Frontend pushes dock config: settings changes apply live (docs/PANEL.md §4.1)
// 命令必须 pub：generate_handler! 需要模块内可见的 __cmd__ 宏
// Commands must be pub: generate_handler! needs the __cmd__ macros visible
#[tauri::command]
pub fn dock_set_config(
    config: tauri::State<'_, Mutex<DockConfig>>,
    enabled: bool,
    auto_hide_ms: u64,
) {
    let mut c = config.lock().unwrap();
    c.enabled = enabled;
    c.auto_hide_ms = auto_hide_ms;
}

// 插件唤醒分发：Phase 3 仅聚焦主面板；Phase 4 按 window_mode 分发（docs/PLUGINS.md）
// Plugin wake dispatch: Phase 3 only focuses the panel; Phase 4 dispatches by window_mode
#[tauri::command]
pub async fn launch_plugin(app: AppHandle, _plugin_id: String) -> Result<(), String> {
    if let Some(main) = app.get_webview_window("main") {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============================================================
// 平台光标与显示器（docs/DOCK.md §4 平台表）
// Platform cursor and monitor (platform table in docs/DOCK.md §4)
// ============================================================

// Windows：GetCursorPos + MonitorFromPoint / GetMonitorInfoW
// Windows: GetCursorPos + MonitorFromPoint / GetMonitorInfoW
#[cfg(windows)]
fn cursor_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::POINT;
    // windows-sys 0.48 中 GetCursorPos 位于 WindowsAndMessaging
    // GetCursorPos lives in WindowsAndMessaging in windows-sys 0.48
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let mut pt = POINT { x: 0, y: 0 };
    // SAFETY: 传入合法指针；GetCursorPos 无副作用
    // SAFETY: valid pointer passed; GetCursorPos has no side effects
    unsafe {
        if GetCursorPos(&mut pt) != 0 {
            Some((pt.x, pt.y))
        } else {
            None
        }
    }
}

// Windows：光标所在显示器 workArea 右缘、垂直居中（创建时一次）
// Windows: right edge of the cursor's monitor workArea, vertically centered (once at creation)
#[cfg(windows)]
fn initial_position() -> Option<(i32, i32)> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };

    let (cx, cy) = cursor_position()?;
    // SAFETY: MONITORINFO 按契约先填 cbSize；句柄来自系统 API
    // SAFETY: cbSize filled per contract; the handle comes from a system API
    unsafe {
        // windows-sys 0.48 的 HANDLE 为 isize：0 即空句柄
        // HANDLE is isize in windows-sys 0.48: 0 means null
        let monitor = MonitorFromPoint(
            windows_sys::Win32::Foundation::POINT { x: cx, y: cy },
            MONITOR_DEFAULTTONEAREST,
        );
        if monitor == 0 {
            return None;
        }
        let zero_rect = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: zero_rect,
            rcWork: zero_rect,
            dwFlags: 0,
        };
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return None;
        }
        let work = info.rcWork;
        let x = work.right - DOCK_W;
        let y = work.top + (work.bottom - work.top - DOCK_H) / 2;
        Some((x, y))
    }
}

// TODO: macOS（CGEventTap + NSScreen）与 Linux X11（XQueryPointer + RandR）光标支持
// TODO: macOS (CGEventTap + NSScreen) and Linux X11 (XQueryPointer + RandR) cursor support
// Wayland 无全局光标 API：降级为仅快捷键唤醒（docs/ROADMAP.md 风险表）
// Wayland has no global cursor API: fall back to shortcut-only wake (docs/ROADMAP.md risks)
#[cfg(not(windows))]
fn cursor_position() -> Option<(i32, i32)> {
    None
}

#[cfg(not(windows))]
fn initial_position() -> Option<(i32, i32)> {
    None
}
