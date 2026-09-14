//! 启动器窗口的 macOS / AppKit 钩子：NSWorkspace 前台应用 PID 查询与激活；Alt 系统菜单钩子为空实现。
//!
//! 只由父模块 `launcher.rs` 以 `#[cfg(target_os = "macos")]` 引入；这里不放任何跨平台逻辑。

use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};

/// 当前前台应用的进程 ID（存为 `isize`，0 表示无有效前台或查询失败）。
pub fn current_foreground() -> isize {
    let Some(app) = NSWorkspace::sharedWorkspace().frontmostApplication() else {
        return 0;
    };
    let pid = app.processIdentifier();
    // processIdentifier 在无 pid 的应用上返回 -1；0 也视为无效
    if pid <= 0 {
        return 0;
    }
    pid as isize
}

/// 判断给定屏幕物理坐标是否落在「通知区域折叠角标」上。
///
/// macOS 菜单栏图标无 Windows 任务栏 `^` 角标借位问题，恒返回 `false`，保持与 Windows 同名钩子契约一致。
pub fn is_cursor_on_notification_chevron(_x: f64, _y: f64) -> bool {
    false
}

/// `pid` 是否属于本进程：与 `std::process::id()` 比较。
pub fn is_own_process(pid: isize) -> bool {
    if pid <= 0 {
        return false;
    }
    (pid as u32) == std::process::id()
}

/// 把给定 PID 对应的应用切到前台；应用已退出或系统拒绝时返回 `false`。
///
/// 必须在本进程仍持有前台（即启动器窗口还没 hide）时调用，否则可能被忽略。
pub fn activate(pid: isize) -> bool {
    if pid <= 0 {
        return false;
    }
    let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
    else {
        return false;
    };
    if app.isTerminated() {
        return false;
    }
    // ActivateIgnoringOtherApps 在 macOS 14+ 已无实际效果，但仍发出激活请求；
    // 空 options 在部分场景下不够可靠，故保留该标志。
    #[allow(deprecated)] // 理由见上：空 options 不够可靠，需保留已弃用标志
    app.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
}

/// macOS 无 Win32 `SC_KEYMENU` 系统菜单机制，提供 no-op 保持与 Windows 同名钩子契约一致。
pub fn suppress_alt_sysmenu(_hwnd: isize) {
    log::debug!("macOS 无需拦截 Alt 系统菜单，跳过钩子安装");
}
