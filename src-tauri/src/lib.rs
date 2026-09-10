//! 应用库入口：只做 Builder 装配（plugin 注册、setup、命令注册），不放业务逻辑。
//!
//! `main.rs` 仅调用本模块的 `run()`（并带 Windows 隐藏控制台的 cfg_attr）；
//! 拆成 lib + bin 是为了让命令与错误类型能被单测与集成测试复用。

// clipboard 公开导出（同 error）：跨平台的读写 / 存储层是 crate 契约，监听与粘贴按平台接入；
// 私有的话非 Windows 下只被 windows.rs 引用的项会被 dead_code 在 -D warnings 下拦下（理由见 clipboard.rs 模块文档）
pub mod clipboard;
mod commands;
// error 模块公开导出：AppError 是本 crate 的错误契约，供命令与集成测试使用
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
                // sqlx 在 Debug 级会把每条 SQL（含整段迁移脚本）打一行，淉没业务日志；只压低这一个 target，不整体降级
                .level_for("sqlx", log::LevelFilter::Warn)
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
            commands::clipboard::list_clipboard_items,
            commands::clipboard::get_clipboard_text,
            commands::clipboard::paste_clipboard_item,
            commands::clipboard::delete_clipboard_item,
            commands::clipboard::set_clipboard_item_favorite,
        ])
        // 用 build + run 而非链式 run：需要在 Exit 事件里停掉剪贴板监听线程，覆盖托盘退出等所有退出路径
        .build(tauri::generate_context!())
        .expect("启动应用失败")
        // 两个参数带下划线：目前只有 Windows 在退出时有事可做，非 Windows 下它们未被使用
        .run(|_app, _event| {
            #[cfg(windows)]
            if let tauri::RunEvent::Exit = _event {
                use tauri::Manager;
                if let Some(hwnd) = _app
                    .try_state::<clipboard::ClipboardWatcher>()
                    .and_then(|watcher| watcher.hwnd())
                {
                    clipboard::stop_monitor(hwnd);
                }
            }
        });
}

/// 剪贴板历史装配：建目录、开库迁移、托管 `ClipboardStore`；Windows 上再启动监听线程。
///
/// 落盘统一在 `app_local_data_dir()/clipboard/`（Windows 为 `%LOCALAPPDATA%\<identifier>\clipboard\`，
/// 与 WebView2 / 日志同目录，删一个目录即可完整清理）；**不用** `app_data_dir()`（Roaming）。
/// `block_on` 只允许在 setup 阶段做一次性初始化。
#[cfg(desktop)]
fn setup_clipboard(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;

    let dir = app.path().app_local_data_dir()?.join("clipboard");
    let images_dir = dir.join("images");
    std::fs::create_dir_all(&images_dir)?;
    let db_path = dir.join("history.db");
    // 库打不开（损坏 / 迁移 checksum 不符 / 目录无权限）直接让 setup 失败、应用报错退出：
    // 不做改名重建之类的自动恢复——静默丢历史比启动失败更糟，用户看到错误后自行处理文件
    let store =
        tauri::async_runtime::block_on(clipboard::ClipboardStore::open(&db_path, images_dir))?;
    app.manage(store);

    #[cfg(windows)]
    {
        // 监听线程必须在自己的线程上跑阻塞的 GetMessageW 循环；句柄写进 ClipboardWatcher 供退出时停止
        app.manage(clipboard::ClipboardWatcher::default());
        let handle = app.handle().clone();
        tauri::async_runtime::spawn_blocking(move || clipboard::run_monitor(handle));
    }
    Ok(())
}

/// 桌面端专属装配：全局快捷键、剪贴板历史、窗口事件、平台钩子、托盘，最后把窗口收进后台。
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

    // 3. 剪贴板历史：只依赖 app_local_data_dir，不依赖窗口，放在托盘之前；库打不开是致命错误（前端列表命令会因无托管状态 panic）
    setup_clipboard(app)?;

    // 4. 平台钩子与窗口事件：失焦收起；无边框窗口没有关闭按钮，但 Alt+F4 仍会触发
    //    CloseRequested，拦成「隐藏」而不是退出。真正退出走托盘 → launcher::quit：
    //    destroy 主窗口（不能 close，会被上面拦住），Destroyed 后再 app.exit(0)，
    //    否则 Windows 上 WebView2 注销 Chrome_WidgetWin_0 会报 Error 1412。
    //    Windows 上还要托管 PreviousForeground：show() 在抢焦点前记录前台窗口，剪贴板粘贴时还回去
    #[cfg(windows)]
    app.manage(launcher::PreviousForeground::default());
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
            tauri::WindowEvent::Destroyed => handle.exit(0),
            _ => {}
        });
    }

    // 5. 托盘要在 init_hidden 之前建好：失焦回调靠 tray_by_id 判断鼠标是否在托盘上
    tray::setup(app.handle())?;

    // 6. 最后隐藏：此时前端尚未加载，init_hidden 不 emit 事件
    launcher::init_hidden(app.handle());
    Ok(())
}
