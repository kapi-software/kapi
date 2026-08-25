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

// 轮询与冷却参数（docs/DOCK.md §3–§4）；热区宽度/展开延迟为可配置项，见 DockConfig
// Polling and cooldown parameters (docs/DOCK.md §3–§4); hotzone width and expand delay are configurable in DockConfig
const POLL_INTERVAL_MS: u64 = 100;
const COOLDOWN_MS: u64 = 120;

// Dock 贴靠边（settings.dock_position：right / left）
// Dock attach side (settings.dock_position: right / left)
#[derive(Clone, Copy, PartialEq)]
enum DockSide {
    Left,
    Right,
}

impl DockSide {
    fn parse(s: &str) -> Self {
        // 未知值一律回退右侧（与前端默认一致）
        // Unknown values fall back to right (frontend default)
        if s.eq_ignore_ascii_case("left") {
            DockSide::Left
        } else {
            DockSide::Right
        }
    }
}

// 前端推送的 Dock 配置（settings 表镜像，经 dock_set_config 命令进入）
// Dock config pushed by the frontend (mirror of the settings table via dock_set_config)
#[derive(Clone)]
pub struct DockConfig {
    pub enabled: bool,
    pub position: String,
    // 热区宽度（逻辑像素，settings.dock_hotzone_width，默认 12）
    // Hotzone width in logical pixels (settings.dock_hotzone_width, default 12)
    pub hotzone_width: i32,
    // 展开延迟（毫秒，settings.dock_expand_delay；0 = 立即展开）
    // Expand delay in ms (settings.dock_expand_delay; 0 = immediate)
    pub expand_delay_ms: u64,
}

impl Default for DockConfig {
    fn default() -> Self {
        // 与前端 DEFAULT_SETTINGS 对应项一致
        // Matches the corresponding DEFAULT_SETTINGS entries on the frontend
        Self {
            enabled: false,
            position: "right".into(),
            hotzone_width: 12,
            expand_delay_ms: 0,
        }
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
    // 初始定位：主显示器 workArea 右缘（默认侧）、垂直居中；运行期随 dock_position 实时切换
    // Initial placement: default side of the primary monitor workArea; repositions live with dock_position
    if let Some(dock) = app.get_webview_window("dock") {
        let scale = dock.scale_factor().unwrap_or(1.0);
        position_dock(&dock, DockSide::Right, scale);
    }
    std::thread::spawn(move || poll_loop(app));
}

// 轮询主循环：100ms 边沿触发热区检测（docs/DOCK.md §4 伪代码对齐）
// Polling loop: 100ms edge-triggered hotzone detection (aligned with docs/DOCK.md §4)
// 无自动收起：光标在 Dock 内即保持展开，离开才收（2026-08-25 需求变更）
// No auto-hide: stays expanded while the cursor is inside (changed 2026-08-25)
fn poll_loop(app: AppHandle) {
    let mut state = DockState::Hidden;
    let mut was_in_hotzone = false;
    // 收起态初始即穿透；pass_through 只用于幂等去重，避免每轮调系统 API
    // Collapsed starts pass-through; pass_through only dedupes system calls
    let mut pass_through = true;
    let mut last_change = Instant::now();
    // 已应用的贴靠边：None = 尚未定位，首轮即按当前配置定位
    // Applied attach side: None = not yet positioned; the first tick positions per config
    let mut applied_side: Option<DockSide> = None;
    // 展开延迟倒计时：上升沿置位，持续停留热区到期才真正展开，提前离开则取消
    // Expand-delay countdown: set on the rising edge, expands on expiry while dwelling; cancelled on early leave
    let mut pending_expand: Option<Instant> = None;

    loop {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));

        let Some(dock) = app.get_webview_window("dock") else { continue };
        let config = app.state::<Mutex<DockConfig>>().inner().lock().unwrap().clone();
        let side = DockSide::parse(&config.position);
        let scale = dock.scale_factor().unwrap_or(1.0);
        // 热区宽度：设置值（逻辑像素）× 缩放 → 物理像素，防御性夹取范围
        // Hotzone width: setting (logical px) × scale → physical px, defensively clamped
        let hotzone = (config.hotzone_width.clamp(1, 64) as f64 * scale).round() as i32;

        // dock_position 变更 → 主显示器上实时换边（docs/PANEL.md §4.2）
        // dock_position changed → reposition live on the primary monitor
        if applied_side != Some(side) {
            position_dock(&dock, side, scale);
            applied_side = Some(side);
            state = DockState::Hidden;
            was_in_hotzone = false;
            last_change = Instant::now();
            let _ = app.emit("dock:state", "hidden");
        }

        // §3.5 启用开关：禁用 → 收起并整窗隐藏；轮询重置热区观察值
        // §3.5 enable switch: disable → collapse and hide; reset the hotzone observation
        if !config.enabled {
            state = DockState::Hidden;
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

        // macOS 光标为逻辑坐标，需按窗口缩放换算成物理像素与 outer_position 对齐
        // macOS cursor is in points; convert to physical pixels to match outer_position
        let Some((cx, cy)) = cursor_position(scale) else { continue };
        let Ok(pos) = dock.outer_position() else { continue };

        let in_window_y = cy >= pos.y && cy <= pos.y + DOCK_H;
        // 收起态热区 = 贴靠边条带（dock_hotzone_width，右贴右条带、左贴左条带）；展开态 = 整窗
        // Collapsed hotzone = strip on the attached edge (dock_hotzone_width); expanded = the whole window
        let in_strip = match side {
            DockSide::Right => cx >= pos.x + DOCK_W - hotzone && cx <= pos.x + DOCK_W,
            DockSide::Left => cx >= pos.x && cx <= pos.x + hotzone,
        };
        let in_hotzone = match state {
            DockState::Hidden | DockState::Peeking => in_window_y && in_strip,
            DockState::Expanded => in_window_y && cx >= pos.x && cx <= pos.x + DOCK_W,
        };

        match (state, in_hotzone, was_in_hotzone) {
            // 上升沿：进入展开延迟倒计时（延迟 0 时由下方到期逻辑同轮立即展开）
            // Rising edge: start the expand-delay countdown (zero delay expands this tick below)
            (DockState::Hidden, true, false) => {
                pending_expand = Some(Instant::now() + Duration::from_millis(config.expand_delay_ms));
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

        // 展开延迟到期判定：持续停留在热区且状态仍为 Hidden 才真正展开
        // Expand-delay expiry: expand only if still dwelling in the hotzone and still Hidden
        if let Some(deadline) = pending_expand {
            if !in_hotzone || state != DockState::Hidden {
                // 提前离开热区（或状态已被外部改变）→ 取消本次展开意图
                // Left early (or state changed externally) → cancel the pending expand
                pending_expand = None;
            } else if Instant::now() >= deadline {
                pending_expand = None;
                // 第一动作 = 关闭穿透（§3.3）
                // The first action is disabling pass-through (§3.3)
                let _ = dock.set_ignore_cursor_events(false);
                pass_through = false;
                state = DockState::Expanded;
                last_change = Instant::now();
                let _ = app.emit("dock:state", "expanded");
            }
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
    position: String,
    hotzone_width: i32,
    expand_delay_ms: u64,
) {
    let mut c = config.lock().unwrap();
    c.enabled = enabled;
    c.position = position;
    c.hotzone_width = hotzone_width;
    c.expand_delay_ms = expand_delay_ms;
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

// 按贴靠边定位：主显示器 workArea 对应边、垂直居中
// Position by attach side: the matching edge of the primary monitor workArea, vertically centered
fn position_dock(dock: &tauri::WebviewWindow, side: DockSide, scale: f64) {
    if let Some((wx, wy, ww, wh)) = primary_workarea(scale) {
        let x = match side {
            DockSide::Right => wx + ww - DOCK_W,
            DockSide::Left => wx,
        };
        let y = wy + (wh - DOCK_H) / 2;
        let _ = dock.set_position(PhysicalPosition::new(x, y));
    }
}

// Windows：GetCursorPos 全局光标（物理像素）
// Windows: GetCursorPos global cursor (physical pixels)
#[cfg(windows)]
fn cursor_position(_scale: f64) -> Option<(i32, i32)> {
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

// Windows：主显示器 workArea（含 (0,0) 的显示器，MONITOR_DEFAULTTOPRIMARY 兜底）
// Windows: primary monitor workArea (the one containing (0,0); MONITOR_DEFAULTTOPRIMARY fallback)
#[cfg(windows)]
fn primary_workarea(_scale: f64) -> Option<(i32, i32, i32, i32)> {
    use windows_sys::Win32::Foundation::{POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
    };

    // SAFETY: MONITORINFO 按契约先填 cbSize；句柄来自系统 API
    // SAFETY: cbSize filled per contract; the handle comes from a system API
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
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
        Some((work.left, work.top, work.right - work.left, work.bottom - work.top))
    }
}

// macOS 最小 CoreGraphics C ABI 绑定：光标 + 主显示器 bounds（零第三方依赖）
// Minimal macOS CoreGraphics C ABI bindings: cursor + main display bounds (zero deps)
#[cfg(target_os = "macos")]
mod macos_api {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }

    #[repr(C)]
    pub struct CGSize {
        pub width: f64,
        pub height: f64,
    }

    #[repr(C)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        // 传 nil source 创建只读事件快照，用于读取当前光标位置
        // nil source creates a read-only event snapshot to read the cursor position
        pub fn CGEventCreate(source: *const c_void) -> *mut c_void;
        pub fn CGEventGetLocation(event: *const c_void) -> CGPoint;
        pub fn CGMainDisplayID() -> u32;
        pub fn CGDisplayBounds(display: u32) -> CGRect;
    }
}

// macOS：CGEventCreate(nil) + CGEventGetLocation（逻辑坐标 points，×scale 换算物理像素）
// macOS: CGEventCreate(nil) + CGEventGetLocation (points; ×scale converts to physical pixels)
#[cfg(target_os = "macos")]
fn cursor_position(scale: f64) -> Option<(i32, i32)> {
    // SAFETY: 只读系统调用；NULL 返回时放弃本轮检测
    // SAFETY: read-only system calls; bail out on NULL
    unsafe {
        let event = macos_api::CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let p = macos_api::CGEventGetLocation(event);
        Some(((p.x * scale).round() as i32, (p.y * scale).round() as i32))
    }
}

// macOS：CGMainDisplayID + CGDisplayBounds（×scale 换算物理像素）
// TODO: 换用 NSScreen.visibleFrame 以扣除菜单栏（当前用全屏 bounds，垂直中心偏差约半个菜单栏高度）
// macOS: CGMainDisplayID + CGDisplayBounds (×scale to physical pixels)
// TODO: switch to NSScreen.visibleFrame to exclude the menu bar (full bounds used for now)
#[cfg(target_os = "macos")]
fn primary_workarea(scale: f64) -> Option<(i32, i32, i32, i32)> {
    // SAFETY: 只读系统调用
    // SAFETY: read-only system calls
    unsafe {
        let bounds = macos_api::CGDisplayBounds(macos_api::CGMainDisplayID());
        let x = (bounds.origin.x * scale).round() as i32;
        let y = (bounds.origin.y * scale).round() as i32;
        let w = (bounds.size.width * scale).round() as i32;
        let h = (bounds.size.height * scale).round() as i32;
        Some((x, y, w, h))
    }
}

// TODO: Linux X11（XQueryPointer + RandR）光标支持
// TODO: Linux X11 (XQueryPointer + RandR) cursor support
// Wayland 无全局光标 API：降级为仅快捷键唤醒（docs/ROADMAP.md 风险表）
// Wayland has no global cursor API: fall back to shortcut-only wake (docs/ROADMAP.md risks)
#[cfg(not(any(windows, target_os = "macos")))]
fn cursor_position(_scale: f64) -> Option<(i32, i32)> {
    None
}

#[cfg(not(any(windows, target_os = "macos")))]
fn primary_workarea(_scale: f64) -> Option<(i32, i32, i32, i32)> {
    None
}
