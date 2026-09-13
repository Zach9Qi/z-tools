//! 剪贴板的 Linux / X11 平台层：XFixes 监听 CLIPBOARD 变化、停止监听、XTest 模拟 Ctrl+V，以及纯 Wayland 降级。
//!
//! 只由父模块 `clipboard.rs` 以 `#[cfg(target_os = "linux")]` 引入。读写剪贴板本身在跨平台的 `backend.rs`，
//! 这里只负责 arboard 不提供的两件事：变化通知与按键模拟。
//! X11 协议经由 `x11rb` 调用，本文件无直接 `unsafe`（FFI 不安全块由 crate 内部封装）。

use std::sync::Mutex;
use std::sync::atomic::{AtomicIsize, Ordering};

use arboard::Clipboard;
use tauri::{AppHandle, Manager, Runtime};
use x11rb::connection::Connection;
use x11rb::cookie::VoidCookie;
use x11rb::errors::{ConnectionError, ReplyError};
use x11rb::protocol::xproto::{
    ClientMessageEvent, ConnectionExt as _, CreateWindowAux, EventMask, KEY_PRESS_EVENT,
    KEY_RELEASE_EVENT, Window, WindowClass,
};
use x11rb::protocol::{Event, xfixes, xtest};
use x11rb::rust_connection::RustConnection;

use super::{Captured, ClipboardStore, backend, record};
use crate::error::AppError;

/// X11 keysym：左 Ctrl
const XK_CONTROL_L: u32 = 0xffe3;
/// X11 keysym：小写 v
const XK_V: u32 = 0x0076;

/// Linux (X11) 剪贴板保活句柄：
///
/// arboard 在 X11 下通过后台服务线程与一个 1x1 隐藏窗口响应其他应用的 `SelectionRequest`。
/// 当进程内所有 `Clipboard` 实例被 drop（引用计数归为 3）时，arboard 的 `Drop` 会主动销毁该窗口
/// 并 join 结束服务线程，导致剪贴板内容立即丢失。
/// z-tools 作为常驻桌面应用，通过此常驻句柄保持 `Arc<Inner>` 引用计数 > 3，
/// 阻止 arboard 提前销毁 X11 服务窗口，确保写回剪贴板后系统内其他应用能随时粘贴数据。
static CLIPBOARD_KEEPALIVE: Mutex<Option<Clipboard>> = Mutex::new(None);

/// 初始化并确保 Linux 剪贴板常驻保活实例存活
pub fn ensure_keepalive() {
    let mut guard = match CLIPBOARD_KEEPALIVE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if guard.is_none() {
        match Clipboard::new() {
            Ok(cb) => {
                log::debug!("Linux 剪贴板常驻保活实例已初始化");
                *guard = Some(cb);
            }
            Err(e) => log::warn!("初始化 Linux 剪贴板常驻保活实例失败: {e}"),
        }
    }
}

/// 释放 Linux 剪贴板常驻保活实例。
///
/// 供应用退出时（`RunEvent::Exit`）调用，触发 `arboard::Clipboard` 的 `Drop`，
/// 优雅销毁 X11 隐藏服务窗口并 join 终止服务线程。
pub fn release_keepalive() {
    let mut guard = match CLIPBOARD_KEEPALIVE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    if let Some(cb) = guard.take() {
        drop(cb);
        log::debug!("Linux 剪贴板常驻保活实例已释放");
    }
}

/// 剪贴板监听器托管状态：保存辅助窗口 XID，供退出时 [`stop_monitor`] 投递停止 ClientMessage。
/// XID 以 `isize` 存放（与 Windows `HWND` 存法对齐，托管状态必须 `Send + Sync`）；0 表示尚未创建或已退出。
#[derive(Debug, Default)]
pub struct ClipboardWatcher {
    hwnd: AtomicIsize,
}

impl ClipboardWatcher {
    /// 当前辅助窗口 XID；未运行时为 `None`
    pub fn hwnd(&self) -> Option<isize> {
        let hwnd = self.hwnd.load(Ordering::SeqCst);
        (hwnd != 0).then_some(hwnd)
    }
}

/// 监听线程主体（阻塞，放 `spawn_blocking`）：建辅助窗口 → XFixes 订阅 CLIPBOARD → `wait_for_event` 循环。
///
/// 收到 Selection 属主变更且拥有者不是本地辅助窗口时读快照并交给异步运行时 `record()`；
/// 收到停止用 ClientMessage（来自 [`stop_monitor`]）时销毁窗口、退出循环。
/// 无 X11 显示（纯 Wayland）时记日志后直接返回，历史不再自动录入，其余命令照常。
pub fn run_monitor<R: Runtime>(app: AppHandle<R>) {
    ensure_keepalive();
    if is_pure_wayland() {
        log::warn!(
            "检测到纯 Wayland 会话且无 DISPLAY，剪贴板自动监听不可用（历史需手动刷新路径外功能仍可用）"
        );
        return;
    }

    let Ok((conn, helper, stop_atom)) = setup_monitor() else {
        return;
    };

    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(helper as isize, Ordering::SeqCst);
    }
    log::debug!("剪贴板 XFixes 监听已启动（helper={helper:#x}）");

    loop {
        let event = match conn.wait_for_event() {
            Ok(event) => event,
            Err(e) => {
                log::warn!("剪贴板监听事件循环出错，退出: {e}");
                break;
            }
        };
        match event {
            Event::XfixesSelectionNotify(ev) => {
                // 本进程写回剪贴板时也可能触发；拥有者是辅助窗口则忽略
                if ev.owner != helper {
                    on_clipboard_update(&app);
                }
            }
            Event::ClientMessage(ev) if ev.window == helper && ev.type_ == stop_atom => break,
            _ => {}
        }
    }

    if let Some(watcher) = app.try_state::<ClipboardWatcher>() {
        watcher.hwnd.store(0, Ordering::SeqCst);
    }
    if let Err(e) = conn.destroy_window(helper) {
        log::warn!("销毁剪贴板辅助窗口失败: {e}");
    }
    let _ = conn.flush();
    log::debug!("剪贴板监听已停止");
}

/// 请求监听线程退出：向辅助窗口发送停止用 ClientMessage，唤醒 `wait_for_event`。
pub fn stop_monitor(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    if let Err(e) = send_stop_message(hwnd as u32) {
        log::warn!("通知剪贴板监听线程退出失败: {e}");
    }
}

/// 把快照写回 Linux 系统剪贴板。
///
/// 写入策略：
/// 1. 确保 arboard 常驻保活（避免 X11 服务窗口与线程在临时实例 drop 时过早销毁）；
/// 2. 优先调用 arboard 原生写入（覆盖 X11 以及开启 `wayland-data-control` 的 Wayland 合成器如 Sway / Hyprland）；
/// 3. 若 arboard 写入失败且当前处于 Wayland 环境，尝试调用系统 `wl-copy` 工具兜底写入；
/// 4. 若最终均无法写入，返回明确的 [`AppError::Unsupported`]，提示用户安装 `wl-clipboard` 或启用 XWayland。
pub fn write_to_clipboard(captured: &Captured) -> Result<(), AppError> {
    ensure_keepalive();

    match backend::write(captured) {
        Ok(()) => Ok(()),
        Err(arboard_err) => {
            if is_wayland() {
                log::debug!("arboard 写入剪贴板失败 ({arboard_err})，尝试通过 wl-copy 兜底写入");
                if let Err(wl_err) = try_wl_copy(captured) {
                    log::warn!("wl-copy 兜底写入失败: {wl_err}");
                    let hint = if is_pure_wayland() {
                        "纯 Wayland 环境无法写入剪贴板：arboard 未能连接到剪贴板，且未找到可用 wl-copy 工具。请启用 XWayland 或安装 wl-clipboard。"
                    } else {
                        "当前 Wayland 环境无法写入剪贴板：arboard 写入失败且未找到可用 wl-copy 工具。请检查剪贴板权限或安装 wl-clipboard。"
                    };
                    return Err(AppError::Unsupported(format!(
                        "{hint}（arboard: {arboard_err}，wl-copy: {wl_err}）"
                    )));
                }
                log::debug!("通过 wl-copy 成功写入 Wayland 剪贴板");
                return Ok(());
            }

            Err(arboard_err)
        }
    }
}

/// 尝试通过外部 `wl-copy` 工具将快照写入 Wayland 剪贴板。
fn try_wl_copy(captured: &Captured) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut cmd = Command::new("wl-copy");
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    match captured {
        Captured::Text(text) => {
            let mut child = cmd.spawn().map_err(|e| format!("启动 wl-copy 失败: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(text.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let status = child
                .wait()
                .map_err(|e| format!("等待 wl-copy 失败: {e}"))?;
            if !status.success() {
                return Err(format!("wl-copy 执行失败，退出码: {:?}", status.code()));
            }
        }
        Captured::Image { png, .. } => {
            cmd.arg("--type").arg("image/png");
            let mut child = cmd.spawn().map_err(|e| format!("启动 wl-copy 失败: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(png).map_err(|e| e.to_string())?;
            }
            let status = child
                .wait()
                .map_err(|e| format!("等待 wl-copy 失败: {e}"))?;
            if !status.success() {
                return Err(format!("wl-copy 执行失败，退出码: {:?}", status.code()));
            }
        }
        Captured::Files(paths) => {
            cmd.arg("--type").arg("text/uri-list");
            let mut uri_list = String::new();
            for path in paths {
                let p = std::path::Path::new(path);
                let abs_path = if p.is_absolute() {
                    p.to_path_buf()
                } else if let Ok(canon) = p.canonicalize() {
                    canon
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(p))
                        .unwrap_or_else(|_| p.to_path_buf())
                };
                if let Ok(url) = tauri::Url::from_file_path(&abs_path) {
                    uri_list.push_str(url.as_str());
                    uri_list.push_str("\r\n");
                } else {
                    log::warn!("无法将文件路径转为合法 URI: {}", abs_path.display());
                }
            }
            let mut child = cmd.spawn().map_err(|e| format!("启动 wl-copy 失败: {e}"))?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(uri_list.as_bytes())
                    .map_err(|e| e.to_string())?;
            }
            let status = child
                .wait()
                .map_err(|e| format!("等待 wl-copy 失败: {e}"))?;
            if !status.success() {
                return Err(format!("wl-copy 执行失败，退出码: {:?}", status.code()));
            }
        }
    }
    Ok(())
}

/// 模拟一次 Ctrl+V（Ctrl↓ V↓ V↑ Ctrl↑）。
///
/// 纯 Wayland 或无法建立 X11 连接时返回 `Ok(())` 并记 debug——内容已由上层写回剪贴板，由用户手动粘贴。
pub fn send_paste() -> Result<(), AppError> {
    if is_pure_wayland() {
        log::debug!("纯 Wayland 环境跳过按键模拟，内容已在剪贴板，请手动 Ctrl+V");
        return Ok(());
    }
    match send_paste_x11() {
        Ok(()) => Ok(()),
        Err(e) => {
            // 连接失败按降级处理（可能 DISPLAY 短暂不可用），不把整次粘贴标成错误
            if e.contains("连接 X11") {
                log::debug!("无法建立 X11 连接，跳过按键模拟: {e}");
                Ok(())
            } else {
                Err(AppError::Clipboard(e))
            }
        }
    }
}

/// 是否运行在 Wayland 会话下（包含 XWayland 会话与纯 Wayland 会话）。
pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

/// 无 `DISPLAY` 且存在 Wayland 显示时视为纯 Wayland（XWayland 通常仍会设置 `DISPLAY`）。
pub fn is_pure_wayland() -> bool {
    is_wayland() && std::env::var_os("DISPLAY").is_none()
}

fn setup_monitor() -> Result<(RustConnection, Window, u32), ()> {
    let (conn, screen_num) = match RustConnection::connect(None) {
        Ok(c) => c,
        Err(e) => {
            log::error!("连接 X11 显示失败，剪贴板历史监听不可用: {e}");
            return Err(());
        }
    };
    let screen = &conn.setup().roots[screen_num];

    if let Err(e) = xfixes::query_version(&conn, 5, 0)
        .map_err(|e| e.to_string())
        .and_then(|c| c.reply().map_err(|e| e.to_string()))
    {
        log::error!("初始化 XFixes 扩展失败，剪贴板历史监听不可用: {e}");
        return Err(());
    }

    let helper = match conn.generate_id() {
        Ok(id) => id,
        Err(e) => {
            log::error!("分配辅助窗口 XID 失败: {e}");
            return Err(());
        }
    };
    // 1×1 INPUT_ONLY 不可见窗口，只用于接收 XFixes / ClientMessage；
    // depth=COPY_DEPTH_FROM_PARENT(=0) 满足 InputOnly 要求
    if let Err(e) = check_void(
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            helper,
            screen.root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            0,
            &CreateWindowAux::new().event_mask(EventMask::NO_EVENT),
        ),
        "创建剪贴板辅助窗口",
    ) {
        log::error!("{e}");
        return Err(());
    }

    let clipboard = match intern_atom(&conn, b"CLIPBOARD") {
        Ok(a) => a,
        Err(e) => {
            log::error!("获取 CLIPBOARD atom 失败: {e}");
            let _ = conn.destroy_window(helper);
            return Err(());
        }
    };
    let stop_atom = match intern_atom(&conn, b"Z_TOOLS_CLIPBOARD_STOP") {
        Ok(a) => a,
        Err(e) => {
            log::error!("获取停止 atom 失败: {e}");
            let _ = conn.destroy_window(helper);
            return Err(());
        }
    };

    let mask = xfixes::SelectionEventMask::SET_SELECTION_OWNER
        | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
        | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
    if let Err(e) = check_void(
        xfixes::select_selection_input(&conn, helper, clipboard, mask),
        "订阅 XFixes 剪贴板事件",
    ) {
        log::error!("{e}");
        let _ = conn.destroy_window(helper);
        return Err(());
    }
    if let Err(e) = conn.flush() {
        log::error!("flush XFixes 订阅失败: {e}");
        let _ = conn.destroy_window(helper);
        return Err(());
    }

    Ok((conn, helper, stop_atom))
}

fn send_stop_message(helper: Window) -> Result<(), String> {
    let (conn, _) = RustConnection::connect(None).map_err(|e| format!("连接 X11 显示失败: {e}"))?;
    let stop_atom = intern_atom(&conn, b"Z_TOOLS_CLIPBOARD_STOP")?;
    let event = ClientMessageEvent::new(32, helper, stop_atom, [0u32; 5]);
    // ClientMessage + NoEventMask：直接投递给窗口所属客户端，无需对方预先 select 事件掩码
    check_void(
        conn.send_event(false, helper, EventMask::NO_EVENT, event),
        "发送停止 ClientMessage",
    )?;
    conn.flush()
        .map_err(|e| format!("flush 停止消息失败: {e}"))?;
    Ok(())
}

fn send_paste_x11() -> Result<(), String> {
    let (conn, _) = RustConnection::connect(None).map_err(|e| format!("连接 X11 显示失败: {e}"))?;

    let ctrl = keysym_to_keycode(&conn, XK_CONTROL_L)
        .ok_or_else(|| "找不到 Control_L 的 keycode".to_string())?;
    let v = keysym_to_keycode(&conn, XK_V).ok_or_else(|| "找不到 v 的 keycode".to_string())?;

    // 顺序：Ctrl↓ → V↓ → V↑ → Ctrl↑；time=0 表示 CurrentTime；root/坐标/device 对键盘事件忽略
    fake_key(&conn, KEY_PRESS_EVENT, ctrl)?;
    fake_key(&conn, KEY_PRESS_EVENT, v)?;
    fake_key(&conn, KEY_RELEASE_EVENT, v)?;
    fake_key(&conn, KEY_RELEASE_EVENT, ctrl)?;
    conn.flush()
        .map_err(|e| format!("flush XTest 按键失败: {e}"))?;
    Ok(())
}

fn fake_key(conn: &RustConnection, event_type: u8, keycode: u8) -> Result<(), String> {
    check_void(
        xtest::fake_input(conn, event_type, keycode, 0, x11rb::NONE, 0, 0, 0),
        "XTest fake_input",
    )
}

/// 检查 void 请求是否被 X 服务器拒绝；未 check 的 VoidCookie 丢弃后错误会挤进事件队列。
fn check_void(
    cookie: Result<VoidCookie<'_, RustConnection>, ConnectionError>,
    what: &str,
) -> Result<(), String> {
    cookie
        .map_err(|e| format!("{what} 失败: {e}"))?
        .check()
        .map_err(|e: ReplyError| format!("{what} 失败: {e}"))
}

fn keysym_to_keycode(conn: &RustConnection, keysym: u32) -> Option<u8> {
    let setup = conn.setup();
    let min = setup.min_keycode;
    let max = setup.max_keycode;
    let count = max.saturating_sub(min).saturating_add(1);
    let mapping = conn.get_keyboard_mapping(min, count).ok()?.reply().ok()?;
    let per = mapping.keysyms_per_keycode as usize;
    if per == 0 {
        return None;
    }
    for (i, chunk) in mapping.keysyms.chunks(per).enumerate() {
        if chunk.contains(&keysym) {
            let code = u32::from(min) + i as u32;
            return u8::try_from(code).ok();
        }
    }
    None
}

fn intern_atom(conn: &RustConnection, name: &[u8]) -> Result<u32, String> {
    conn.intern_atom(false, name)
        .map_err(|e| format!("intern_atom 失败: {e}"))?
        .reply()
        .map_err(|e| format!("intern_atom 回复失败: {e}"))
        .map(|r| r.atom)
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
