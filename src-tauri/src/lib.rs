//! 应用库入口：只做 Builder 装配（plugin 注册、setup、命令注册），不放业务逻辑。
//!
//! `main.rs` 仅调用本模块的 `run()`（并带 Windows 隐藏控制台的 cfg_attr）；
//! 拆成 lib + bin 是为了让命令与错误类型能被单测与集成测试复用。

mod commands;
// error 模块公开导出：AppError 是本 crate 的错误契约，即使当前命令都不可失败也要保留给后续命令与集成测试使用
pub mod error;
mod launcher;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 日志插件：debug 构建输出 Debug 级，发布构建只输出 Info 级
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .setup(|app| {
            // 装配位置：需要在启动时初始化的全局状态（app.manage）、系统插件等放在这里；
            // 桌面端专属装配拆在 setup_desktop，setup 内出错一律返回 Err，不 panic（会跨 FFI 直接 abort）
            #[cfg(desktop)]
            setup_desktop(app)?;
            Ok(())
        })
        // 全部命令在此注册，漏注册前端 invoke 会直接报错
        .invoke_handler(tauri::generate_handler![
            commands::launcher::hide_launcher,
            commands::launcher::get_toggle_shortcut,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}

/// 桌面端专属装配：全局快捷键、窗口事件、平台钩子、托盘，最后把窗口收进后台。
#[cfg(desktop)]
fn setup_desktop(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    // 1. 插件必须先注册，否则下面 global_shortcut() 取不到实例
    app.handle()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())?;

    // 2. 注册默认唤出快捷键（见 `launcher::DEFAULT_TOGGLE_SHORTCUT`）开合面板；只处理 Pressed，否则松开时会再 toggle 一次。
    //    快捷键被其他软件占用不是致命错误：托盘仍可打开面板，所以记 error 后继续启动
    if let Err(e) = app.global_shortcut().on_shortcut(
        launcher::DEFAULT_TOGGLE_SHORTCUT,
        |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                launcher::toggle(app);
            }
        },
    ) {
        let shortcut = launcher::DEFAULT_TOGGLE_SHORTCUT;
        log::error!("注册全局唤出快捷键 {shortcut} 失败: {e}");
    }

    // 3. 平台钩子与窗口事件：失焦收起；无边框窗口没有关闭按钮，但 Alt+F4 仍会触发
    //    CloseRequested，拦成「隐藏」而不是退出，退出只走托盘菜单
    if let Some(window) = app.get_webview_window(launcher::MAIN_WINDOW) {
        #[cfg(windows)]
        launcher::install_platform_hooks(&window);
        let handle = app.handle().clone();
        window.on_window_event(move |event| match event {
            tauri::WindowEvent::Focused(false) => launcher::hide_on_blur(&handle),
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                launcher::hide(&handle);
            }
            _ => {}
        });
    }

    // 4. 托盘要在 init_hidden 之前建好：失焦回调靠 tray_by_id 判断鼠标是否在托盘上
    tray::setup(app.handle())?;

    // 5. 最后隐藏：此时前端尚未加载，init_hidden 不 emit 事件
    launcher::init_hidden(app.handle());
    Ok(())
}
