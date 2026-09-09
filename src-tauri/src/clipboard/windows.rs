//! 剪贴板的 Windows 平台层：消息窗口监听 `WM_CLIPBOARDUPDATE`、停止监听、`SendInput` 模拟 Ctrl+V。
//!
//! 只由父模块 `clipboard.rs` 以 `#[cfg(windows)]` 引入。读写剪贴板本身在跨平台的 `backend.rs`，
//! 这里只负责 arboard 不提供的两件事：变化通知与按键模拟。后续 macOS / Linux 各补一份同签名文件即可。

use std::sync::atomic::{AtomicIsize, Ordering};

use tauri::{AppHandle, Manager, Runtime};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, GetClipboardSequenceNumber, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput,
    VIRTUAL_KEY, VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostMessageW, RegisterClassW, UnregisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_CLIPBOARDUPDATE, WM_CLOSE, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

use super::{ClipboardStore, backend, record};
use crate::error::AppError;

/// 监听窗口的类名；进程内只注册一次
const CLASS_NAME: PCWSTR = w!("ZToolsClipboardWatcher");

/// 剪贴板监听器托管状态：保存消息窗口句柄，供退出时 [`stop_monitor`] 投递 `WM_CLOSE`。
/// 句柄以 `isize` 存放（`HWND` 是裸指针不 `Send`）；0 表示监听线程尚未创建窗口或已退出。
#[derive(Debug, Default)]
pub struct ClipboardWatcher {
    hwnd: AtomicIsize,
}

impl ClipboardWatcher {
    /// 当前监听窗口句柄；未运行时为 `None`
    pub fn hwnd(&self) -> Option<isize> {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        (hwnd != 0).then_some(hwnd)
    }
}

/// 监听线程主体（阻塞，放 `spawn_blocking`）：创建消息窗口 → `AddClipboardFormatListener` → `GetMessageW` 循环。
///
/// 收到 `WM_CLIPBOARDUPDATE` 时比对 `GetClipboardSequenceNumber` 去重，再读快照并交给异步运行时 `record()`；
/// 收到 `WM_CLOSE`（来自 [`stop_monitor`]）时摘除监听、销毁窗口、退出循环。
/// 消息窗口必须在创建它的线程上收消息，所以整个循环都在这一个线程里。
pub fn run_monitor<R: Runtime>(app: AppHandle<R>) {
    let hwnd = match create_message_window() {
        Ok(hwnd) => hwnd,
        Err(e) => {
            log::error!("创建剪贴板监听窗口失败，历史记录功能不可用: {e}");
            return;
        }
    };
    // 安全：hwnd 是本线程刚创建的存活窗口
    if let Err(e) = unsafe { AddClipboardFormatListener(hwnd) } {
        log::error!("注册剪贴板变化监听失败，历史记录功能不可用: {e}");
        destroy_window(hwnd);
        return;
    }
    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(hwnd.0 as isize, Ordering::SeqCst);
    }
    log::debug!("剪贴板监听已启动");

    // 安全：只读取序列号，无参数
    let mut last_sequence = unsafe { GetClipboardSequenceNumber() };
    let mut msg = MSG::default();
    loop {
        // 安全：msg 是本栈上的有效可写结构；hwnd None 表示接收本线程所有消息（本线程只有这一个窗口）
        let got = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        // 0 = WM_QUIT，-1 = 出错，两者都结束循环
        if got.0 <= 0 {
            break;
        }
        match msg.message {
            WM_CLIPBOARDUPDATE => {
                // 安全：只读取序列号，无参数
                let sequence = unsafe { GetClipboardSequenceNumber() };
                if sequence == last_sequence {
                    continue;
                }
                last_sequence = sequence;
                on_clipboard_update(&app);
            }
            WM_CLOSE if msg.hwnd == hwnd => break,
            _ => {
                // 仅收消息的窗口没有键盘输入，不需要 TranslateMessage
                // 安全：msg 由 GetMessageW 填充，原样派发给窗口过程
                unsafe { DispatchMessageW(&msg) };
            }
        }
    }

    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(0, Ordering::SeqCst);
    }
    // 安全：hwnd 仍是本线程持有的存活窗口，与 Add 配对
    if let Err(e) = unsafe { RemoveClipboardFormatListener(hwnd) } {
        log::warn!("摘除剪贴板变化监听失败: {e}");
    }
    destroy_window(hwnd);
    log::debug!("剪贴板监听已停止");
}

/// 请求监听线程退出：向消息窗口投递 `WM_CLOSE`，由监听线程自己做清理（跨线程不能直接 `DestroyWindow`）。
pub fn stop_monitor(hwnd: isize) {
    let hwnd = HWND(hwnd as *mut core::ffi::c_void);
    if hwnd.is_invalid() {
        return;
    }
    // 安全：PostMessageW 只把消息放进目标线程队列，窗口若已销毁则返回错误而不会崩溃
    if let Err(e) = unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
        log::warn!("通知剪贴板监听线程退出失败: {e}");
    }
}

/// 模拟一次 Ctrl+V（Ctrl↓ V↓ V↑ Ctrl↑），用虚拟键码而非扫描码，不受键盘布局影响。
///
/// 目标窗口以管理员权限运行时会被 UIPI 拒绝（已知限制）；注入数量不足即返回错误，调用方只 warn。
pub fn send_paste() -> Result<(), AppError> {
    let inputs = [
        key_input(VK_CONTROL, false),
        key_input(VK_V, false),
        key_input(VK_V, true),
        key_input(VK_CONTROL, true),
    ];
    // 安全：inputs 是栈上有效数组，cbsize 与 INPUT 实际大小一致
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    if sent as usize == inputs.len() {
        Ok(())
    } else {
        let reason = windows::core::Error::from_win32();
        Err(AppError::Clipboard(format!(
            "SendInput 只注入了 {sent}/{} 个按键事件: {reason}",
            inputs.len()
        )))
    }
}

/// 剪贴板变化：在监听线程同步读快照（arboard 非 Send），录入交给异步运行时，避免在本线程 block_on
fn on_clipboard_update<R: Runtime>(app: &AppHandle<R>) {
    let Some(captured) = backend::read_snapshot() else {
        return;
    };
    let Some(store) = app.try_state::<ClipboardStore>() else {
        log::warn!("ClipboardStore 未托管，丢弃本次剪贴板内容");
        return;
    };
    let store = store.inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        record(&app, &store, captured).await;
    });
}

/// 注册窗口类并创建 `HWND_MESSAGE` 子窗口（不可见、只收消息）
fn create_message_window() -> Result<HWND, AppError> {
    // 安全：None 取当前可执行模块句柄，无需释放
    let module =
        unsafe { GetModuleHandleW(None) }.map_err(|e| AppError::Clipboard(e.to_string()))?;
    let instance = HINSTANCE::from(module);
    let class = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    // 安全：class 是完整初始化的栈上结构，指针仅在调用期间使用
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        let reason = windows::core::Error::from_win32();
        return Err(AppError::Clipboard(format!("注册监听窗口类失败: {reason}")));
    }
    // 安全：类名已注册；HWND_MESSAGE 父句柄创建仅收消息的窗口；其余参数为空 / 零值
    let created = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            w!(""),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    };
    created.map_err(|e| {
        unregister_class(instance);
        AppError::Clipboard(format!("创建监听窗口失败: {e}"))
    })
}

fn destroy_window(hwnd: HWND) {
    // 安全：hwnd 由本线程创建且尚未销毁，DestroyWindow 必须在创建线程上调用
    if let Err(e) = unsafe { DestroyWindow(hwnd) } {
        log::warn!("销毁剪贴板监听窗口失败: {e}");
    }
    // 安全：None 取当前可执行模块句柄
    if let Ok(module) = unsafe { GetModuleHandleW(None) } {
        unregister_class(HINSTANCE::from(module));
    }
}

fn unregister_class(instance: HINSTANCE) {
    // 安全：类名与注册时相同；窗口已销毁，类不再被引用；失败无害（进程退出时系统回收）
    if let Err(e) = unsafe { UnregisterClassW(CLASS_NAME, Some(instance)) } {
        log::debug!("注销剪贴板监听窗口类失败: {e}");
    }
}

fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// 窗口过程：本窗口不处理任何消息（`WM_CLIPBOARDUPDATE` / `WM_CLOSE` 都在消息循环里截获），全部交给默认过程
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // 安全：只会由系统作为已注册类的窗口过程调用，参数原样透传
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
