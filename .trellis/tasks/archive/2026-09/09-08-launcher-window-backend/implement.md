# 执行计划:启动器窗口管理(Rust 侧)

> 设计见 `design.md`。后端在 `src-tauri/` 下验证;首次跑 cargo 门禁前 `mkdir -p dist`(仓库根)或先 `bun run build`。

## 0. 前置

- [ ] 读 `.trellis/spec/backend/index.md` 开发前检查清单;spec 清单见 `implement.jsonl`。
- [ ] `Cargo.toml`:`tauri` features 加 `tray-icon`;加 `tauri-plugin-global-shortcut = "2"`(注释:Alt+Enter 唤出面板);加 `[target.'cfg(windows)'.dependencies] windows = { version = "0.61", features = ["Win32_Foundation", "Win32_UI_Shell", "Win32_UI_WindowsAndMessaging"] }`(注释:仅用于拦截无边框窗口的 Alt 系统菜单;版本对齐 tauri 间接依赖)。`cargo fetch` 确认锁文件更新且 `windows` 仍只有 0.61 一个版本(`grep -c 'name = "windows"$' Cargo.lock`)。

## 1. 领域层(先测再写)

- [ ] `src/launcher.rs`:`//!` 模块文档;常量 `MAIN_WINDOW` / `TRAY_ID` / `LAUNCHER_OPENED` / `LAUNCHER_CLOSED`;`WorkArea` + `anchor_position` 纯函数与 `#[cfg(test)]` 三个用例(居中 / 第二显示器偏移 / 窄工作区贴左缘);`show` / `hide` / `toggle` / `hide_on_blur` / `toggle_from_tray` / `init_hidden` / 私有 `main_window` / `hide_window` / `position_anchored` / `cursor_on_tray` / `rect_contains`。全部 `log::warn!` 吞错,不 `unwrap`。
- [ ] `src/launcher/windows.rs`:`#[cfg(windows)]` 由父模块门控;`suppress_alt_sysmenu(hwnd: isize)` + `subclass_proc`;`launcher.rs` 加 `#[cfg(windows)] mod windows;` 与 `#[cfg(windows)] pub fn install_platform_hooks`。
- 验证:`cd src-tauri && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test`。

## 2. 托盘 / 命令 / 装配

- [ ] `src/tray.rs`:`setup(app) -> tauri::Result<()>`,菜单 + 事件按 design §2.3。
- [ ] `src/commands/launcher.rs` + `commands.rs` 加 `pub mod launcher;`。
- [ ] `lib.rs`:`mod launcher; mod tray;`;`#[cfg(desktop)] fn setup_desktop`(design §2.5,含 5 步顺序注释);`setup` 调用;`generate_handler!` 加 `commands::launcher::hide_launcher`。
- [ ] `tauri.conf.json5` 主窗口 11 个属性 + 中文注释;`capabilities/default.json` 移除 `core:window:allow-hide`,更新 description。
- 验证:`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`。

## 3. 前端对接

- [ ] `src/lib/api.ts` 加 `hideLauncher()`;`src/lib/window.ts` 删 `hideLauncher` 并改头注释。
- [ ] `src/lib/events.ts`(新)、`src/composables/useTauriEvent.ts`(新)。
- [ ] `SearchInput.vue` `focus(options?)`;`HomeSearchBar.vue` / `ToolSearchBar.vue` 透传;`LauncherPanel.vue`:`hideLauncher` 改从 `@/lib/api` import,`useTauriEvent(EVENTS.LAUNCHER_OPENED, ...)`。
- [ ] `src/index.css` `body` 去 `background-color` + 改注释。
- 验证:`bun run format && bun run format:check && bun run lint && bun run test && bun run build`;grep:
  ```bash
  grep -rn "@tauri-apps/api" src --include=*.vue --include=*.ts | grep -v "src/lib/api.ts\|src/lib/window.ts\|src/composables/useTauriEvent.ts"
  ```
  应无输出。

## 4. 手工验收(`bun run tauri dev`)

- [ ] AC1 启动隐藏 + 托盘;Alt+Enter 弹出 / 收起;位置正确;搜索框聚焦。
- [ ] AC2 单按 Alt 不弹系统菜单。
- [ ] AC3 点外面收起;收起后原区域可点击桌面。
- [ ] AC4 托盘左键 / 右键菜单 / 退出。
- [ ] AC5 主页 Esc 收起;工具页 Esc 返回。
- [ ] AC6 Alt+F4 只收起。
- [ ] AC7 唤出时旧搜索词全选。
- [ ] AC8 圆角外透明;深浅色;高度同步。
- [ ] AC10 `bun run dev` 浏览器预览无报错。

## 5. 收尾(Phase 3)

- [ ] 3.3 spec 回写:`backend/directory-structure.md` 当前布局加 `launcher.rs` / `launcher/windows.rs` / `tray.rs` / `commands/launcher.rs`;`backend/state-events-async.md` 「目前没有事件」改为列出 `launcher://open|close`;`backend/config-and-permissions.md` 加窗口配置现状、`tray-icon` / global-shortcut / windows crate 依赖说明、`[target.'cfg(windows)']` 实例;`frontend/styling-guidelines.md` §3/§5 改「body 不设底色(透明窗口),表面色由面板根承担」;`frontend/ipc-guidelines.md` §5 事件「尚未使用」改为已用 + 指向 `useTauriEvent`;`frontend/directory-structure.md` 加 `lib/events.ts` / `composables/useTauriEvent.ts`;`guides/ipc-contract.md` 若有「尚无事件」措辞同步。
- [ ] 3.4 提交:`feat(backend): 启动器窗口管理(透明置顶/全局快捷键/托盘/失焦隐藏)` + `docs(spec): ...`。

## 风险文件 / 回滚点

- 步骤 1 结束(纯 Rust 领域层 + 测试,未接线)是第一个安全点;步骤 2 结束(后端完整,前端未改)第二个安全点——此时前端仍直接 `hide()`,因权限已移除会失败,所以步骤 2 与 3 应在同一提交。
- 回滚见 design §6。

## 子代理分工建议(Phase 2)

- 步骤 0~2(Rust)一个 `trellis-implement`;步骤 3(前端)第二个;`trellis-check` 全量检查后由主代理跑 `tauri dev` 手工验收(AC1~AC8 需真实窗口,无法自动化)。
