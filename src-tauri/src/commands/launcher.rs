//! 启动器窗口命令：前端唯一能触发的窗口操作入口，只做转发，逻辑在 `crate::launcher`。

use tauri::{AppHandle, Runtime};

/// 隐藏启动器（主页 Esc）。
///
/// 不可失败：窗口 API 失败已在领域层记日志，前端无需也无法处理。
/// 显示由快捷键 / 托盘在 Rust 侧触发，故不提供 `show_launcher`。
#[tauri::command]
pub fn hide_launcher<R: Runtime>(app: AppHandle<R>) {
    crate::launcher::hide(&app);
}

/// 返回当前生效的全局唤出快捷键（plugin 语法，默认值见 `launcher::DEFAULT_TOGGLE_SHORTCUT`），供前端渲染键帽提示。
///
/// 目前恒为默认值；做成可配置后改为读取设置，前端无需改动。不可失败。
#[tauri::command]
pub fn get_toggle_shortcut() -> &'static str {
    crate::launcher::DEFAULT_TOGGLE_SHORTCUT
}
