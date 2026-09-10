//! 系统托盘装配：应用常驻后台且窗口不在任务栏显示，
//! 托盘是用户感知进程存在、开合启动器与主动退出的唯一可见入口。
//!
//! 只做菜单 / 事件到 `launcher` 领域函数的映射，不放窗口逻辑。

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Runtime};

use crate::launcher;

/// 菜单项 id：打开启动器
const MENU_OPEN: &str = "open-launcher";
/// 菜单项 id：退出应用
const MENU_QUIT: &str = "quit";

/// 创建系统托盘：左键单击开合启动器，右键只弹出菜单、不改变面板状态。
///
/// 菜单构建或托盘注册失败返回 `tauri::Error`，由 `setup_desktop` 决定是否让启动失败。
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    // 「打开启动器」语义固定为显示并聚焦：面板已可见时等价于把它叫回前台，不做切换
    let open = MenuItem::with_id(app, MENU_OPEN, "打开启动器", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut tray = TrayIconBuilder::with_id(launcher::TRAY_ID)
        .menu(&menu)
        // 左键留给开合启动器，菜单只在右键弹出
        .show_menu_on_left_click(false)
        .tooltip(app.package_info().name.clone())
        .on_menu_event(|app, event| match event.id.as_ref() {
            MENU_OPEN => launcher::show(app),
            MENU_QUIT => launcher::quit(app), // 先 destroy 窗口再退出，见 launcher::quit
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // 只处理左键松开：托盘引发的失焦不收窗口（hide_on_blur 跳过），
            // 所以此处看到的可见性是点击前的真实状态。
            // 右键不做任何窗口操作，只让系统弹出菜单——面板保持原状，用户可能只是想看菜单或退出
            let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            else {
                return;
            };
            launcher::toggle_from_tray(tray.app_handle());
        });

    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }

    tray.build(app)?;
    Ok(())
}
