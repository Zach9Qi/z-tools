# Linux 平台支持技术方案设计 (Design)

## 1. 架构总览与分层设计

根据 `.trellis/spec/backend/directory-structure.md` 的平台隔离规范，将所有平台专属逻辑严格收敛在对应领域目录下的平台文件中，通过父模块的同名函数对外暴露统一契约：

```text
src-tauri/src/
├── launcher.rs                 # 启动器领域契约（通过 cfg 路由到 windows.rs / linux.rs）
├── launcher/
│   ├── windows.rs              # Win32 前台窗口、Alt 系统菜单拦截
│   └── linux.rs                # 【新增】X11 EWMH 前台窗口记录与切回、空实现钩子
├── clipboard.rs                # 剪贴板领域契约（跨平台编排 + 平台分发）
├── clipboard/
│   ├── backend.rs              # arboard 剪贴板读写、图片解码、哈希（完全跨平台，无 cfg）
│   ├── store.rs                # SQLite 存储层（完全跨平台，无 cfg）
│   ├── windows.rs              # Win32 剪贴板监听、SendInput 粘贴
│   └── linux.rs                # 【新增】x11rb XFixes 监听、XTest 模拟粘贴、Wayland 降级
└── lib.rs                      # 组合根：setup_desktop / setup_clipboard / Exit 事件平台装配
```

---

## 2. 核心技术实现细节

### 2.1 依赖与通信基础：`x11rb`
在 `src-tauri/Cargo.toml` 中增加 Linux 专属依赖：
```toml
[target.'cfg(target_os = "linux")'.dependencies]
# 纯 Rust X11 协议客户端，不依赖 libX11 / libXtst 等 C 头文件，开启 xfixes 和 xtest
x11rb = { version = "0.14.0", default-features = false, features = ["allow-unsafe-code", "xfixes", "xtest"] }
```

### 2.2 启动器前台窗口追踪 (`launcher/linux.rs`)
- **窗口识别**：
  - Windows 下窗口句柄为 `HWND`（`isize`）；
  - Linux X11 下窗口标识为 `u32`（XID）。
  - 在 `PreviousForeground` 中统一以 `AtomicIsize` 存储（Linux 侧将 `u32` 强转为 `isize`，0 统一表示无记录）。
- **查询当前前台 (`current_foreground`)**：
  - 建立 X11 连接，获取根窗口（Root Window）。
  - 查询根窗口上的 `_NET_ACTIVE_WINDOW` 属性（Atom: `_NET_ACTIVE_WINDOW`，Type: `ATOM_WINDOW`）。
  - 返回活跃窗口的 XID。
- **切回前台窗口 (`activate`)**：
  - 目标窗口存活性检查：通过 `conn.get_window_attributes(window)?.reply()` 确认目标 XID 依然有效，若窗口已关闭销毁（返回 `BadWindow` 等错误）则立即返回 `false`。
  - 构建 EWMH ClientMessage 事件：
    - window: 目标窗口 XID；
    - type: `_NET_ACTIVE_WINDOW`；
    - format: 32；
    - data.as_data32(): `[1, 0, current_active, 0, 0]`（1 表示来自应用程序的请求，附带请求方当前活跃窗口供 WM 鉴权）。
  - 向根窗口发送该 ClientMessage（EventMask: `SubstructureNotify | SubstructureRedirect`），调用 `flush()`。
  - 焦点转移结果确认：轮询读取根窗口 `_NET_ACTIVE_WINDOW`（最长 60ms，步进 5ms），确认 WM 真正完成了焦点切换；若因防焦点窃取（Focus Stealing Prevention）被 WM 静默忽略，判定激活失败，防止后续盲目注入按键。
- **平台钩子**：
  - `suppress_alt_sysmenu`：Linux GTK 窗口无 Win32 系统菜单机制，提供 no-op 空函数保持接口一致。
  - `is_cursor_on_notification_chevron`：Linux 下恒返回 `false`。

### 2.3 剪贴板变化监听 (`clipboard/linux.rs`)
- **监听原理**：
  - 后台在 `tauri::async_runtime::spawn_blocking` 中运行 `run_monitor`。
  - 创建独立的 X11 连接，调用 `xfixes::query_version` 初始化扩展。
  - 创建一个 1x1 的不可见辅助窗口（Helper Window）。
  - 获取 `CLIPBOARD` 原子（Atom），调用 `xfixes::select_selection_input` 监听 `SetSelectionOwner`、`SelectionWindowDestroy`、`SelectionClientClose` 等事件。
  - 进入事件循环 `conn.wait_for_event()`：
    - 当捕获到 `Event::XfixesSetSelectionOwnerNotify` 时，如果选择区拥有者不是本地辅助窗口，则调用 `on_clipboard_update(&app)`。
- **线程退出与资源清理 (`stop_monitor`)**：
  - 将辅助窗口的 XID 记录在 `ClipboardWatcher` 托管状态中。
  - `stop_monitor` 时向该辅助窗口发送自定义 ClientMessage（如 `WM_DELETE_WINDOW`）或通过通信管道唤醒事件循环，使监听循环安全退出并销毁连接。

### 2.4 自动粘贴模拟与 Wayland 降级 (`clipboard/linux.rs` & `clipboard.rs`)
- **环境检测**：
  - 检测环境变量 `XDG_SESSION_TYPE` 和 `WAYLAND_DISPLAY`。
- **X11 自动粘贴 (`send_paste`)**：
  - 使用 `xtest::fake_input`：
    1. 查询键盘映射（或已知 Keycode）获取 `Control_L` 与 `v` 的 Keycode；
    2. 按下 `Control_L`（`fake_input(conn, KeyPress, ctrl_code, 0, ...)`）；
    3. 按下 `v`；
    4. 释放 `v`；
    5. 释放 `Control_L`；
    6. 执行 `conn.flush()`。
- **Wayland 剪贴板写回、降级与明确报错**：
  - `Cargo.toml` 中为 Linux 开启 `arboard` 的 `wayland-data-control` feature，在支持 `wlr-data-control` 的合成器（如 Sway / Hyprland）下可原生读写。
  - 在不支持 `wlr-data-control` 的 Wayland 环境下（如 GNOME Mutter 纯 Wayland），`write_to_clipboard` 会通过系统 `wl-copy` 工具兜底写回剪贴板。文件列表写回时通过 `Url::from_file_path` 严格进行标准 URI 编码（避免 `%`、`#`、空格等特殊字符被目标应用错误解析）。
  - 若 `arboard` 与 `wl-copy` 均无法工作，返回带明确诊断指引的 `AppError::Unsupported`（提示启用 XWayland 或安装 `wl-clipboard`），绝不静默失败或导致面板残留。
- **arboard X11 服务窗口常驻保活与优雅释放 (`ensure_keepalive` / `release_keepalive`)**：
  - X11 采用 Selection Owner 拉取模型，剪贴板数据必须由所有者窗口提供响应。`arboard` 的 `Drop` 会在所有 `Clipboard` 实例销毁（`strong_count == 3`）时主动销毁 X11 服务窗口并结束线程。
  - 在 Linux 平台层建立常驻保活句柄（`CLIPBOARD_KEEPALIVE`），使引用计数维持在 3 以上，避免临时实例 drop 导致写回数据立失。应用启动与写回时均确保保活实例存在。
  - 应用退出（`RunEvent::Exit`）时调用 `release_keepalive()`，主动将保活实例置空并 drop，触发 `arboard` 优雅销毁窗口并 join 后台服务线程，完成完整的资源生命周期闭环。

---

## 3. 跨平台接口契约收敛

在 `launcher.rs` 与 `clipboard.rs` 中，抹平 Windows 与 Linux 的接口差异：

```rust
// launcher.rs:
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

// 托管状态跨平台统一：
#[cfg(any(windows, target_os = "linux"))]
#[derive(Debug, Default)]
pub struct PreviousForeground(std::sync::atomic::AtomicIsize);

// clipboard.rs:
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(windows, target_os = "linux"))]
pub use platform::{ClipboardWatcher, run_monitor, stop_monitor};
```

在 `lib.rs` 中：
```rust
#[cfg(desktop)]
fn setup_clipboard(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 目录建立与数据库开库代码完全跨平台不变...
    #[cfg(any(windows, target_os = "linux"))]
    {
        app.manage(clipboard::ClipboardWatcher::default());
        let handle = app.handle().clone();
        tauri::async_runtime::spawn_blocking(move || clipboard::run_monitor(handle));
    }
    Ok(())
}
```

---

## 4. 风险、边界与应对

1. **Wayland 会话兼容风险**：
   - 部分现代发行版（如 Ubuntu 22.04+、Fedora）默认启用 Wayland。但在默认配置下，大多数应用通常支持 XWayland。`x11rb` 在 XWayland 仍然能够正常建立 X11 连接并监听大多数应用的剪贴板操作。
   - 应对：检测到无 X11 显示环境时，平稳回退，不崩溃、不 panic。
2. **多线程 X11 连接生命周期**：
   - 监听线程拥有独立的 X11 连接，模拟粘贴拥有独立的 X11 连接，互不抢占或死锁。
3. **Windows 功能回归**：
   - 所有改动严格遵循 Rust 条件编译，在 Windows 构建下生成的代码路径与此前保持严格一致。
