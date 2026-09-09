//! 启动器窗口的领域逻辑：显示 / 隐藏 / 定位 / 失焦策略，被命令层、托盘、全局快捷键共同调用。
//!
//! 这里不处理 IPC 参数，也不负责创建窗口；窗口的静态形态（透明、无边框、置顶等）在
//! `tauri.conf.json5` 定义。所有 Tauri 窗口 API 的失败都在本模块吞掉并记日志——
//! 调用方（托盘 / 快捷键回调）没有能力处理这些错误。

use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};

#[cfg(windows)]
mod windows;

/// 主窗口 label，与 `tauri.conf.json5` 的 `app.windows[0].label` 一致
pub const MAIN_WINDOW: &str = "main";
/// 托盘图标 id；放在这里而不是 `tray.rs`，是因为失焦回调要凭它查托盘矩形，避免 tray ↔ launcher 双向依赖
pub const TRAY_ID: &str = "z-tools-tray";
/// 默认的全局唤出快捷键（tauri-plugin-global-shortcut 语法）。
/// 这是**唯一**允许出现该字面量的地方：后续做成可配置时由设置层读取用户值并覆盖，
/// 注册、日志、前端提示都必须经由本常量或读取当前生效值，不得各自写死。
pub const DEFAULT_TOGGLE_SHORTCUT: &str = "alt+enter";
/// 面板已显示并聚焦；前端 `src/lib/events.ts` 的 `EVENTS.LAUNCHER_OPENED` 与此一一对应，无 payload
pub const LAUNCHER_OPENED: &str = "launcher://open";
/// 面板已隐藏；前端 `src/lib/events.ts` 的 `EVENTS.LAUNCHER_CLOSED` 与此一一对应，无 payload
pub const LAUNCHER_CLOSED: &str = "launcher://close";

/// 显示器工作区（排除任务栏后的可用区域），物理像素坐标
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkArea {
    /// 工作区左上角 x（多显示器时可能非 0 甚至为负）
    pub x: i32,
    /// 工作区左上角 y
    pub y: i32,
    /// 工作区宽度
    pub width: u32,
    /// 工作区高度
    pub height: u32,
}

/// 计算启动器窗口左上角的物理坐标：水平居中、顶边固定在工作区高度 1/4 处
/// （Flow Launcher / PowerToys Run / Spotlight 同款摆法）。
///
/// 顶边只取决于工作区，不依赖窗口内容高度：内容增高时窗口从这条顶边向下生长，
/// 搜索框在任何状态切换中都不移动。工作区比窗口还窄时贴住左缘，不让面板顶出屏幕。
pub fn anchor_position(work: WorkArea, window: (u32, u32)) -> (i32, i32) {
    let (win_w, _win_h) = window;
    let x = f64::from(work.x) + (f64::from(work.width) - f64::from(win_w)) / 2.0;
    let x = x.max(f64::from(work.x));
    let y = f64::from(work.y) + f64::from(work.height) / 4.0;
    (x.round() as i32, y.round() as i32)
}

/// 唤出启动器前的前台窗口句柄（`isize`，0 = 未记录）。`#[cfg(windows)]` 且 `app.manage` 托管；
/// `show()` 在抢焦点之前写入，剪贴板粘贴据此把焦点还给原窗口再模拟 Ctrl+V。
#[cfg(windows)]
#[derive(Debug, Default)]
pub struct PreviousForeground(std::sync::atomic::AtomicIsize);

/// 最近一次唤出前记录的前台窗口；从未记录 / 状态未托管时为 `None`。
#[cfg(windows)]
pub fn previous_foreground<R: Runtime>(app: &AppHandle<R>) -> Option<isize> {
    let hwnd = app
        .try_state::<PreviousForeground>()?
        .0
        .load(std::sync::atomic::Ordering::SeqCst);
    (hwnd != 0).then_some(hwnd)
}

/// 把某个窗口切到前台；窗口已关闭或系统拒绝时返回 `false`。见 `launcher/windows.rs::activate`。
#[cfg(windows)]
pub fn activate_window(hwnd: isize) -> bool {
    windows::activate(hwnd)
}

/// 显示启动器：记录前台窗口 → 恢复鼠标命中 → 按当前显示器定位 → 显示 → 抢焦点 → 广播 `launcher://open`。
pub fn show<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    // 必须在 show / set_focus 之前记录，之后前台就是我们自己了
    #[cfg(windows)]
    remember_foreground(app, &window);
    // 隐藏期间设置了忽略鼠标事件，显示前必须恢复，否则面板点不动
    if let Err(e) = window.set_ignore_cursor_events(false) {
        log::warn!("恢复窗口鼠标事件失败: {e}");
    }
    position_anchored(&window);
    if let Err(e) = window.show() {
        log::warn!("显示启动器窗口失败: {e}");
    }
    if let Err(e) = window.set_focus() {
        log::warn!("聚焦启动器窗口失败: {e}");
    }
    if let Err(e) = app.emit(LAUNCHER_OPENED, ()) {
        log::warn!("发送 {LAUNCHER_OPENED} 事件失败: {e}");
    }
}

/// 隐藏启动器并广播 `launcher://close`，前端据此复位状态。
///
/// 幂等：窗口已不可见时直接返回、不重复广播。剪贴板粘贴先 `SetForegroundWindow` 到原窗口（触发失焦回调 hide）
/// 再显式调 hide，没有这一步前端会收到两次 `launcher://close`。查不到可见性时按可见处理，宁可多隐藏一次。
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    if !window.is_visible().unwrap_or(true) {
        return;
    }
    hide_window(&window);
    if let Err(e) = app.emit(LAUNCHER_CLOSED, ()) {
        log::warn!("发送 {LAUNCHER_CLOSED} 事件失败: {e}");
    }
}

/// 全局快捷键的开合切换：只有「可见且持有焦点」才收起，否则一律显示并抢焦点。
///
/// 可见但失焦的情形（例如失焦隐藏尚未生效、或被其他置顶窗口盖住）按用户意图应是「把面板叫回来」。
pub fn toggle<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    if visible && focused {
        hide(app);
    } else {
        show(app);
    }
}

/// 失焦自动收起。鼠标正悬于本应用托盘图标上时跳过：
/// 该失焦由托盘按下引起，窗口保持原状，开合决策交给随后到达的托盘点击事件，
/// 这样点击事件看到的是窗口的真实可见状态，不会出现「失焦收起 → 点击再反向打开」。
pub fn hide_on_blur<R: Runtime>(app: &AppHandle<R>) {
    if cursor_on_tray(app) {
        return;
    }
    let Some(window) = main_window(app) else {
        return;
    };
    // 自身 hide() 也会触发一次失焦事件，到达时窗口已隐藏，不再重复 hide / 重复广播 close
    if window.is_visible().unwrap_or(false) {
        hide(app);
    }
}

/// 托盘左键点击的开合切换：只看可见性。
///
/// 托盘引发的失焦不会收起窗口（见 [`hide_on_blur`]），因此此处的可见性是点击前的真实状态；
/// 不额外判断焦点是因为点托盘时焦点必然已经离开面板。
pub fn toggle_from_tray<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = main_window(app) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        hide(app);
    } else {
        show(app);
    }
}

/// 应用启动时的初始收纳：只隐藏窗口，不广播事件（此时前端尚未加载，没人监听）。
pub fn init_hidden<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = main_window(app) {
        hide_window(&window);
    }
}

/// 安装 Windows 平台钩子：拦截 Alt 弹出的无边框窗口系统菜单。
/// 仅 Windows 编译，调用方需同样以 `#[cfg(windows)]` 守卫。
#[cfg(windows)]
pub fn install_platform_hooks<R: Runtime>(window: &WebviewWindow<R>) {
    match window.hwnd() {
        Ok(hwnd) => windows::suppress_alt_sysmenu(hwnd.0 as isize),
        Err(e) => log::warn!("获取窗口句柄失败，Alt 系统菜单拦截未安装: {e}"),
    }
}

/// 记录当前前台窗口到 `PreviousForeground`；前台是启动器自己（可见但失焦后再次唤出）或任务栏（从托盘点开）时
/// 保留上一次的记录，否则粘贴会把 Ctrl+V 发给任务栏。
#[cfg(windows)]
fn remember_foreground<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let Some(state) = app.try_state::<PreviousForeground>() else {
        return;
    };
    let foreground = windows::current_foreground();
    if foreground == 0 || windows::is_taskbar(foreground) {
        return;
    }
    let is_self = window.hwnd().is_ok_and(|own| own.0 as isize == foreground);
    if !is_self {
        state
            .0
            .store(foreground, std::sync::atomic::Ordering::SeqCst);
    }
}

fn main_window<R: Runtime>(app: &AppHandle<R>) -> Option<WebviewWindow<R>> {
    let window = app.get_webview_window(MAIN_WINDOW);
    if window.is_none() {
        log::warn!("找不到主窗口 {MAIN_WINDOW}");
    }
    window
}

fn hide_window<R: Runtime>(window: &WebviewWindow<R>) {
    // 隐藏期间忽略鼠标事件：透明窗口隐藏后仍可能残留命中区域，会挡住桌面 / 其他窗口的点击
    if let Err(e) = window.set_ignore_cursor_events(true) {
        log::warn!("设置窗口忽略鼠标事件失败: {e}");
    }
    if let Err(e) = window.hide() {
        log::warn!("隐藏启动器窗口失败: {e}");
    }
}

/// 把窗口摆到当前显示器工作区的启动器惯例位置；任一信息取不到就保持原位。
fn position_anchored<R: Runtime>(window: &WebviewWindow<R>) {
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => monitor,
        Ok(None) => {
            log::warn!("取不到窗口所在显示器，跳过定位");
            return;
        }
        Err(e) => {
            log::warn!("查询窗口所在显示器失败: {e}");
            return;
        }
    };
    let size = match window.outer_size() {
        Ok(size) => size,
        Err(e) => {
            log::warn!("查询窗口尺寸失败: {e}");
            return;
        }
    };
    let area = monitor.work_area();
    let work = WorkArea {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    };
    let (x, y) = anchor_position(work, (size.width, size.height));
    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        log::warn!("移动启动器窗口失败: {e}");
    }
}

/// 当前鼠标是否悬于本应用的托盘图标上。任一信息拿不到时按「不在」处理，
/// 退化为普通失焦收起——宁可误收起，不可让面板留在屏幕上不收。
fn cursor_on_tray<R: Runtime>(app: &AppHandle<R>) -> bool {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return false;
    };
    let Ok(Some(rect)) = tray.rect() else {
        return false;
    };
    let Ok(cursor) = app.cursor_position() else {
        return false;
    };
    rect_contains(&rect, cursor.x, cursor.y)
}

/// 判断屏幕坐标是否落在矩形内。托盘 rect 与鼠标位置在桌面端都是物理像素，
/// 但 `Rect` 类型允许逻辑坐标，两种枚举都展开为 f64 后比较。
fn rect_contains(rect: &tauri::Rect, x: f64, y: f64) -> bool {
    let (left, top) = match rect.position {
        tauri::Position::Physical(p) => (f64::from(p.x), f64::from(p.y)),
        tauri::Position::Logical(p) => (p.x, p.y),
    };
    let (width, height) = match rect.size {
        tauri::Size::Physical(s) => (f64::from(s.width), f64::from(s.height)),
        tauri::Size::Logical(s) => (s.width, s.height),
    };
    x >= left && x < left + width && y >= top && y < top + height
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers_horizontally_and_anchors_top_at_quarter() {
        let work = WorkArea {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(anchor_position(work, (800, 600)), (560, 270));
    }

    #[test]
    fn respects_secondary_monitor_offset() {
        let work = WorkArea {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(anchor_position(work, (800, 600)), (2480, 270));
    }

    #[test]
    fn clamps_to_left_edge_when_work_area_is_narrower() {
        let work = WorkArea {
            x: 100,
            y: 50,
            width: 600,
            height: 800,
        };
        let (x, y) = anchor_position(work, (800, 600));
        assert_eq!(x, work.x);
        assert_eq!(y, 250);
    }
}
