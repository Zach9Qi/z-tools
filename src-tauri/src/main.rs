// 正式构建时在 Windows 上隐藏额外控制台窗口，不要删除这行
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri_vue_starter_lib::run()
}
