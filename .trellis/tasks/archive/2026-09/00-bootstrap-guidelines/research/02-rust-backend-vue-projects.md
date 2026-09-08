I have enough evidence. Here's the report.

# Tauri 2 后端(src-tauri)开源约定调研报告

> **样本说明**:BiliTools 仓库已被作者清空(仅剩 README/法律公告,`BiliTools/README.md`),**无法调研**。改用同目录下 **EcoPaste** 作为替补第 5 样本。以下 N/5 = HuLa、PakePlus、JiwuChat、AIaW、EcoPaste。
> 样本规模差异大:HuLa(~80 文件,含 workspace)、EcoPaste(~100 文件)为「重后端」;JiwuChat(20 文件)中等;PakePlus(6 文件)、AIaW(4 文件)为「薄壳」。薄壳项目对"分层"类结论参考价值有限。

---

## 1. 模块分层

- **lib.rs 只做 Builder 装配、main.rs 只调 `run()`**:5/5。main.rs 均为 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` + `xxx_lib::run()`(`PakePlus/src/main.rs`、`JiwuChat/src/main.rs`、`AIaW/src/main.rs`;HuLa `main.rs` 额外加 dotenv + runtime_guard)。
- **命令按领域一个文件**:3/5(HuLa `src/command/*_command.rs` 16 个文件;EcoPaste `src/commands/{clipboard,settings,window,...}.rs`;JiwuChat 按平台 `desktops/commands.rs`)。PakePlus 全部塞在 `command/cmds.rs`(770 行);AIaW 每命令一文件但仅 2 个。
- **顶层模块集合**:无统一命名。HuLa 用 Java 风(`command/ repository/ pojo/ vo/ utils/ common/ error.rs configuration.rs`);EcoPaste 用领域模块(`commands/ core/{error,paths} db/ settings/ window/ tray/ menu/ clipboard/ ...`);JiwuChat/HuLa 用 `desktops/` + `mobiles/` 顶层拆平台(2/5)。
- **mod.rs 风格**:5/5 有子目录者均用 `mod.rs`(`HuLa/src/command/mod.rs`、`EcoPaste/src/core/mod.rs`、`JiwuChat/src/desktops/mod.rs`),未见 `xxx.rs + xxx/` 2018 风格。
- **薄命令层显式声明**:仅 EcoPaste,`commands/mod.rs:1` 写明"`#[tauri::command]` 入口层(薄封装):参数校验 + 调用下层,不写业务逻辑",并用 `pub use xxx::*` glob 再导出以配合 `generate_handler!`。

## 2. 命令写法

- **`async fn`**:5/5 绝大多数命令为 `pub async fn`,即使内部无 await(JiwuChat `desktops/commands.rs:7 exist_file`、PakePlus `cmds.rs`)。
- **返回类型**:
  - `Result<T, String>`:4/5 主流(HuLa command/ 下 40+ 处;PakePlus;JiwuChat;AIaW `stream.rs:40`)。
  - `Result<T, AppError>` 自定义可序列化错误:1/5(EcoPaste `core::Result<T>` 别名,`commands/settings.rs:11`)。HuLa 少数命令直接返 `Result<_, CommonError>`(`contact_command.rs:74`),依赖 `impl From<CommonError> for String`。
  - 直接返 `T`/`bool`/`()`:JiwuChat `exist_file -> bool`、PakePlus `preview_from_config` 无返回值;`tauri::Result<()>`:JiwuChat `create_window`。
- **参数接收**:`State<'_, T>` 5/5(HuLa `State<'_, AppData>`;PakePlus `State<'_, Arc<Mutex<ServerState>>>`;EcoPaste 多个 `State<'_, DatabaseState>` 并列);`AppHandle` 5/5;`WebviewWindow` 1/5(AIaW `stream.rs:35`);`ipc::Channel` 1/5(HuLa `message_command.rs`)。
- **命令厚薄**:分歧。EcoPaste 明确薄层;HuLa 命令含大量业务(`message_command.rs` 860 行,含重试循环 `run_with_write_lock`),repository 层只封 ORM;PakePlus/JiwuChat 命令即业务。
- **`rename_all = "snake_case"`**:0/5 使用;`#[command]` 缩写导入 1/5(JiwuChat)。
- **`generate_handler!` 位置**:5/5 在 lib.rs(或 setup 函数)集中一处;HuLa 抽为 `get_invoke_handlers()` 供 desktop/mobile 复用,并在宏内 `#[cfg(desktop)]` 逐条标注(`HuLa/src/lib.rs:395+`)。

## 3. 错误处理

- **统一错误枚举 + thiserror**:2/5(HuLa `src/error.rs CommonError`;EcoPaste `src/core/error.rs AppError`),二者均 `thiserror` + `anyhow` 组合,用 `#[error(transparent)] Other(#[from] anyhow::Error)` 兜底,`#[from] sea_orm::DbErr` 做 ORM 转换。其余 3 个项目无错误类型。
- **Serialize 方式**:HuLa 转 String(`impl From<CommonError> for String`);EcoPaste 手写 `impl Serialize` 输出 `{kind, message}` 结构体(`core/error.rs:23-33`)——**仅 1 个项目**。
- **`map_err(|e| e.to_string())`**:HuLa 命令层 18 处,属泛滥;EcoPaste 仅 3 处(`clipboard/sound.rs`);JiwuChat/AIaW 用 `format!("...: {}", e)` 带上下文。
- **错误文案语言**:混杂。JiwuChat 中文(`"路径不存在"`);HuLa/AIaW/EcoPaste 英文;HuLa repository 层 `anyhow!("查询配置失败")` 中文。

## 4. 状态管理

- **`app.manage()`**:5/5 使用(AIaW 无状态除外 → 4/5)。命名:HuLa 单一大 `AppData`(`lib.rs:96`);EcoPaste 按职责多个 `XxxState/XxxStore`(`DatabaseState`、`SettingsStore`、`WindowStateStore`、`WindowLifecycleManager`);PakePlus `ServerState`。
- **锁类型**:`tokio::sync::Mutex/RwLock` 2/5(HuLa、EcoPaste `db/state.rs`);`std::sync::Mutex` 2/5(PakePlus 在 async 命令里 `lock().unwrap()`;EcoPaste 平台层)。未见 parking_lot。
- **全局 static**:3/5。HuLa `AtomicBool APP_STATE_READY`、`OnceLock`、`lazy_static!`(`desktops/directory_scanner.rs:31`);EcoPaste `std::sync::LazyLock`/`OnceLock`(`backup/mod.rs:55`);AIaW `static REQUEST_COUNTER: AtomicU32`。

## 5. 事件

- `app.emit` 为主 5/5;`window.emit` 1/5(AIaW);未见 `emit_to`。
- **事件名管理**:EcoPaste 用 `const XXX_EVENT: &str = "settings://updated"` 命名空间风格,并注释"与前端 `src/constants/events.ts` 一一对应"(`commands/settings.rs:8`)——**仅 1 个项目**;其余 4/5 用字面量(`"oauth-callback"`、`"ws-login-qr-code"`、`"tray_click"`),命名风格 kebab/snake 混用。
- **payload**:`#[derive(Clone, serde::Serialize)] struct Payload`:JiwuChat `desktops/tray.rs:10`、AIaW `stream.rs:23`;直接 emit `()`/`""`/`serde_json::json!` 也常见。
- 发送结果处理:`let _ = app.emit(...)`(HuLa/JiwuChat)或 `if let Err(e) ... log::warn!`(EcoPaste)。

## 6. 异步与运行时

- **tokio 依赖**:4/5 显式引入(HuLa `rt,rt-multi-thread,macros`;PakePlus `full`;EcoPaste 仅 `time`;JiwuChat/AIaW 无)。
- **spawn**:HuLa 混用 `tokio::spawn` 与 `tauri::async_runtime::spawn`;EcoPaste 统一 `tauri::async_runtime::spawn`;JiwuChat 用 `std::thread::spawn`(`deeplink/handlers.rs:12`)。
- **`spawn_blocking`**:仅 EcoPaste 系统性使用(`commands/clipboard.rs:143,229,610,1057`;`apps_registry.rs:81`),并在 doc 注释说明原因。
- **`block_on`**:setup 内 `tauri::async_runtime::block_on` 3/5(HuLa `lib.rs:353`;PakePlus `lib.rs:62`;EcoPaste `lib.rs:220`),属 setup 阶段惯用法。
- **阻塞反例**:JiwuChat `animate_window_resize` 在 async 命令里 `std::thread::sleep`(`desktops/commands.rs:148`);PakePlus async 命令内 `std::thread::yield_now` 轮询。

## 7. 日志

- **tauri-plugin-log**:3/5(HuLa、AIaW、EcoPaste);JiwuChat/PakePlus 纯 `println!/eprintln!`。
- 门面:HuLa 用 `tracing` 宏桥接 log(`Cargo.toml` 同时含 tracing + tauri-plugin-log);EcoPaste/AIaW 用 `log::` 宏。
- **级别/目标策略**:EcoPaste 最完整——release 仅 `LogDir`,debug 才加 `Stdout+Webview`,并注释原因(Windows release stdout 缓冲阻塞)(`lib.rs:27-50`);HuLa 三目标全开 + 对 `sqlx/sea_orm/tauri/wry` 逐个 `level_for` 降噪(`common/init.rs:10-33`);AIaW 仅 debug 时注册插件。

## 8. 序列化约定

- `#[serde(rename_all = "camelCase")]`:2/5 系统性使用(HuLa、EcoPaste 几乎所有 IPC 结构体);JiwuChat/AIaW 不加(前端收到 snake_case)。
- `skip_serializing_if`:仅 EcoPaste(`db/models.rs:72`);`#[serde(default)]` 兼容旧配置:EcoPaste `settings/model.rs:15`。
- **ts-rs / specta / tauri-specta**:0/5。TS 类型全部前端手写。

## 9. 插件与权限

- **通用官方插件(≥4/5)**:`os`、`process`、`clipboard-manager`、`opener`、`dialog`、`updater`;`fs`/`shell`/`notification`/`single-instance`/`autostart` 3/5;`log` 3/5。
- **capabilities 拆分**:按平台拆 `default.json + desktop.json (+ mobile.json)` 3/5(HuLa、JiwuChat、AIaW);EcoPaste `default.json + macos-permissions.json`(按插件/平台);PakePlus 单文件。未见按窗口拆分,`windows: ["*"]` 4/5。
- **最小化程度**:分歧。EcoPaste 仅 9 条 permission(`capabilities/default.json`),依赖 Rust 命令而非前端插件 API;HuLa/PakePlus 100+ 条且 `fs: "**"`、`http: http://**` 全开(`HuLa/capabilities/default.json`)。
- 桌面/移动依赖用 `[target.'cfg(...)'.dependencies]` 分段:4/5(HuLa、JiwuChat、AIaW、EcoPaste)。

## 10. 配置与依赖

- **`[profile.release]`**:2/5(HuLa `lto=true, codegen-units=1, opt-level=3, panic="abort", strip=true`;JiwuChat 同但 `opt-level="s"`)。其余 3 个用默认。
- **edition**:2021 4/5;HuLa 2024。
- **rustfmt.toml**:1/5(EcoPaste `max_width=100`);**rust-toolchain.toml** 1/5(EcoPaste 锁 1.96.0 + rustfmt/clippy);**clippy.toml / `[lints]`**:0/5。
- **workspace 拆 crate**:1/5(HuLa `entity/ + migration/`,sea-orm 惯例)。
- `[lib] name = "xxx_lib"` + `crate-type = ["staticlib","cdylib","rlib"]`:5/5(模板产物)。

## 11. 测试

- **内联 `#[cfg(test)] mod tests`**:2/5(EcoPaste 35 处;HuLa 1 处 `websocket/message.rs:152`)。0/5 有 `tests/` 目录。
- **覆盖层次**(EcoPaste):纯函数(`clipboard/detect.rs`)、db 仓储(内存 sqlite,`db/groups.rs:136`)、命令文件中的辅助函数(`commands/storage.rs:584`)。未见对 `#[tauri::command]` 本身或需要 `AppHandle` 的测试;EcoPaste `[dev-dependencies] tokio = {features=["macros","rt"]}` 供 `#[tokio::test]`。

## 12. 平台差异

- **顶层 `desktops/` + `mobiles/` 模块**:2/5(HuLa、JiwuChat),`lib.rs` 里 `#[cfg(desktop)] mod desktops;`,`run()` 内分叉 `setup_desktop()/setup_mobile()`。
- **OS 级子文件**:EcoPaste `autostart/{macos,windows}.rs`、`keystroke/{macos,windows}.rs`;HuLa `utils/{linux,macos,win}_runtime_guard.rs`。
- **内联 `#[cfg(target_os)]` 块**:5/5 普遍,尤其窗口构建(JiwuChat `desktops/commands.rs:165-179`)与 Builder 链上(`EcoPaste/lib.rs:73`)。
- `#[cfg_attr(mobile, tauri::mobile_entry_point)]` 标注 `run()`:4/5(模板)。

## 13. 注释与文档

- **`//!` 模块级文档**:仅 EcoPaste(几乎每个模块首行,含设计动机,`core/paths.rs:1-13`);其他 0。
- **`///` 文档注释**:EcoPaste 普遍;HuLa repository 层有(`im_config_repository.rs:6`);JiwuChat 零散;PakePlus/AIaW 几乎无。
- **语言**:中文 4/5(HuLa、JiwuChat、PakePlus、EcoPaste 均以中文为主,EcoPaste 少量英文);AIaW 无注释。

## 14. 明显反模式/踩坑

| 反模式 | 出处 |
|---|---|
| `unwrap()` 泛滥于命令/事件回调 | PakePlus `cmds.rs:130-148`(窗口构建链 5 连 unwrap);JiwuChat `tray.rs:45-56`(`window.show().unwrap()`、`emit(...).unwrap()`);AIaW `stream.rs:46` header parse unwrap |
| 巨型 lib.rs 兼当 state/模型/业务 | HuLa `lib.rs` 480 行,含 `AppData`、`UserInfo`、`handle_logout_windows` 业务 |
| async 命令中 `std::sync::Mutex::lock().unwrap()` | PakePlus `cmds.rs:35` |
| async 命令中同步 sleep/轮询 | JiwuChat `commands.rs:148`;PakePlus `cmds.rs:124-131` |
| 命令 `panic!` 处理非法入参 | HuLa `command/mod.rs:33` |
| `Result<(), ()>` 无信息错误 | HuLa `command/mod.rs:28` |
| 生产代码 `println!` 调试 + 大量注释掉的 println | PakePlus `cmds.rs:627-632`;JiwuChat 全局 |
| capabilities 全开 `fs "**"`/`http "**"` | HuLa、PakePlus `capabilities/default.json` |
| 桌面/移动命令代码重复维护两份 | JiwuChat `desktops/commands.rs` vs `mobiles/commands.rs`、两套 deeplink/ |
| 混用 `tokio::spawn` 与 `tauri::async_runtime::spawn` | HuLa |
| `.expect("error while running tauri application")` 收尾 | 4/5(模板产物,HuLa 改为 `CommonError` + exit(1)) |

---

## 汇总表

| 档位 | 结论 |
|---|---|
| **公认(≥4/5)** | main.rs 只调 `lib::run()`,lib.rs 装配 Builder;`generate_handler!` 集中一处;命令一律 `pub async fn`;命令错误 `Result<T, String>`;`State<'_, T>` + `AppHandle` 注入;`app.manage()` 管状态;`app.emit` + 字面量事件名;`mod.rs` 目录风格;`[lib] xxx_lib` 三 crate-type;平台依赖 `[target.'cfg'.dependencies]` 分段;插件基线 os/process/clipboard-manager/opener/dialog/updater;`windows:["*"]`;edition 2021;注释中文;无 specta/ts-rs;无 `tests/`、无 clippy.toml/`[lints]`;`#[cfg(target_os)]` 内联块 |
| **多数(3/5)** | 命令按领域拆文件;tauri-plugin-log;setup 内 `async_runtime::block_on` 初始化;capabilities 按 default/desktop/mobile 平台拆;全局 static(Atomic/OnceLock/LazyLock);tokio 显式依赖 |
| **分歧/不确定** | 统一错误枚举(2/5,thiserror+anyhow;结构化 `{kind,message}` 仅 EcoPaste);命令薄/厚;`rename_all = "camelCase"`(2/5);`tokio::sync::Mutex` vs `std::sync::Mutex`;`spawn_blocking`(仅 EcoPaste);事件名常量化(仅 EcoPaste);`[profile.release]` 优化(2/5);rustfmt/toolchain 锁定(仅 EcoPaste);`//!` 模块文档(仅 EcoPaste);单元测试(实质仅 EcoPaste);`desktops/`+`mobiles/` 顶层分平台(2/5);workspace 拆 crate(仅 HuLa);权限最小化(仅 EcoPaste);日志 release/debug 分目标(仅 EcoPaste) |

**给 spec 的建议倾向**:社区"公认"部分多为 Tauri 模板产物,属底线;工程质量方面 EcoPaste 是 5 个中唯一系统化的(错误结构体、事件常量、spawn_blocking、`//!` 文档、测试、rustfmt/toolchain、最小权限),但都属「仅 1 个项目」,写入 spec 时应标为"推荐"而非"社区共识"。