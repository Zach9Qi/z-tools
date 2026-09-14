//! 剪贴板的 macOS 平台层：NSPasteboard `changeCount` 轮询监听、停止监听、CGEvent 模拟 Cmd+V 与辅助功能权限降级。
//!
//! 只由父模块 `clipboard.rs` 以 `#[cfg(target_os = "macos")]` 引入。读写剪贴板本身在跨平台的 `backend.rs`，
//! 这里只负责 arboard 不提供的两件事：变化通知与按键模拟。

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::time::Duration;

use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use objc2_app_kit::NSPasteboard;
use tauri::{AppHandle, Manager, Runtime};

use super::{ClipboardStore, backend, record};
use crate::error::AppError;

/// 轮询间隔：单次 `changeCount` 仅为内存/轻量 IPC 整数读取，250ms 在灵敏度与占用间平衡。
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// 进程级停止信号：`stop_monitor` 置位后，监听循环在下一个轮询周期退出。
/// 不放进托管状态，是因为 Exit 路径的 `stop_monitor(hwnd)` 没有 `AppHandle`。
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);

/// 剪贴板监听器托管状态：以哨兵值标记监听是否在跑，供退出时 [`stop_monitor`] 发现并请求停止。
/// 字段名 `hwnd` 与 Windows / Linux 对齐（托管状态必须 `Send + Sync`）；0 表示尚未启动或已退出，1 表示运行中。
#[derive(Debug, Default)]
pub struct ClipboardWatcher {
    hwnd: AtomicIsize,
}

impl ClipboardWatcher {
    /// 当前监听是否在跑；未运行时为 `None`
    pub fn hwnd(&self) -> Option<isize> {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        (hwnd != 0).then_some(hwnd)
    }
}

/// 监听线程主体（阻塞，放 `spawn_blocking`）：轮询 `NSPasteboard.changeCount`，变化时读快照并 `record()`。
///
/// 收到 [`stop_monitor`] 的停止请求后，在下一个 250ms 周期内退出并清零托管标记。
pub fn run_monitor<R: Runtime>(app: AppHandle<R>) {
    STOP_REQUESTED.store(false, Ordering::SeqCst);
    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(1, Ordering::SeqCst);
    }

    let pasteboard = NSPasteboard::generalPasteboard();
    let mut last_count = pasteboard.changeCount();
    log::debug!(
        "剪贴板 changeCount 轮询监听已启动（interval={}ms）",
        POLL_INTERVAL.as_millis()
    );

    while !STOP_REQUESTED.load(Ordering::SeqCst) {
        std::thread::sleep(POLL_INTERVAL);
        if STOP_REQUESTED.load(Ordering::SeqCst) {
            break;
        }
        let change_count = pasteboard.changeCount();
        if change_count == last_count {
            continue;
        }
        last_count = change_count;
        on_clipboard_update(&app);
    }

    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(0, Ordering::SeqCst);
    }
    log::debug!("剪贴板监听已停止");
}

/// 请求监听线程退出：置位停止标记，由轮询循环自行收尾。
pub fn stop_monitor(_hwnd: isize) {
    STOP_REQUESTED.store(true, Ordering::SeqCst);
}

/// 模拟一次 Cmd+V（V↓ / V↑，事件 flags 带 Command）。
///
/// 未授予辅助功能（Accessibility）权限时优雅降级：记 warn、返回 `Ok`——内容已由上层写回剪贴板，
/// 原应用也已激活，用户手动按一次 Cmd+V 即可。授权后无需重启，下次粘贴自动生效完整模拟。
pub fn send_paste() -> Result<(), AppError> {
    // 安全：AXIsProcessTrusted 无参数、只读查询当前进程是否已在辅助功能白名单中，不弹授权框
    if !unsafe { AXIsProcessTrusted() } {
        log::warn!("macOS 未授予辅助功能权限，模拟 Cmd+V 跳过，已优雅降级为仅写回剪贴板");
        return Ok(());
    }

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|()| AppError::Clipboard("创建 CGEventSource 失败".into()))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, true)
        .map_err(|()| AppError::Clipboard("创建 Cmd+V 按下事件失败".into()))?;
    let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_V, false)
        .map_err(|()| AppError::Clipboard("创建 Cmd+V 释放事件失败".into()))?;

    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);

    // 先按下再释放，顺序与真实 Cmd+V 一致；core-graphics 的 post 为安全封装，失败不 panic
    key_down.post(CGEventTapLocation::Session);
    key_up.post(CGEventTapLocation::Session);
    Ok(())
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

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    /// 查询本进程是否已获辅助功能权限（不弹系统授权框）。
    fn AXIsProcessTrusted() -> bool;
}
