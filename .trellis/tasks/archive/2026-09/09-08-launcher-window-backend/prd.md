# 启动器窗口管理(Rust 侧)

## Goal

让 z-tools 的启动器像 Flow Launcher / PowerToys Run 一样工作:平时隐藏在托盘,`Alt+Enter` 随时唤出一个透明无边框、置顶、按当前显示器定位的面板;点外面 / 按 Esc 即收起。上个任务(`09-08-launcher-shell-frontend`)已完成前端壳与 `Esc → hide`、`ResizeObserver → setSize` 两处窗口调用,本任务补齐 Rust 侧窗口管理并把前端的隐藏路径改为经由后端命令。

## Background(已确认的事实)

### 参考实现(zach-tools 后端,只取行为与结构)

- `tauri.conf.json5` 主窗口:`800×600, center, visible:false, focus:false, decorations:false, transparent:true, alwaysOnTop:true, skipTaskbar:true, resizable:false, shadow:false`;宽度 800 是唯一定义处(前端 `setSize` 读 `window.innerWidth`),不设 `minHeight`。无 `app.trayIcon` 配置,托盘在 Rust 端 `TrayIconBuilder` 创建。
- `Cargo.toml`:`tauri` feature `tray-icon`;`tauri-plugin-global-shortcut 2.3.x`;`[target."cfg(windows)".dependencies] windows`(仅需 `Win32_Foundation`、`Win32_UI_Shell`、`Win32_UI_WindowsAndMessaging`)。
- `setup_desktop` 顺序:`app.handle().plugin(global_shortcut)` → `on_shortcut("alt+enter")`(只处理 `Pressed`)→ Windows 装 `SetWindowSubclass` 钩子 → `on_window_event(Focused(false))` → 托盘 → `init_hidden`。
- `launcher_window` 服务:`show`(`set_ignore_cursor_events(false)` → 定位 → `show` → `set_focus` → emit open)、`hide`(`set_ignore_cursor_events(true)` → `hide` → emit close)、`toggle`(`visible && focused` 才 hide,否则 show 抢焦点)、`hide_on_blur`(光标在托盘 rect 内则跳过)、`toggle_from_tray`(只看 visible)、`init_hidden`(不 emit)。定位:`x = work.x + (work.w - win.w)/2`(不小于 `work.x`),`y = work.y + work.h/4`,用 `PhysicalPosition`。
- 托盘:菜单「打开启动器」/ 分隔 / 「退出」(`app.exit(0)`);`show_menu_on_left_click(false)`;左键 Up → `toggle_from_tray`,右键 Up → `hide`(菜单弹出前主动收起,让「打开启动器」语义固定);图标用 `default_window_icon()`。
- Windows 钩子:`SetWindowSubclass` 拦 `WM_SYSCOMMAND` 且 `(wparam & 0xFFF0) == SC_KEYMENU` 返回 0(否则单按 Alt / Alt+Enter 会弹无边框窗口的系统菜单);`WM_NCDESTROY` 自移除;自定义 `SUBCLASS_ID`;安装失败只 `warn`。
- 前端:`hide_launcher` 命令替代直接 `hide()`;监听 `launcher-open` 事件后 `searchBar.focus()`(全选旧内容);`html/body` 不设背景,面板根承担底色与 `color-scheme`。
- 坑:隐藏期间必须 `set_ignore_cursor_events(true)`,否则透明窗口残留命中区挡住桌面点击;托盘引发的失焦不收窗,决策交给随后到达的 tray Click;快捷键回调只处理 `Pressed` 避免二次 toggle。

### 本仓库现状(z-tools)

- Tauri `2.11.5`(`Cargo.lock`),间接依赖 `windows 0.61.3`;`Cargo.toml` 只有 `tauri(config-json5)` / `serde` / `serde_json` / `thiserror` / `log` / `tauri-plugin-log`。
- `lib.rs` 只有日志插件 + 空 `setup` + `generate_handler![commands::greet::greet]`;`error.rs` 已有 `AppError::{InvalidInput, Io, Tauri}`;无 `services/`、无 `platform/`、无事件。
- `tauri.conf.json5` 主窗口 `800×600 center`,有边框不透明;`capabilities/default.json` = `core:default` + `core:window:allow-hide` + `core:window:allow-set-size`。
- 前端:`src/lib/window.ts` 直接 `getCurrentWindow().hide()` / `setSize()`;`LauncherPanel.vue` 的 `onEscape` 主页调 `hideLauncher()`;`index.css` 的 `body` 设 `background-color: var(--color-background)`(规范 styling §3 要求文档底色挂 body);尚无 `src/lib/events.ts`、无事件 composable。
- 后端规范约束:`xxx.rs + xxx/` 不用 `mod.rs`;三段式(命令层薄 / 领域层 / 平台层拆文件);命令返回 `Result<T, AppError>` 或不可失败 `T`,无 await 不 async;事件名 `pub const` `domain://action` 放 emit 点领域模块,payload 独立结构体,`emit` 失败只 `warn`;setup 内不 panic 返回 `Err`;不 `unwrap/expect/println!`;非通用依赖 / 配置项 / 权限带中文注释;平台依赖放 `[target.'cfg(...)']`;`#[cfg(desktop)] fn setup_desktop`;每个 `.rs` 有 `//!`,`pub` 项有 `///`;门禁 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。
- 前端规范约束:`invoke` 只在 `src/lib/api.ts`;`@tauri-apps/api/event` 只在事件 composable;事件常量 + payload 类型放 `src/lib/events.ts`;`listen` 需处理卸载竞态。

## Decisions

- **D1 快捷键**:`Alt+Enter`(与前端搜索栏已渲染的 `Alt` `Enter` 键帽一致)。
- **D2 启动即隐藏**:应用启动后只在托盘出现,不自动弹出面板(`visible:false` + `init_hidden`),与参考项目一致。
- **D3 关闭请求**:窗口无边框无关闭按钮,但 `Alt+F4` 仍会触发 `CloseRequested`;拦截为「隐藏」而不是退出,退出只走托盘菜单。
- **D4 前端透明底**:`index.css` 的 `body` 不再设 `background-color`(透明窗口的圆角外必须透明);`color-scheme` 仍留在 `html`(规范 §3 的 Lightning CSS polyfill 约束不变);面板根已是 `bg-card`。规范 styling §3/§5 相应改写(Phase 3.3)。
- **D5 隐藏路径**:前端主页 Esc 改调 `hide_launcher` 命令(后端要顺带 `set_ignore_cursor_events(true)` 并 emit close),`src/lib/window.ts` 只保留 `resizeLauncherToContent`;`core:window:allow-hide` 权限随之移除。
- **D6 打开事件**:后端 `show` 后 emit `launcher://open`;前端监听后聚焦搜索框并全选旧搜索词(直接输入即覆盖,Esc 仍可关闭);不清空任何状态。

- **D7 验收反馈修正**:(a) 搜索框不再画焦点环——它是启动器内唯一且常驻的焦点目标,每次唤出都程序聚焦,环会常亮成一圈突兀边框且不传递信息;作为 styling §4/§10「outline-hidden 必配 ring」的显式例外记录进 spec。(b) 托盘右键**只弹菜单,不动面板**(面板保持原状;`hide_on_blur` 已因光标在托盘上而跳过);「打开启动器」菜单项语义为显示并聚焦。(c) 顺带清理脚手架遗留:删除 `greet` 命令(Rust + `api.ts`),`error.rs` 改为 `pub mod` 并补 `to_string()` 契约测试,避免 AppError 成为死代码。

## Requirements

### R1 窗口配置与权限

- R1.1 `tauri.conf.json5` 主窗口改为:`width 800, height 600, center true, visible false, focus false, decorations false, transparent true, alwaysOnTop true, skipTaskbar true, resizable false, shadow false`;每项一行中文注释;`title` 保留。
- R1.2 `capabilities/default.json`:保留 `core:default`、`core:window:allow-set-size`;移除 `core:window:allow-hide`(改走命令);更新中文 description。
- R1.3 `Cargo.toml`:`tauri` 加 feature `tray-icon`;新增 `tauri-plugin-global-shortcut`;`[target.'cfg(windows)'.dependencies] windows = "0.61"`(与 tauri 当前间接依赖同大版本,避免重复编译)只开 `Win32_Foundation`、`Win32_UI_Shell`、`Win32_UI_WindowsAndMessaging`;每项中文注释。

### R2 窗口服务(`src-tauri/src/launcher.rs` 领域模块)

- R2.1 常量:`MAIN_WINDOW = "main"`;事件 `LAUNCHER_OPENED = "launcher://open"`、`LAUNCHER_CLOSED = "launcher://close"`(注释指向前端 `EVENTS`)。
- R2.2 `show(app)`:取主窗口 → `set_ignore_cursor_events(false)` → 按当前显示器工作区定位(水平居中且 ≥ 工作区左缘,顶边 = 工作区高 1/4)→ `show` → `set_focus` → emit `launcher://open`(payload 空结构体或 `()`)。
- R2.3 `hide(app)`:`set_ignore_cursor_events(true)` → `hide` → emit `launcher://close`。
- R2.4 `toggle(app)`:`is_visible && is_focused` → hide;否则 show。
- R2.5 `hide_on_blur(app)`:光标位于托盘图标 rect 内 → 不动;否则 hide。
- R2.6 `toggle_from_tray(app)`:`is_visible` → hide;否则 show。
- R2.7 `init_hidden(app)`:`set_ignore_cursor_events(true)` + `hide`,不 emit。
- R2.8 窗口 API 调用失败:`log::warn!` 中文文案并继续,不 panic、不向上抛(这些函数无返回值)。
- R2.9 `position_anchored` 的坐标计算抽为纯函数 `anchor_position(work: Rect-like, window_size) -> (i32, i32)` 并单测(居中、贴左缘、1/4 顶边)。

### R3 全局快捷键与窗口事件(`lib.rs` `setup_desktop`)

- R3.1 `#[cfg(desktop)] fn setup_desktop(app: &tauri::App) -> Result<(), Box<dyn Error>>`,在 `setup` 中调用;失败返回 `Err` 不 panic。
- R3.2 注册 `tauri_plugin_global_shortcut`,`on_shortcut("alt+enter")` 仅在 `ShortcutState::Pressed` 时调 `launcher::toggle`。注册失败(快捷键被占用)→ `log::error!` 并继续启动(托盘仍可用)。
- R3.3 主窗口 `on_window_event`:`Focused(false)` → `hide_on_blur`;`CloseRequested` → `api.prevent_close()` + `hide`(D3)。
- R3.4 Windows 下调用 `launcher::install_platform_hooks(&window)` 安装 `SC_KEYMENU` 拦截;非 Windows 无此调用(整段 `#[cfg(windows)]`)。
- R3.5 顺序:插件 → 快捷键 → 平台钩子 → 窗口事件 → 托盘 → `init_hidden`,并在代码注释写明为何 `init_hidden` 最后。

### R4 托盘(`src-tauri/src/tray.rs`)

- R4.1 `TRAY_ID = "z-tools-tray"`;菜单:「打开启动器」→ `launcher::show`;分隔;「退出」→ `app.exit(0)`。
- R4.2 `show_menu_on_left_click(false)`;`on_tray_icon_event` 只响应左键 `Click` + `MouseButtonState::Up` → `toggle_from_tray`;右键不做窗口操作(系统弹菜单),中键忽略。
- R4.3 图标 `app.default_window_icon()`;tooltip 为 `productName`(读 `app.package_info().name` 或硬编码「z-tools」)。
- R4.4 `setup(app) -> tauri::Result<()>`。

### R5 Windows 平台钩子(`src-tauri/src/launcher/windows.rs`)

- R5.1 `pub fn suppress_alt_sysmenu(hwnd: isize)`:`SetWindowSubclass` + 自定义 `SUBCLASS_ID`;子类过程拦 `WM_SYSCOMMAND` 且 `(wparam & 0xFFF0) == SC_KEYMENU` 返回 `LRESULT(0)`;`WM_NCDESTROY` 时 `RemoveWindowSubclass`;其余 `DefSubclassProc`。
- R5.2 `unsafe` 块最小化并注释理由;安装失败 `log::warn!`。
- R5.3 `launcher.rs` 中 `#[cfg(windows)] mod windows;` + `#[cfg(windows)] pub fn install_platform_hooks(window)`(取 `window.hwnd()`,失败 `warn`)。非 Windows 不编译该文件,无 stub。

### R6 命令与前端对接

- R6.1 `src-tauri/src/commands/launcher.rs`:`#[tauri::command] pub fn hide_launcher(app: AppHandle)`(不可失败,返回 `()`),注册到 `generate_handler!`。
- R6.2 `src/lib/api.ts` 新增 `hideLauncher()`:非 Tauri no-op,否则 `invoke<void>("hide_launcher")`;`src/lib/window.ts` 删除 `hideLauncher`,只留 `resizeLauncherToContent`;`LauncherPanel.vue` 改 import。
- R6.3 `src/lib/events.ts`:`EVENTS.LAUNCHER_OPENED = "launcher://open"`、`EVENTS.LAUNCHER_CLOSED = "launcher://close"`,`EventPayloads` 均为 `null`(无 payload)。
- R6.4 `src/composables/useTauriEvent.ts`:按 `composable-guidelines.md` §2 示意实现(非 Tauri 不订阅;处理卸载竞态;`onUnmounted` unlisten)。这是唯一允许 import `@tauri-apps/api/event` 的文件。
- R6.5 `LauncherPanel.vue`:`useTauriEvent(EVENTS.LAUNCHER_OPENED, () => searchBar.value?.focus({ selectAll: true }))`;`SearchInput.focus` 增加可选参数 `{ selectAll?: boolean }`(默认光标到末尾,`selectAll` 时 `select()`),两层搜索栏透传。
- R6.7 快捷键单一来源(用户要求预留可配置口子):Rust `launcher::DEFAULT_TOGGLE_SHORTCUT` 是字面量唯一出处;`on_shortcut` 注册与新增命令 `get_toggle_shortcut() -> &'static str` 均读它;前端 `api.ts` `getToggleShortcut()`(浏览器回退默认值)+ `lib/launcher/keyLabels.ts` `parseShortcut()` 纯函数(带测试)→ `LauncherPanel` 挂载时读取并传给两个搜索栏渲染键帽;注释 / 文档不再写死具体键位。
- R6.6 `index.css`:`body` 移除 `background-color`,注释改写为「透明窗口:文档底透明,表面色由面板根承担;深色模式仍靠 html 的 color-scheme」。

### R7 工程

- R7.1 后端门禁 `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`(在 `src-tauri/`,需先 `mkdir -p ../dist` 或先 `bun run build`)全绿;前端五条门禁全绿。
- R7.2 `bun run tauri dev` 可启动。

## Acceptance Criteria

- [ ] AC1 `bun run tauri dev` 启动后面板不出现,托盘有图标;`Alt+Enter` 弹出透明无边框面板,位于当前显示器工作区水平居中、顶边约 1/4 高,搜索框自动聚焦;再按 `Alt+Enter` 收起。
- [ ] AC2 面板显示时单按 `Alt`(或 `Alt` 后 `Enter`)不弹出系统菜单(Windows)。
- [ ] AC3 点击面板外任意位置面板收起;面板收起后,点击原面板区域的桌面 / 其他窗口能正常命中(无透明残留挡点击)。
- [ ] AC4 托盘左键 toggle;右键只弹菜单、面板保持原状,「打开启动器」显示并聚焦面板,「退出」结束进程;鼠标停在托盘图标上点击时不会先因失焦收起再被 toggle 反向打开。
- [ ] AC5 主页 `Esc` 收起面板(经 `hide_launcher`);工具页 `Esc` 仍只返回主页。
- [ ] AC6 面板显示时 `Alt+F4` 只收起不退出;再按 `Alt+Enter` 可唤回。
- [ ] AC7 唤出时搜索框聚焦且旧搜索词被全选;直接输入覆盖旧词。
- [ ] AC8 面板四个圆角外透明(无白 / 黑底),深浅色模式下面板本身颜色正确;主页内容变化时窗口高度随之变化,工具页高 600。
- [ ] AC9 `cargo test` 含 `anchor_position` 纯函数测试(居中、贴左缘、1/4 顶边);两侧门禁全绿;`grep` 确认 `@tauri-apps/api/event` 只在 `useTauriEvent.ts`、`@tauri-apps/api/window` 只在 `lib/window.ts`、`invoke` 只在 `lib/api.ts`。
- [ ] AC11 搜索栏键帽来自后端 `get_toggle_shortcut`;`grep -rni "alt+enter" src src-tauri/src` 只命中 `launcher.rs` 常量、`api.ts` 回退值与测试用例。
- [ ] AC10 浏览器预览 `bun run dev` 仍可打开且无报错(所有窗口 / 事件调用降级)。

## Out of Scope

- 快捷键可配置 / 设置页;多显示器下记忆上次位置;开机自启;窗口出现 / 消失动画;macOS / Linux 平台专属行为验证(编译通过即可,行为不承诺)。
- 剪贴板等任何真实工具。
- `[profile.release]` 优化、CSP 收紧。

## Risks / Deferred

- `tauri dev` 下热重载会重建 WebView 但不重跑 `setup`:改 Rust 侧需重启进程。
- 失焦隐藏在开发时切到 DevTools / 编辑器也会触发,属预期。
- `alt+enter` 若被其他软件占用,注册失败仅记日志,用户只能用托盘打开;设置页留后续。
- `transparent: true` 在 Windows 上依赖 WebView2 透明支持,老版本 WebView2 可能出现黑底;不做兼容处理。
