//! 启动器窗口的 Win32 钩子：给无边框窗口装子类过程，拦截 Alt 弹出的系统菜单。
//!
//! 只由父模块 `launcher.rs` 以 `#[cfg(windows)]` 引入；这里不放任何跨平台逻辑。

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{SC_KEYMENU, WM_NCDESTROY, WM_SYSCOMMAND};

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
