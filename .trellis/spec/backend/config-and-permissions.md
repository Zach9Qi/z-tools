# 配置、权限与依赖

> 涉及 `tauri.conf.json5`、`capabilities/`、`Cargo.toml`、日志与平台差异。改这些文件前先读。

---

## 1. `tauri.conf.json5`

- 用 JSON5 是为了**每个配置项都能带中文注释**(`Cargo.toml` 里 `config-json5` feature 的注释);新增任何配置项必须在其上方写一行说明。
- `version` 只能由 `bun run release` 修改(三处同步,见 `../guides/project-conventions.md`);`.prettierignore` 排除了该文件,因为 Prettier 会去掉键引号导致版本脚本正则失配。
- `【新项目必改】` 标记的项(此文件的 `identifier`,`Cargo.toml` 的 `authors`)是模板占位;`productName`、窗口 `title` 等其余占位见 README「新项目必改清单」。
- 窗口 `label` 是代码与 capabilities 的关联键;新增窗口时同时新增或更新 capability 文件。
- `security.csp` 当前为 `null`(开发方便);上线前收紧为官方基线 `default-src 'self'; connect-src ipc: http://ipc.localhost`,并把这条注释留在原处。
- `build.devUrl` 端口 1420 与 `vite.config.ts` 的 `strictPort` 一致,改一处必须改另一处。
- 主窗口现状是**启动器形态**:`decorations:false` + `transparent:true` + `alwaysOnTop:true` + `skipTaskbar:true` + `shadow:false`,`visible:false` / `focus:false` 启动即隐藏在托盘(由 `setup_desktop` 的 `init_hidden` 接管)。其中:
  - `width` 是面板宽度的**全仓库唯一定义处**,前端 `setSize` 时沿用 `window.innerWidth`,不在 TS 里再写一份宽度;
  - `resizable:false`:高度由前端按内容 `setSize` 同步、宽度固定,用户拖拽缩放会和它打架;同理**不设 `minHeight`**,否则主页内容很少时窗口收不小;
  - `height` 只在前端首次 `setSize` 之前生效;`center` 只管创建时,每次唤出前 Rust 侧会按当前显示器重新定位;
  - `transparent:true` 要求前端 `html` / `body` 不设底色(见 `../frontend/styling-guidelines.md` §3);`shadow:false` 是因为系统阴影会在透明区画出一圈方框。

## 2. capabilities(权限最小化)

- 当前只有 `capabilities/default.json`:`windows: ["main"]`,`permissions: ["core:default", "core:window:allow-set-size"]`。`allow-set-size` 给前端按内容高度同步窗口;**不给 `core:window:allow-hide` / `allow-show`**——显示 / 隐藏由 Rust 侧控制(全局快捷键、托盘、失焦回调、`hide_launcher` 命令),隐藏时还要顺带 `set_ignore_cursor_events(true)` 并 emit 事件,前端直调 `hide()` 会绕过这两步,所以前端只能走 `hideLauncher()` 命令封装。
- 新增插件时:先加 `<plugin>:default`,再按需追加具体 `allow-*`;**不要** `fs: "**"` / `http: "http://**"` 全放;fs scope 限定到 `$APPDATA` 等具体目录。
- 权限按**窗口**与**平台**拆文件:`default.json`(所有窗口通用)、`desktop.json`(`platforms: ["macOS","windows","linux"]`)、`<label>.json`(某个窗口专属)。当前单文件起步,出现平台专属权限或新窗口时再拆。
- `windows` 字段写明确 label,不用 `"*"`。
- 每个 capability 文件的 `description` 用中文写清「给谁、为什么」。

## 3. `Cargo.toml`

- 非通用依赖上方一行中文注释说明用途与选型原因(现有:`thiserror` 统一错误、`log` + `tauri-plugin-log` 日志门面、`tauri` 的 `config-json5` / `tray-icon` feature、`tauri-plugin-global-shortcut` 全局快捷键、`windows` 平台 crate);`serde` 这类人人都懂的基础依赖可省略。`serde_json` 当前只在 `error.rs` 的测试里断言 wire 格式,后续 IPC 结构体测试也会用,保留不删。
- `tauri` feature `tray-icon`:启动器常驻后台且不在任务栏显示,托盘是用户开合面板 / 退出的唯一可见入口;托盘在 Rust 侧 `TrayIconBuilder` 创建(`tray.rs`),不走 `tauri.conf.json5` 的 `app.trayIcon`,因为菜单与事件映射本来就要写代码,配置里再写一半反而分散。
- `tauri-plugin-global-shortcut`:在任意前台应用下唤出 / 收起面板;默认键位只在 `launcher::DEFAULT_TOGGLE_SHORTCUT` 一处定义(见 §7)。插件在 `setup_desktop` 内用 `app.handle().plugin(...)` 注册而不是 Builder 链上,因为它本身只在桌面端可用,与 `#[cfg(desktop)]` 的边界一致。
- `edition = "2024"`,`rust-version = "1.85"`;提升 MSRV 要同步 README 与 `Cargo.toml` 里 `rust-version` 的注释。
- 平台专属依赖放 `[target.'cfg(…)'.dependencies]`,不要无条件引入 windows / cocoa crate。现例 `[target.'cfg(windows)'.dependencies] windows = "0.61"`,只开 `Win32_Foundation` / `Win32_UI_Shell` / `Win32_UI_WindowsAndMessaging` 三个 feature(`launcher/windows.rs` 的 `SetWindowSubclass` 一套所需);大版本对齐 tauri 当前的间接依赖(`Cargo.lock` 可查),否则会编译两份 `windows` crate,升级 tauri 时顺带核对。
- `[lib] name` 保留 `_lib` 后缀与三种 `crate-type`(`Cargo.toml` 注释解释 Windows 冲突与用途)。
- `tokio` 若引入,精确列 features。
- 尚未配置、需要时另开任务讨论:
  - `[profile.release]`:一种取向是 `panic = "abort"`、`codegen-units = 1`、`lto = true`、`strip = true`(体积小、启动快);另一种是保留 unwind 与符号方便崩溃分析。两种都有理由,按发布需求选。
  - `[lints.clippy]` 与 `clippy.toml`:可用 `await_holding_lock` / `unwrap_used` / `unused_async` 等 lint 机制化本规范中的禁止项;本仓库目前靠 `-D warnings` + 评审。

## 4. 日志

- 业务代码只用 `log::{error, warn, info, debug, trace}!` 宏,**禁止 `println!` / `eprintln!` / `dbg!`**(README 与 `Cargo.toml` 注释)。
- 级别在 `lib.rs` 由 `cfg!(debug_assertions)` 决定:debug 构建 `Debug`,发布 `Info`。
- 日志文案中文,带上下文(哪个命令 / 哪个文件),不带敏感信息(token、完整路径中的用户名)。
- 当前 `tauri-plugin-log` 使用默认 target(stdout + LogDir);Windows release 下 stdout 可能阻塞,若发布后出现卡顿,把 release 目标改为仅 `LogDir` 并保留该注释。
- 第三方 crate 噪音用 `.level_for("tauri", LevelFilter::Warn)` 压低,不要整体降级。

## 5. 平台差异

- 应用侧用 `#[cfg(windows)]`(与 `target_os = "windows"` 等价,本仓库用短写)/ `#[cfg(target_os = "macos" | "linux")]`;`#[cfg(desktop)]` / `#[cfg(mobile)]` 只用于 Builder 装配(`lib.rs` 的 `setup_desktop`)与 `mobile_entry_point`。
- 平台实现拆文件 `src/<domain>/{windows,macos,linux}.rs`,在 `<domain>.rs` 里 `#[cfg(windows)] mod windows;` 并暴露统一函数签名。现例 `launcher/windows.rs`:`SetWindowSubclass` 给无边框窗口装子类过程,拦 `WM_SYSCOMMAND` 的 `SC_KEYMENU`(否则含 Alt 的唤出键会弹出系统菜单),`WM_NCDESTROY` 时自摧除;`launcher.rs` 以 `#[cfg(windows)] pub fn install_platform_hooks` 包一层取 `hwnd`,调用点 `lib.rs` 同样 `#[cfg(windows)]` 守卫,非 Windows **不编译该文件也不提供 stub**(没有东西可拦,空函数只会让人以为有行为)。安装失败只 `log::warn!`,Alt 菜单只是体验瑕疵,不值得让应用起不来。
- 不支持的平台要么编译期排除,要么返回 `AppError` 明确文案,不静默 no-op。
- `main.rs` 的 `windows_subsystem = "windows"` 属性不可删(否则 Windows 发布版弹控制台)。

## 6. 构建

- `build.rs` 只有 `tauri_build::build()`。
- CI 在跑 Rust 门禁前 `mkdir -p ../dist` 满足 `frontendDist` 检查(`ci.yml` 注释);本地 `cargo test` 若报 dist 不存在,同样处理,不要改 `frontendDist`。

## 7. 启动器窗口约定(`launcher.rs` / `tray.rs` / `lib.rs::setup_desktop`)

这些是踩过坑后定下的时序与职责划分,改窗口行为前先读;它们都在代码注释里有对应说明,这里只汇总「为什么」。

| 约定 | 为什么 |
|---|---|
| **显示 / 隐藏由 Rust 控制**,入口只有全局快捷键、托盘、失焦回调、`CloseRequested`、`hide_launcher` 命令;前端不接触 `show()` / `hide()` | 显示隐藏不是单一 API 调用而是一串时序(下两行),分散到前端会漏步 |
| `show`:`set_ignore_cursor_events(false)` → 定位 → `show` → `set_focus` → emit `launcher://open`;`hide`:`set_ignore_cursor_events(true)` → `hide` → emit `launcher://close` | 透明窗口隐藏后仍可能残留命中区挡住桌面 / 其他窗口的点击,隐藏期间必须忽略鼠标事件;显示前若不恢复,面板点不动 |
| 定位用 `anchor_position` 纯函数:水平居中(不小于工作区左缘)、顶边 = 工作区高 1/4,`PhysicalPosition` | 顶边只取决于工作区,内容增高时窗口向下生长、搜索框不动;抽成纯函数才能单测 |
| `toggle`(快捷键)只在 `visible && focused` 时隐藏,否则显示并抢焦点;快捷键回调只处理 `ShortcutState::Pressed` | 可见但失焦的面板按用户意图是「叫回来」;不过滞 Pressed 会在松键时再 toggle 一次 |
| **失焦隐藏** + **托盘豁免**:`Focused(false)` → `hide_on_blur`,光标在托盘图标 rect 内则不动 | 点托盘也会先触发失焦;若此时收起,紧接的托盘点击看到的是「已隐藏」反而取反打开。托盘矩形凭 `launcher::TRAY_ID` 查,因此托盘要在 `init_hidden` 之前建好;任一信息拿不到时按「不在托盘上」处理——宁可误收起,不可不收起 |
| **托盘右键不动面板**,只让系统弹菜单;左键 `Click`+`Up` → `toggle_from_tray`(只看可见性);「打开启动器」菜单项语义固定为显示并聚焦 | 用户右键可能只是想看菜单或退出,面板不应因此消失;托盘点击时焦点必然已离开面板,不再判焦点 |
| `CloseRequested` 拦为隐藏(`api.prevent_close()` + `hide`),退出只走托盘菜单 `app.exit(0)` | 无边框窗口没有关闭按钮,但 Alt+F4 仍会触发;启动器应常驻,误按不该退进程 |
| `setup_desktop` 顺序:插件 → 快捷键 → 平台钩子 + 窗口事件 → 托盘 → `init_hidden` **最后且不 emit** | 插件不先注册 `global_shortcut()` 取不到;托盘要在 `init_hidden` 前建好(上述豁免);`init_hidden` 时前端尚未加载,emit 没人听 |
| 快捷键注册失败 `log::error!` 后继续启动 | 被其他软件占用不应让应用起不来,托盘仍能打开面板 |
| **唤出快捷键单一常量** `launcher::DEFAULT_TOGGLE_SHORTCUT`(plugin 语法),注册、日志、`get_toggle_shortcut` 命令都读它;前端经 `getToggleShortcut()` 取当前生效值渲染键帽 | 为后续可配置预留口子:到时只改命令实现读设置,前端不动。因此代码注释与本规范都只写「默认唤出键」,不写具体键位 |
| 窗口 API 失败一律 `log::warn!` 继续,领域函数不返回 `Result` | 调用方(托盘 / 快捷键回调)没能力处理这些错误;统一在领域层吞掉并记日志 |

## 8. 禁止

- 无注释的配置项 / 非通用依赖。
- 给前端 `core:window:allow-hide` / `allow-show`(显示 / 隐藏由 Rust 控制,见 §2、§7)。
- 在 `launcher::DEFAULT_TOGGLE_SHORTCUT` 之外写唤出键字面量(注释、日志、文档一律写「默认唤出键」)。
- 手改三处版本号中的任何一处。
- capabilities 里 `windows: ["*"]`、通配 scope。
- `println!` 系列。
- 在通用代码里内联大段 `#[cfg(target_os)]` 分支。
