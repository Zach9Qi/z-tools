//! 启动器窗口的 Win32 钩子：给无边框窗口装子类过程拦截 Alt 弹出的系统菜单；记录 / 激活前台窗口供粘贴回原窗口。
//!
//! 只由父模块 `launcher.rs` 以 `#[cfg(windows)]` 引入；这里不放任何跨平台逻辑。

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, IsWindow, SC_KEYMENU, SetForegroundWindow, WM_NCDESTROY,
    WM_SYSCOMMAND,
};

/// 任务栏窗口类名：主屏 `Shell_TrayWnd`，副屏 `Shell_SecondaryTrayWnd`
const TASKBAR_CLASSES: [&str; 2] = ["Shell_TrayWnd", "Shell_SecondaryTrayWnd"];

/// 当前前台窗口句柄（`isize`，0 表示没有前台窗口，例如屏幕正在锁定）。
pub fn current_foreground() -> isize {
    // 安全：无参数、无副作用的查询
    unsafe { GetForegroundWindow() }.0 as isize
}

/// `hwnd` 是否为任务栏（从托盘点开启动器时前台窗口就是它）；取不到类名按「不是」处理。
pub fn is_taskbar(hwnd: isize) -> bool {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    if hwnd.is_invalid() {
        return false;
    }
    // 类名上限 256 字符（RegisterClass 限制），多留一位结尾
    let mut buf = [0u16; 257];
    // 安全：buf 是栈上可写缓冲，长度由切片传入；句柄无效时返回 0 不写入
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    let Ok(len) = usize::try_from(len) else {
        return false;
    };
    let class = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
    TASKBAR_CLASSES.contains(&class.as_str())
}

/// 把 `hwnd` 切到前台；窗口已销毁或系统拒绝（本进程不再是前台进程）时返回 `false`。
///
/// 必须在本进程仍持有前台（即启动器窗口还没 hide）时调用，否则 `SetForegroundWindow` 只会闪烁任务栏。
pub fn activate(hwnd: isize) -> bool {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    if hwnd.is_invalid() {
        return false;
    }
    // 安全：IsWindow 对任意句柄值都只做查询，无效句柄返回 false
    if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
        return false;
    }
    // 安全：上一步已确认 hwnd 是存活窗口
    unsafe { SetForegroundWindow(hwnd) }.as_bool()
}

/// 与本模块回调配对的 subclass id（"ZTOL"）；自定义值避免与 Tauri / WebView2 自身的 subclass 冲突
const SUBCLASS_ID: usize = 0x5A54_4F4C;

/// 在启动器顶层窗口上拦截 `WM_SYSCOMMAND / SC_KEYMENU`。
///
/// 单独按下并松开 Alt（或先 Alt 再 Enter）时，系统会把窗口切入菜单模式；
/// 无边框启动器没有菜单栏，`DefWindowProc` 便弹出系统菜单（还原 / 移动 / 关闭）。
/// 默认唤出键含 Alt（也可能被用户配成其它含 Alt 的组合），这个菜单会误伤。吞掉 `SC_KEYMENU` 即可，
/// Alt+F4 等其它系统命令不受影响。安装失败只记日志，不让启动失败。
pub fn suppress_alt_sysmenu(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    if hwnd.is_invalid() {
        log::warn!("窗口句柄无效，跳过 Alt 系统菜单拦截");
        return;
    }
    // 安全：hwnd 来自 Tauri 刚创建的存活窗口；subclass_proc 签名与 SUBCLASSPROC 完全一致
    let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0) };
    if ok.as_bool() {
        log::debug!("已拦截 Alt 触发的窗口系统菜单");
    } else {
        log::warn!("安装 Alt 系统菜单拦截失败");
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    // wparam 低 4 位由系统内部使用，比较前必须用 0xFFF0 掩掉
    if msg == WM_SYSCOMMAND && (wparam.0 as u32 & 0xFFF0) == SC_KEYMENU {
        return LRESULT(0);
    }
    if msg == WM_NCDESTROY {
        // 安全：窗口销毁的最后一条消息，此刻子类仍在链上，用安装时的同一组参数即可摘除
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
        }
    }
    // 安全：本函数只会由系统作为已安装的子类过程调用，参数原样透传给链上下一个过程
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
