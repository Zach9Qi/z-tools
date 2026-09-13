//! 启动器窗口的 Linux / X11 钩子：EWMH 前台窗口查询与激活；Alt 系统菜单钩子为空实现。
//!
//! 只由父模块 `launcher.rs` 以 `#[cfg(target_os = "linux")]` 引入；这里不放任何跨平台逻辑。
//! X11 协议经由 `x11rb` 调用，本文件无直接 `unsafe`（FFI 不安全块由 crate 内部封装）。

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

/// 当前前台窗口 XID（存为 `isize`，0 表示没有前台窗口或查询失败）。
pub fn current_foreground() -> isize {
    match active_window() {
        Ok(xid) => xid as isize,
        Err(e) => {
            log::debug!("查询 _NET_ACTIVE_WINDOW 失败: {e}");
            0
        }
    }
}

/// 判断给定屏幕物理坐标是否落在「通知区域折叠角标」上。
///
/// Linux 桌面没有 Windows 任务栏 `^` 角标借位问题，恒返回 `false`，保持与 Windows 同名钩子契约一致。
pub fn is_cursor_on_notification_chevron(_x: f64, _y: f64) -> bool {
    false
}

/// `hwnd`（XID）是否属于本进程：读 `_NET_WM_PID` 与 `std::process::id()` 比较。
/// 属性缺失或查询失败时视为否，避免误跳过真正的外部前台窗口。
pub fn is_own_process(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    match window_pid(hwnd as u32) {
        Ok(Some(pid)) => pid == std::process::id(),
        Ok(None) | Err(_) => false,
    }
}

/// 把 `hwnd`（XID）切到前台：向根窗口发送 EWMH `_NET_ACTIVE_WINDOW` ClientMessage。
///
/// 窗口已销毁、无 X11 显示或窗口管理器拒绝时返回 `false`。
/// 必须在本进程仍持有前台（即启动器窗口还没 hide）时调用，否则可能被忽略。
pub fn activate(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    match activate_inner(hwnd as u32) {
        Ok(()) => true,
        Err(e) => {
            log::debug!("激活窗口 {hwnd:#x} 失败: {e}");
            false
        }
    }
}

/// Linux GTK 窗口无 Win32 `SC_KEYMENU` 系统菜单机制，提供 no-op 保持与 Windows 同名钩子契约一致。
pub fn suppress_alt_sysmenu(_hwnd: isize) {
    log::debug!("Linux 无需拦截 Alt 系统菜单，跳过钩子安装");
}

fn connect() -> Result<(RustConnection, usize), String> {
    RustConnection::connect(None).map_err(|e| format!("连接 X11 显示失败: {e}"))
}

fn active_window() -> Result<u32, String> {
    let (conn, screen_num) = connect()?;
    let root = conn.setup().roots[screen_num].root;
    let atom = intern(&conn, b"_NET_ACTIVE_WINDOW")?;
    read_active_window(&conn, root, atom)
}

fn read_active_window(conn: &RustConnection, root: Window, atom: u32) -> Result<u32, String> {
    let prop = conn
        .get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)
        .map_err(|e| format!("读取 _NET_ACTIVE_WINDOW 失败: {e}"))?
        .reply()
        .map_err(|e| format!("读取 _NET_ACTIVE_WINDOW 回复失败: {e}"))?;
    // value32 按连接字节序解码 CARD32/WINDOW；0 表示当前无活跃窗口
    prop.value32()
        .and_then(|mut it| it.next())
        .filter(|&xid| xid != 0)
        .ok_or_else(|| "根窗口无有效的 _NET_ACTIVE_WINDOW".into())
}

fn window_pid(window: Window) -> Result<Option<u32>, String> {
    let (conn, _) = connect()?;
    let atom = intern(&conn, b"_NET_WM_PID")?;
    let prop = conn
        .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1)
        .map_err(|e| format!("读取 _NET_WM_PID 失败: {e}"))?
        .reply()
        .map_err(|e| format!("读取 _NET_WM_PID 回复失败: {e}"))?;
    Ok(prop.value32().and_then(|mut it| it.next()))
}

fn activate_inner(window: Window) -> Result<(), String> {
    let (conn, screen_num) = connect()?;
    let root = conn.setup().roots[screen_num].root;
    let atom = intern(&conn, b"_NET_ACTIVE_WINDOW")?;

    // 1. 目标窗口有效性检查（对应 Windows 的 IsWindow 校验；若窗口已销毁，此处直接返回 BadWindow 错误）
    conn.get_window_attributes(window)
        .map_err(|e| format!("查询窗口属性连接失败: {e}"))?
        .reply()
        .map_err(|e| format!("目标窗口 {window:#x} 无效或已销毁: {e}"))?;

    // 2. 若目标窗口当前已是活跃窗口，直接返回成功
    if read_active_window(&conn, root, atom).ok() == Some(window) {
        return Ok(());
    }

    // 3. 构造并发送 EWMH _NET_ACTIVE_WINDOW ClientMessage
    // EWMH 约定：
    // data[0]=1 表示请求来自应用程序；
    // data[1]=0 表示当前时间戳（CurrentTime）；
    // data[2]=发送方当前活跃窗口（供 WM 判定请求合法性及防焦点窃取策略）。
    let current_active = read_active_window(&conn, root, atom).unwrap_or(0);
    let event = ClientMessageEvent::new(32, window, atom, [1u32, 0, current_active, 0, 0]);

    // check：确认 X 服务器接受并排队了该事件；丢弃未 check 的 VoidCookie 会把 X 错误变成事件队列噪声
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
        event,
    )
    .map_err(|e| format!("发送 _NET_ACTIVE_WINDOW 失败: {e}"))?
    .check()
    .map_err(|e| format!("发送 _NET_ACTIVE_WINDOW 被拒绝: {e}"))?;
    conn.flush()
        .map_err(|e| format!("flush 激活请求失败: {e}"))?;

    // 4. 轮询确认窗口管理器是否真正完成了焦点切换（最长等待 60ms，步进 5ms）
    // 避免窗口管理器因防焦点窃取（Focus Stealing Prevention）静默拒绝时误判激活成功导致后续误发击键
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(60);
    while start.elapsed() < timeout {
        std::thread::sleep(std::time::Duration::from_millis(5));
        if read_active_window(&conn, root, atom).ok() == Some(window) {
            return Ok(());
        }
    }

    Err(format!(
        "窗口管理器未在超时时间内切换焦点到目标窗口 {window:#x}（可能被防焦点窃取策略静默忽略）"
    ))
}

fn intern(conn: &RustConnection, name: &[u8]) -> Result<u32, String> {
    conn.intern_atom(false, name)
        .map_err(|e| format!("intern_atom 失败: {e}"))?
        .reply()
        .map_err(|e| format!("intern_atom 回复失败: {e}"))
        .map(|r| r.atom)
}
