# 技术设计:启动器窗口管理(Rust 侧)

> 需求见 `prd.md`。参考实现的行为已在 prd Background 列出,这里只写本仓库的落点、与参考的差异、契约与取舍。

## 1. 目录与分层

```text
src-tauri/
├── Cargo.toml                 # + tray-icon feature, tauri-plugin-global-shortcut, [target.'cfg(windows)'] windows 0.61
├── tauri.conf.json5           # 主窗口改为启动器形态(R1.1)
├── capabilities/default.json  # core:default + core:window:allow-set-size
└── src/
    ├── lib.rs                 # + mod launcher; mod tray; #[cfg(desktop)] fn setup_desktop; generate_handler 加 hide_launcher
    ├── commands.rs            # + pub mod launcher;
    ├── commands/
    │   └── launcher.rs        # hide_launcher 命令(薄:直接转 launcher::hide)
    ├── launcher.rs            # 领域层:窗口 show/hide/toggle/hide_on_blur/toggle_from_tray/init_hidden + 事件常量 + anchor_position 纯函数(含测试)
    ├── launcher/
    │   └── windows.rs         # #[cfg(windows)] SetWindowSubclass 拦 SC_KEYMENU
    └── tray.rs                # 托盘构建与事件
```

- 遵循 `backend/directory-structure.md`:`launcher.rs + launcher/` 风格,平台实现拆 `launcher/windows.rs`,`#[cfg(windows)] mod windows;` 只出现在 `launcher.rs`。
- 命令层只做转发;所有窗口逻辑在 `launcher.rs`,托盘与快捷键回调都调用它(三处消费者:命令 / 托盘 / 快捷键),这正是「领域层可被命令、托盘共同复用」的场景。
- `tray.rs` 依赖 `launcher.rs`(调 show / hide / toggle_from_tray);`launcher.rs` 依赖 `tray::TRAY_ID` 做 `cursor_on_tray`。为避免双向依赖,把 `TRAY_ID` 常量放 `launcher.rs`(`pub const TRAY_ID`),`tray.rs` 引用它。

## 2. 后端契约

### 2.1 `launcher.rs`

```rust
//! 启动器窗口的领域逻辑:显示 / 隐藏 / 定位 / 失焦策略;被命令层、托盘、全局快捷键共同调用。
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewWindow};

pub const MAIN_WINDOW: &str = "main";
pub const TRAY_ID: &str = "z-tools-tray";
/// 面板已显示并聚焦;前端 EVENTS.LAUNCHER_OPENED
pub const LAUNCHER_OPENED: &str = "launcher://open";
/// 面板已隐藏;前端 EVENTS.LAUNCHER_CLOSED
pub const LAUNCHER_CLOSED: &str = "launcher://close";
/// 默认唤出快捷键,字面量唯一出处;可配置化后由设置层覆盖
pub const DEFAULT_TOGGLE_SHORTCUT: &str = "alt+enter";

pub fn show<R: Runtime>(app: &AppHandle<R>);
pub fn hide<R: Runtime>(app: &AppHandle<R>);
pub fn toggle<R: Runtime>(app: &AppHandle<R>);
pub fn hide_on_blur<R: Runtime>(app: &AppHandle<R>);
pub fn toggle_from_tray<R: Runtime>(app: &AppHandle<R>);
pub fn init_hidden<R: Runtime>(app: &AppHandle<R>);
#[cfg(windows)]
pub fn install_platform_hooks<R: Runtime>(window: &WebviewWindow<R>);

/// 纯函数:给定工作区(x, y, w, h)与窗口尺寸(w, h),返回窗口左上角物理坐标
pub fn anchor_position(work: WorkArea, window: (u32, u32)) -> (i32, i32);
pub struct WorkArea { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
```

- 事件 payload:两个事件都无数据,emit `()`(序列化为 `null`),前端 `EventPayloads` 对应 `null`。不为此造空结构体。
- 错误策略:窗口 API 返回 `tauri::Result`,失败一律 `log::warn!("<动作>失败: {e}")` 继续;取不到主窗口 `log::warn!` 并 return。emit 失败同样 `warn`。这些函数不返回 `Result`,因为调用方(托盘 / 快捷键回调)无法处理错误,统一在此吞掉并记日志。
- `anchor_position`:
  ```rust
  let x = work.x as f64 + (work.width as f64 - win_w as f64) / 2.0;
  let x = x.max(work.x as f64);              // 工作区比窗口窄时贴左缘
  let y = work.y as f64 + work.height as f64 / 4.0;
  (x.round() as i32, y.round() as i32)
  ```
  单测:居中(1920×1080 工作区、800 宽 → x=560,y=270);带偏移的第二显示器(work.x=1920 → x=2480);窄工作区(600 宽 → x=work.x)。
- `cursor_on_tray`:`app.tray_by_id(TRAY_ID)?.rect()?` + `app.cursor_position()?`,`Rect` 的 `Position/Size` 枚举展开为 f64 后判包含;任何一步失败返回 `false`(宁可误收起,不可不收起)。

### 2.2 `launcher/windows.rs`

与参考一致(`SetWindowSubclass` / `DefSubclassProc` / `RemoveWindowSubclass`,`SUBCLASS_ID = 0x5A54_4F4C` 即 "ZTOL"),`windows` crate 0.61 API:`HWND(hwnd as *mut c_void)`、`BOOL::as_bool()`。`unsafe` 只包三处 FFI 调用,各带一行注释说明前置条件(hwnd 有效、回调签名匹配、子类已安装)。

### 2.3 `tray.rs`

```rust
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()>;
const MENU_OPEN: &str = "open-launcher";
const MENU_QUIT: &str = "quit";
```
菜单事件 `match event.id.as_ref()` 穷尽 + `_ => {}`;托盘事件只取 `TrayIconEvent::Click { button, button_state: Up, .. }`。tooltip 用 `app.package_info().name.clone()`,不硬编码。

### 2.4 `commands/launcher.rs`

```rust
/// 当前生效的唤出快捷键(plugin 语法),供前端渲染键帽;目前恒为默认值,可配置后改读设置
#[tauri::command]
pub fn get_toggle_shortcut() -> &'static str { crate::launcher::DEFAULT_TOGGLE_SHORTCUT }

/// 隐藏启动器(主页 Esc)。不可失败:窗口 API 失败已在领域层记日志。
#[tauri::command]
pub fn hide_launcher<R: Runtime>(app: AppHandle<R>) { crate::launcher::hide(&app); }
```

### 2.5 `lib.rs`

```rust
mod commands; mod error; mod launcher; mod tray;

.setup(|app| {
    #[cfg(desktop)]
    setup_desktop(app)?;
    Ok(())
})
.invoke_handler(tauri::generate_handler![commands::greet::greet, commands::launcher::hide_launcher])

#[cfg(desktop)]
fn setup_desktop(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 插件必须先注册,否则 global_shortcut() 取不到
    app.handle().plugin(tauri_plugin_global_shortcut::Builder::new().build())?;
    // 2. 快捷键被占用不是致命错误:托盘仍可打开面板
    if let Err(e) = app.global_shortcut().on_shortcut("alt+enter", |app, _s, ev| {
        if ev.state == ShortcutState::Pressed { launcher::toggle(app); }
    }) { log::error!("注册全局快捷键 Alt+Enter 失败: {e}"); }
    // 3. 窗口钩子与事件
    if let Some(window) = app.get_webview_window(launcher::MAIN_WINDOW) {
        #[cfg(windows)]
        launcher::install_platform_hooks(&window);
        let handle = app.handle().clone();
        window.on_window_event(move |event| match event {
            WindowEvent::Focused(false) => launcher::hide_on_blur(&handle),
            WindowEvent::CloseRequested { api, .. } => { api.prevent_close(); launcher::hide(&handle); }
            _ => {}
        });
    }
    // 4. 托盘要在 init_hidden 之前建好,cursor_on_tray 才能工作
    tray::setup(app.handle())?;
    // 5. 最后隐藏:此时前端未加载,不 emit
    launcher::init_hidden(app.handle());
    Ok(())
}
```
超过十来行,按规范抽成独立函数;`lib.rs` 只增长 `mod` / `plugin` / `generate_handler` 行 + 这一个 setup 函数。

## 3. 前端契约

### 3.1 `src/lib/api.ts`(唯一 invoke 入口)

```ts
/** 隐藏启动器窗口。非 Tauri 运行时 no-op;后端不可失败,失败即 IPC 层异常,调用方吞掉并 console.error。*/
export function hideLauncher(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve();
  return invoke<void>("hide_launcher");
}
```

`getToggleShortcut(): Promise<string>`:非 Tauri 回退 `"alt+enter"`(前端唯一字面量出处),否则 `invoke<string>("get_toggle_shortcut")`;`LauncherPanel` `onMounted` 读取后经 `parseShortcut()`(`lib/launcher/keyLabels.ts`,`"alt+enter" → ["Alt","Enter"]`)传给 `HomeSearchBar` / `ToolSearchBar` 的 `shortcutKeys` prop 渲染键帽。

### 3.2 `src/lib/window.ts`

删除 `hideLauncher`,文件头注释更新:「只负责尺寸同步;显示 / 隐藏由后端命令控制,见 lib/api.ts」。

### 3.3 `src/lib/events.ts`(新)

```ts
export const EVENTS = {
  LAUNCHER_OPENED: "launcher://open",
  LAUNCHER_CLOSED: "launcher://close",
} as const;
/** 事件名 → payload;与 src-tauri/src/launcher.rs 的常量一一对应;两个事件无 payload */
export interface EventPayloads {
  [EVENTS.LAUNCHER_OPENED]: null;
  [EVENTS.LAUNCHER_CLOSED]: null;
}
```

### 3.4 `src/composables/useTauriEvent.ts`(新,唯一 import `@tauri-apps/api/event`)

按 `composable-guidelines.md` §2 示例实现:非 Tauri 直接 return;`listen(...).then(fn => disposed ? fn() : unlisten = fn)`;`onUnmounted` 置 `disposed` 并 `unlisten?.()`。

### 3.5 `SearchInput.focus(options?)`

```ts
function focus(options: { selectAll?: boolean } = {}): void {
  el.focus();
  if (options.selectAll) el.select(); else el.setSelectionRange(end, end);
}
```
`HomeSearchBar` / `ToolSearchBar` 的 `defineExpose({ focus })` 透传参数。`LauncherPanel`:
```ts
useTauriEvent(EVENTS.LAUNCHER_OPENED, () => searchBar.value?.focus({ selectAll: true }));
```
`onEscape` 主页分支:`void hideLauncher().catch((e) => console.error("隐藏启动器失败:", e))`,import 改自 `@/lib/api`。

### 3.6 `index.css`

`body` 块删除 `background-color`,保留 `color` / `font-family` / 字体平滑 / overscroll;注释改为:「透明启动器窗口:文档底必须透明,面板圆角外才不会露底;表面色由面板根 `bg-card` 承担。`color-scheme` 仍在 html(见上方注释)」。`::selection` 等不动。

## 4. 与参考的差异

| 点 | 参考 | 本仓库 | 原因 |
|---|---|---|---|
| 模块位置 | `services/launcher_window.rs` + `services/tray.rs` + `platform/win_window.rs` | `launcher.rs` + `launcher/windows.rs` + `tray.rs` | 规范三段式:领域模块直接放 `src/`,平台实现放 `<domain>/<os>.rs` |
| 事件名 | `launcher-open` / `launcher-close` | `launcher://open` / `launcher://close` | 规范 `domain://action` |
| `TRAY_ID` 位置 | `tray.rs` | `launcher.rs` | 避免 tray ↔ launcher 双向依赖 |
| 关闭请求 | 未处理 | `prevent_close` + hide | D3 |
| 托盘右键 | 弹菜单前 hide | 不动面板 | 用户要求(D7b) |
| 唤出键 | 字面量散落 | 单一常量 + 命令下发前端 | 预留可配置(R6.7) |
| 搜索框焦点环 | — | 不画(显式例外) | D7a |
| `color-scheme` 位置 | 面板根 `scheme-light-dark` | 保留在 html | 规范 §3(Lightning CSS polyfill) |
| 快捷键注册失败 | `?` 让 setup 失败 | `log::error!` 继续 | 被占用不应让应用起不来 |
| windows crate | 0.62 全 feature | 0.61 三 feature | 与 tauri 间接依赖同版本;最小 feature |
| 前端 hide | `invoke` 在 `lib/window.ts` | `invoke` 在 `lib/api.ts` | 规范 invoke 唯一入口 |

## 5. 兼容 / 迁移

- 前端上个任务的 `hideLauncher`(直接 `hide()`)被替换;`core:window:allow-hide` 权限移除。
- `body` 背景移除后,浏览器预览页面底变为浏览器默认白;深色系统下浅色面板外是白底——这是预览态可接受的(真实窗口透明)。若觉得刺眼可在 `App.vue` 的 `<main>` 上用 `bg-background`?——**不做**:那会让透明窗口再次露底。预览态接受。
- Rust 侧新增 `mod launcher; mod tray;`,`greet` 保留。

## 6. 回滚

- 修改的既有文件:`Cargo.toml` / `Cargo.lock` / `tauri.conf.json5` / `capabilities/default.json` / `lib.rs` / `commands.rs` / `src/lib/window.ts` / `src/lib/api.ts` / `src/index.css` / `SearchInput.vue` / `HomeSearchBar.vue` / `ToolSearchBar.vue` / `LauncherPanel.vue`;新增 `launcher.rs` / `launcher/windows.rs` / `tray.rs` / `commands/launcher.rs` / `src/lib/events.ts` / `src/composables/useTauriEvent.ts`。`git checkout` 前者 + 删除后者即回滚。
