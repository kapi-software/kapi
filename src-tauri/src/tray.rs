// 系统托盘：应用驻留运行 + 主面板/设置/退出菜单（菜单文案跟随应用语言）
// System tray: resident app + panel/settings/quit menu (labels follow the app language)
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

// 托盘菜单项 id
// Tray menu item ids
const ID_PANEL: &str = "panel";
const ID_SETTINGS: &str = "settings";
const ID_QUIT: &str = "quit";

// 托盘状态：当前语言（决定菜单文案）
// Tray state: current language (drives menu labels)
#[derive(Default)]
pub struct TrayState(pub Mutex<String>);

// 菜单文案（中/英，与语言包保持一致）
// Menu labels (zh/en, kept in sync with the locale packs)
fn menu_labels(lang: &str) -> (&'static str, &'static str, &'static str) {
    if lang.starts_with("en") {
        ("Show Panel", "Settings", "Quit")
    } else {
        ("主面板", "设置", "退出")
    }
}

// 显示并聚焦主面板
// Show and focus the main panel
fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

// 构建托盘菜单
// Build the tray menu
fn build_menu(app: &AppHandle, lang: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let (panel, settings, quit) = menu_labels(lang);
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ID_PANEL, panel, true, None::<&str>)?,
            &MenuItem::with_id(app, ID_SETTINGS, settings, true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_QUIT, quit, true, None::<&str>)?,
        ],
    )
}

// 初始化托盘：图标 + 菜单 + 事件（应用启动时调用一次）
// Initialize the tray: icon + menu + events (called once at startup)
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let lang = app.state::<TrayState>().0.lock().unwrap().clone();
    let menu = build_menu(app, &lang)?;

    let mut builder = TrayIconBuilder::with_id("kapi-tray")
        .tooltip("Kapi")
        .menu(&menu)
        // Windows/macOS 惯例：左键单击 = 显示主面板，菜单只在右键弹出
        // Convention: left click shows the panel; the menu opens on right click only
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            id if id == ID_PANEL => show_main(app),
            id if id == ID_SETTINGS => {
                show_main(app);
                // 主窗口（可能在后台）内的路由跳转到设置页
                // Navigate the (possibly hidden) main window to the settings page
                let _ = app.emit_to("main", "app:navigate", "/settings");
            }
            id if id == ID_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });

    // 复用应用图标作为托盘图标
    // Reuse the app icon as the tray icon
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    builder.build(app)?;
    Ok(())
}

// 前端推送语言变更：重建托盘菜单文案
// Frontend pushes language changes: rebuild the tray menu labels
#[tauri::command]
pub fn tray_set_language(app: AppHandle, state: tauri::State<'_, TrayState>, language: String) {
    {
        let mut lang = state.0.lock().unwrap();
        if *lang == language {
            return;
        }
        *lang = language;
    }
    if let Some(tray) = app.tray_by_id("kapi-tray") {
        let lang = state.0.lock().unwrap().clone();
        if let Ok(menu) = build_menu(&app, &lang) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}
