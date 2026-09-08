//! 应用库入口：只做 Builder 装配（plugin 注册、setup、命令注册），不放业务逻辑。
//!
//! `main.rs` 仅调用本模块的 `run()`（并带 Windows 隐藏控制台的 cfg_attr）；
//! 拆成 lib + bin 是为了让命令与错误类型能被单测与集成测试复用。

mod commands;
mod error;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 日志插件：debug 构建输出 Debug 级，发布构建只输出 Info 级
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(if cfg!(debug_assertions) {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Info
                })
                .build(),
        )
        .setup(|_app| {
            // 装配位置：需要在启动时初始化的全局状态（app.manage）、系统插件等放在这里；
            // 桌面端专属装配建议拆成 `#[cfg(desktop)] fn setup_desktop`
            Ok(())
        })
        // 全部命令在此注册，漏注册前端 invoke 会直接报错
        .invoke_handler(tauri::generate_handler![commands::greet::greet])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
