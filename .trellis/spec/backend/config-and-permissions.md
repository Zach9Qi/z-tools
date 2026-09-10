# 配置、权限与依赖

> 涉及 `tauri.conf.json5`、落盘目录、`capabilities/`、`Cargo.toml`、日志与平台差异。改这些文件前先读。

---

## 1. `tauri.conf.json5`

- 用 JSON5 是为了**每个配置项都能带中文注释**(`Cargo.toml` 里 `config-json5` feature 的注释);新增任何配置项必须在其上方写一行说明。
- `version` 只能由 `bun run release` 修改(三处同步,见 `../guides/project-conventions.md`);`.prettierignore` 排除了该文件,因为 Prettier 会去掉键引号导致版本脚本正则失配。
- `【新项目必改】` 标记的项(此文件的 `identifier`,`Cargo.toml` 的 `authors`)是模板占位;`productName`、窗口 `title` 等其余占位见 README「新项目必改清单」。
- 窗口 `label` 是代码与 capabilities 的关联键;新增窗口时同时新增或更新 capability 文件。
- `security.csp` 当前为 `null`(开发方便);上线前收紧为官方基线 `default-src 'self'; connect-src ipc: http://ipc.localhost`,**并追加 `img-src 'self' asset: http://asset.localhost`**(否则下一条 assetProtocol 的图片全部加载失败),这两句注释都留在原处。
- `security.assetProtocol`(现状 `enable: true`,`scope: ["$APPLOCALDATA/clipboard/images/**"]`):让前端用 `convertFileSrc()` 把 Rust 返回的绝对路径变成可直接放进 `<img src>` 的 `asset://` URL。要点:
  - 必须同时开 `tauri` 的 `protocol-asset` feature(`Cargo.toml` 注释「缺一不可」),否则 `tauri-build` 报错;
  - scope 只放具体目录,不放 `$APPLOCALDATA/**`;变量用 `$APPLOCALDATA`(= `app_local_data_dir()`,与 Rust 落盘目录同源,见 §2),**不用 `$APPDATA`**;
  - asset 协议不走 ACL,`capabilities/` 里没有对应权限项(`gen/schemas/desktop-schema.json` 无 `core:asset*`),不要去 capability 里找;
  - `convertFileSrc` 在前端只允许出现在 `src/lib/api/**`(`toAssetUrl()`,见 `../frontend/ipc-guidelines.md`)。
- `build.devUrl` 端口 1420 与 `vite.config.ts` 的 `strictPort` 一致,改一处必须改另一处。端口被占时的处理规则(禁止 kill、`tauri dev` 仅用户手测)见 `../guides/project-conventions.md` §4.1。
- 主窗口现状是**启动器形态**:`decorations:false` + `transparent:true` + `alwaysOnTop:true` + `skipTaskbar:true` + `shadow:false`,`visible:false` / `focus:false` 启动即隐藏在托盘(由 `setup_desktop` 的 `init_hidden` 接管)。其中:
  - `width` 是面板宽度的**全仓库唯一定义处**,前端 `setSize` 时沿用 `window.innerWidth`,不在 TS 里再写一份宽度;
  - `resizable:false`:高度由前端按内容 `setSize` 同步、宽度固定,用户拖拽缩放会和它打架;同理**不设 `minHeight`**,否则主页内容很少时窗口收不小;
  - `height` 只在前端首次 `setSize` 之前生效;`center` 只管创建时,每次唤出前 Rust 侧会按当前显示器重新定位;
  - `transparent:true` 要求前端 `html` / `body` 不设底色(见 `../frontend/styling-guidelines.md` §3);`shadow:false` 是因为系统阴影会在透明区画出一圈方框。

## 2. 落盘目录约定(项目级)

**所有持久化文件统一放在 `app.path().app_local_data_dir()` 之下**(Windows:`%LOCALAPPDATA%\<identifier>\`;配置 / scope 变量写 `$APPLOCALDATA`)。**禁止 `app_data_dir()` / `$APPDATA`**:在 Windows 上那是 Roaming 目录,会随域账户漫游,剪贴板历史、缓存这类机器本地数据不该漫游;更重要的是用户删一个目录就能完整清理,不需要在两处找。

当前该目录下的内容清单(新增落盘项时在此登记):

| 子目录 / 文件 | 谁写 | 说明 |
|---|---|---|
| `EBWebView/` | WebView2(Tauri 默认) | 前端缓存、localStorage 等,不由我们管理 |
| `logs/` | `tauri-plugin-log`(`LogDir` = `app_log_dir()`) | 日志文件 |
| `clipboard/history.db`(+ `-wal` / `-shm`) | `lib.rs::setup_clipboard` → `ClipboardStore::open` | SQLite 历史库;打不开则应用启动失败,不自动恢复 |
| `clipboard/images/<hash>.png` / `<hash>.thumb.png` | `clipboard.rs::record` | 图片原图与缩略图;asset 协议 scope 只覆盖此目录 |

规则:

- 取路径只在 `setup_xxx` 里做一次(`app.path().app_local_data_dir()?.join("clipboard")`),`create_dir_all` 后把 `PathBuf` 交给托管状态;命令 / 领域层不再各自取路径。
- 后续的设置文件、缓存、导出临时文件同样放这里的子目录,每个领域一个子目录(`clipboard/`、将来 `settings/`),不直接堆在根下。
- 验收时确认 `%APPDATA%\<identifier>\`(Roaming)**没有被创建**;出现了就是有人调了 `app_data_dir()`。
- 持久化的写法(sqlx / 迁移 / 恢复策略)见 `persistence.md`。

## 3. capabilities(权限最小化)

- 当前只有 `capabilities/default.json`:`windows: ["main"]`,`permissions: ["core:default", "core:window:allow-set-size"]`。`allow-set-size` 给前端按内容高度同步窗口;**不给 `core:window:allow-hide` / `allow-show`**——显示 / 隐藏由 Rust 侧控制(全局快捷键、托盘、失焦回调、`hide_launcher` 命令),隐藏时还要顺带 `set_ignore_cursor_events(true)` 并 emit 事件,前端直调 `hide()` 会绕过这两步,所以前端只能走 `hideLauncher()` 命令封装。
- 新增插件时:先加 `<plugin>:default`,再按需追加具体 `allow-*`;**不要** `fs: "**"` / `http: "http://**"` 全放;fs scope 限定到 `$APPLOCALDATA/<领域>/**` 等具体目录(不是 `$APPDATA`,见 §2)。
- 权限按**窗口**与**平台**拆文件:`default.json`(所有窗口通用)、`desktop.json`(`platforms: ["macOS","windows","linux"]`)、`<label>.json`(某个窗口专属)。当前单文件起步,出现平台专属权限或新窗口时再拆。
- `windows` 字段写明确 label,不用 `"*"`。
- 每个 capability 文件的 `description` 用中文写清「给谁、为什么」。
- 自定义 `#[tauri::command]` 不需要 capability 项(剪贴板 6 个命令没有加任何权限);asset 协议同样不走 ACL(§1)。

## 4. `Cargo.toml`

- 非通用依赖上方一行中文注释说明**用途 + 选型原因**(现有:`thiserror` 统一错误、`log` + `tauri-plugin-log` 日志门面、`tauri` 的 `config-json5` / `tray-icon` / `protocol-asset` feature、`tauri-plugin-global-shortcut` 全局快捷键、`arboard` / `sqlx` / `image` / `blake3` 剪贴板一组、`windows` 平台 crate);`serde` 这类人人都懂的基础依赖可省略。`serde_json` 在 `error.rs` 测试断言 wire 格式,现在也用于 `files` 列 JSON 编解码。
- `tauri` feature:
  - `tray-icon`:启动器常驻后台且不在任务栏显示,托盘是用户开合面板 / 退出的唯一可见入口;托盘在 Rust 侧 `TrayIconBuilder` 创建(`tray.rs`),不走 `tauri.conf.json5` 的 `app.trayIcon`,因为菜单与事件映射本来就要写代码,配置里再写一半反而分散。
  - `protocol-asset`:与 `tauri.conf.json5` 的 `assetProtocol` 配套,缺一不可(§1)。
- `tauri-plugin-global-shortcut`:在任意前台应用下唤出 / 收起面板;默认键位只在 `launcher::DEFAULT_TOGGLE_SHORTCUT` 一处定义(见 §8)。插件在 `setup_desktop` 内用 `app.handle().plugin(...)` 注册而不是 Builder 链上,因为它本身只在桌面端可用,与 `#[cfg(desktop)]` 的边界一致。
- 剪贴板一组依赖(注释里写明为什么选它、为什么不选别的):

| 依赖 | 版本 / feature | 用途 | 注释里必须写的取舍 |
|---|---|---|---|
| `arboard` | `3.6`(默认 feature 含 `image-data`) | 跨平台剪贴板**读写**(`clipboard/backend.rs`) | Windows 侧只依赖 `windows-sys`,不引入第二份 `windows` crate;不选 `clipboard-rs`(小众且依赖 `windows 0.59` 冲突)、不选官方 clipboard-manager 插件(无文件列表、无监听) |
| `sqlx` | `0.8`,`default-features = false`,`["runtime-tokio", "sqlite", "migrate", "macros"]` | 历史库 | 只用运行时 `sqlx::query*`,**不用 `query!` 宏**(避免编译期依赖 `DATABASE_URL`);`macros` 只为 `sqlx::migrate!`;详见 `persistence.md` |
| `image` | `0.25`,`default-features = false`,`["png"]` | RGBA8 ↔ PNG 编解码、缩略图 | 与 arboard 内部依赖同一大版本不重复编译;只开 png |
| `blake3` | `1` | 内容哈希去重 | 需要跨版本稳定的哈希,std `DefaultHasher` 不保证 |
| `tokio`(dev) | `1`,`["macros", "rt"]` | `#[tokio::test]` 驱动 sqlx 内存库单测 | 运行时代码只用 `tauri::async_runtime`,不直接依赖 tokio |

- `edition = "2024"`,`rust-version = "1.85"`;提升 MSRV 要同步 README 与 `Cargo.toml` 里 `rust-version` 的注释。
- 平台专属依赖放 `[target.'cfg(…)'.dependencies]`,不要无条件引入 windows / cocoa crate。现例 `[target.'cfg(windows)'.dependencies] windows = "0.61"`,features 与用途(注释里逐条列出,新增 feature 时补一行):

| feature | 谁用 |
|---|---|
| `Win32_Foundation` / `Win32_UI_Shell` / `Win32_UI_WindowsAndMessaging` | `launcher/windows.rs` 的 `SetWindowSubclass` 拦 Alt 菜单、`GetForegroundWindow` / `SetForegroundWindow` / `IsWindow` / `GetClassNameW`;`clipboard/windows.rs` 的消息窗口(`RegisterClassW` / `CreateWindowExW` / `GetMessageW` / `PostMessageW`) |
| `Win32_Graphics_Gdi` | `WNDCLASSW.hbrBackground` 的 `HBRUSH` 类型定义在此,注册窗口类必需 |
| `Win32_System_DataExchange` | `AddClipboardFormatListener` / `RemoveClipboardFormatListener` / `GetClipboardSequenceNumber` |
| `Win32_System_LibraryLoader` | `GetModuleHandleW` 取当前模块句柄以注册窗口类 |
| `Win32_UI_Input_KeyboardAndMouse` | `SendInput` 模拟 Ctrl+V |

  大版本对齐 tauri 当前的间接依赖(`Cargo.lock` 可查),否则会编译两份 `windows` crate;**核对方法 `cargo tree -i windows`**,输出里只应出现一个版本;升级 tauri 或新增依赖后顺带跑一次。
- `[lib] name` 保留 `_lib` 后缀与三种 `crate-type`(`Cargo.toml` 注释解释 Windows 冲突与用途)。
- 尚未配置、需要时另开任务讨论:
  - `[profile.release]`:一种取向是 `panic = "abort"`、`codegen-units = 1`、`lto = true`、`strip = true`(体积小、启动快);另一种是保留 unwind 与符号方便崩溃分析。两种都有理由,按发布需求选。
  - `[lints.clippy]` 与 `clippy.toml`:可用 `await_holding_lock` / `unwrap_used` / `unused_async` 等 lint 机制化本规范中的禁止项;本仓库目前靠 `-D warnings` + 评审。

## 5. 日志

- 业务代码只用 `log::{error, warn, info, debug, trace}!` 宏,**禁止 `println!` / `eprintln!` / `dbg!`**(README 与 `Cargo.toml` 注释)。
- 级别在 `lib.rs` 由 `cfg!(debug_assertions)` 决定:debug 构建 `Debug`,发布 `Info`。
- 日志文案中文,带上下文(哪个命令 / 哪个文件),不带敏感信息(token、完整路径中的用户名)。
- 当前 `tauri-plugin-log` 使用默认 target(stdout + LogDir);Windows release 下 stdout 可能阻塞,若发布后出现卡顿,把 release 目标改为仅 `LogDir` 并保留该注释。
- 第三方 crate 噪音用 `.level_for("<target>", LevelFilter::Warn)` 压低,不要整体降级。现例:`.level_for("sqlx", log::LevelFilter::Warn)`——sqlx 在 Debug 级把每条 SQL(含整段迁移脚本)打一行,会淹没业务日志。
- 剪贴板监听器这类「没有调用方」的后台路径:失败只记日志(`record()` 内 `warn`、监听窗口创建失败 `error`),不 panic、不让线程带着错误退出而无声。

## 6. 平台差异

- 应用侧用 `#[cfg(windows)]`(与 `target_os = "windows"` 等价,本仓库用短写)/ `#[cfg(target_os = "macos" | "linux")]`;`#[cfg(desktop)]` / `#[cfg(mobile)]` 只用于 Builder 装配(`lib.rs` 的 `setup_desktop`)与 `mobile_entry_point`。
- 平台实现拆文件 `src/<domain>/{windows,macos,linux}.rs`,在 `<domain>.rs` 里 `#[cfg(windows)] mod windows;` 并暴露统一函数签名。两个现例:
  - `launcher/windows.rs`:`SetWindowSubclass` 给无边框窗口装子类过程,拦 `WM_SYSCOMMAND` 的 `SC_KEYMENU`(否则含 Alt 的唤出键会弹出系统菜单),`WM_NCDESTROY` 时自摧除;另有 `current_foreground()` / `is_taskbar(hwnd)` / `activate(hwnd)` 供「粘贴回原窗口」使用。`launcher.rs` 以 `#[cfg(windows)] pub fn install_platform_hooks` 包一层取 `hwnd`,调用点 `lib.rs` 同样 `#[cfg(windows)]` 守卫。安装失败只 `log::warn!`,Alt 菜单只是体验瑕疵,不值得让应用起不来。
  - `clipboard/windows.rs`:`run_monitor(app)` / `stop_monitor(hwnd)` / `send_paste()`,父模块 `clipboard.rs` 用 `#[cfg(windows)] pub use windows::{ClipboardWatcher, run_monitor, stop_monitor};` 转出。**跨平台的读写(`backend.rs`,arboard)与存储(`store.rs`)不带 cfg**,后续 macOS / Linux 只补一份同签名平台文件。
- 非 Windows **不编译平台文件也不提供 stub**(没有东西可拦,空函数只会让人以为有行为);但用户可感知的能力要明确报错:`clipboard::paste` 的 `#[cfg(not(windows))]` 版本返回 `AppError::Unsupported("剪贴板粘贴")`,不静默 no-op。
- **非 Windows 编译路径(Ubuntu CI 会跑 `clippy -D warnings`)**:
  - 只被 `#[cfg(windows)]` 代码引用的**跨平台**项,在非 Windows 上会变成 dead_code 被 `-D warnings` 拦下(如 `record()`、`backend::read_snapshot()` 只有监听线程调用)。本仓库的做法是把这些模块公开:`lib.rs` 里 `pub mod clipboard;`,`clipboard.rs` 里 `pub mod backend; pub mod store;`——与 `pub mod error` 同一先例,理由必须写在模块文档里(`clipboard.rs` 的「可见性」段落)。不要用 `#[allow(dead_code)]` 一把盖住。
  - `.run(|_app, _event| …)` 闭包参数在非 Windows 下未使用,形参用下划线前缀并注释「目前只有 Windows 在退出时有事可做」。
  - 命令层对平台的差异只体现在领域函数的 cfg 版本上(`paste` 有两份),命令签名本身不带 cfg,`generate_handler!` 也不分平台。
- `main.rs` 的 `windows_subsystem = "windows"` 属性不可删(否则 Windows 发布版弹控制台)。

## 7. 构建

- `build.rs` 只有 `tauri_build::build()`。
- CI 在跑 Rust 门禁前 `mkdir -p ../dist` 满足 `frontendDist` 检查(`ci.yml` 注释);本地 `cargo test` 若报 dist 不存在,同样处理,不要改 `frontendDist`。
- `sqlx::migrate!("./migrations")` 在编译期读目录;`src-tauri/migrations/` 必须提交进仓库(见 `persistence.md`)。

## 8. 启动器窗口约定(`launcher.rs` / `tray.rs` / `lib.rs::setup_desktop`)

这些是踩过坑后定下的时序与职责划分,改窗口行为前先读;它们都在代码注释里有对应说明,这里只汇总「为什么」。

| 约定 | 为什么 |
|---|---|
| **显示 / 隐藏由 Rust 控制**,入口只有全局快捷键、托盘、失焦回调、`CloseRequested`、`hide_launcher` 命令、剪贴板 `paste()`;前端不接触 `show()` / `hide()` | 显示隐藏不是单一 API 调用而是一串时序(下两行),分散到前端会漏步 |
| `show`:**`remember_foreground`(仅 Windows)** → `set_ignore_cursor_events(false)` → 定位 → `show` → `set_focus` → emit `launcher://open`;`hide`:`set_ignore_cursor_events(true)` → `hide` → emit `launcher://close` | 透明窗口隐藏后仍可能残留命中区挡住桌面 / 其他窗口的点击,隐藏期间必须忽略鼠标事件;显示前若不恢复,面板点不动。前台窗口必须在 show / set_focus 之前记,之后前台就是我们自己 |
| `remember_foreground` 写 `PreviousForeground(AtomicIsize)` 托管状态;前台是启动器自己或任务栏(类名 `Shell_TrayWnd` / `Shell_SecondaryTrayWnd`,`launcher/windows.rs::is_taskbar`)时**保留上一次记录** | 从托盘点开时前台是任务栏,记下它会让粘贴把 Ctrl+V 发给任务栏;可见但失焦后再次唤出时前台是自己,同样不该覆盖 |
| `hide()` **幂等**:`is_visible()` 为 false 时直接 return、不 emit;查不到可见性按可见处理 | 粘贴流程先 `SetForegroundWindow` 到原窗口(触发失焦回调 → hide)再显式调 hide,不幂等前端会收到两次 `launcher://close`;宁可多隐藏一次不可漏隐藏 |
| 定位用 `anchor_position` 纯函数:水平居中(不小于工作区左缘)、顶边 = 工作区高 1/4,`PhysicalPosition` | 顶边只取决于工作区,内容增高时窗口向下生长、搜索框不动;抽成纯函数才能单测 |
| `toggle`(快捷键)只在 `visible && focused` 时隐藏,否则显示并抢焦点;快捷键回调只处理 `ShortcutState::Pressed` | 可见但失焦的面板按用户意图是「叫回来」;不过滞 Pressed 会在松键时再 toggle 一次 |
| **失焦隐藏** + **托盘豁免**:`Focused(false)` → `hide_on_blur`,光标在托盘图标 rect 内则不动 | 点托盘也会先触发失焦;若此时收起,紧接的托盘点击看到的是「已隐藏」反而取反打开。托盘矩形凭 `launcher::TRAY_ID` 查,因此托盘要在 `init_hidden` 之前建好;任一信息拿不到时按「不在托盘上」处理——宁可误收起,不可不收起 |
| **托盘右键不动面板**,只让系统弹菜单;左键 `Click`+`Up` → `toggle_from_tray`(只看可见性);「打开启动器」菜单项语义固定为显示并聚焦 | 用户右键可能只是想看菜单或退出,面板不应因此消失;托盘点击时焦点必然已离开面板,不再判焦点 |
| `CloseRequested` 拦为隐藏(`api.prevent_close()` + `hide`);退出走托盘菜单 `launcher::quit`:先 `destroy` 主窗口,等 `Destroyed` 再 `app.exit(0)` | 无边框窗口没有关闭按钮,但 Alt+F4 仍会触发;启动器应常驻,误按不该退进程。不能 `close()`(会被拦成隐藏);也不能直接 `exit(0)`:Windows 上 WebView2 注销 `Chrome_WidgetWin_0` 时父窗口还在会报 Error 1412 |
| `setup_desktop` 顺序:插件 → 快捷键 → **clipboard(`setup_clipboard`:store + Windows 监听线程)** → 平台钩子 + `PreviousForeground` + 窗口事件 → 托盘 → `init_hidden` **最后且不 emit** | 插件不先注册 `global_shortcut()` 取不到;clipboard 只依赖 `app_local_data_dir`、不依赖窗口,放托盘前且库打不开要早失败(否则列表命令因无托管状态 panic);托盘要在 `init_hidden` 前建好(上述豁免);`init_hidden` 时前端尚未加载,emit 没人听 |
| Builder 用 `.build(ctx)?.run(\|app, event\| …)` 而不是链式 `.run(ctx)`;`RunEvent::Exit` 时取 `ClipboardWatcher.hwnd()` 调 `stop_monitor` | `launcher::quit` 的 `Destroyed` → `exit(0)`、系统关机等所有退出路径都经过 `Exit`,监听线程的消息窗口才能被正确摘除 |
| 快捷键注册失败 `log::error!` 后继续启动 | 被其他软件占用不应让应用起不来,托盘仍能打开面板 |
| **唤出快捷键单一常量** `launcher::DEFAULT_TOGGLE_SHORTCUT`(plugin 语法),注册、日志、`get_toggle_shortcut` 命令都读它;前端经 `getToggleShortcut()` 取当前生效值渲染键帽 | 为后续可配置预留口子:到时只改命令实现读设置,前端不动。因此代码注释与本规范都只写「默认唤出键」,不写具体键位 |
| 窗口 API 失败一律 `log::warn!` 继续,领域函数不返回 `Result` | 调用方(托盘 / 快捷键回调)没能力处理这些错误;统一在领域层吞掉并记日志 |
| 粘贴回原窗口:`SetForegroundWindow` 必须在 `hide()` **之前**调用(`clipboard::paste`) | hide 后本进程可能失去前台进程资格,`SetForegroundWindow` 会静默失败只闪任务栏 |

## 9. 禁止

- 无注释的配置项 / 非通用依赖。
- `app_data_dir()` / `$APPDATA`(Roaming);落盘一律 `app_local_data_dir()` / `$APPLOCALDATA`(§2)。
- assetProtocol scope 通配整个 `$APPLOCALDATA`。
- 给前端 `core:window:allow-hide` / `allow-show`(显示 / 隐藏由 Rust 控制,见 §3、§8)。
- 在 `launcher::DEFAULT_TOGGLE_SHORTCUT` 之外写唤出键字面量(注释、日志、文档一律写「默认唤出键」)。
- 手改三处版本号中的任何一处。
- capabilities 里 `windows: ["*"]`、通配 scope。
- `println!` 系列。
- 在通用代码里内联大段 `#[cfg(target_os)]` 分支;用 `#[allow(dead_code)]` 掩盖非 Windows 下的未使用告警(改为 `pub mod` 并写理由)。
- 引入第二份 `windows` crate(新增依赖后 `cargo tree -i windows` 核对)。
