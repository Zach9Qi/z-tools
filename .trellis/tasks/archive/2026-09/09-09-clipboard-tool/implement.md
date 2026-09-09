# 执行计划：剪切板工具（Windows）

> 按顺序执行；每步的验证命令在括号内。所有步骤在 Windows 上开发验证。规范见 `implement.jsonl` 所列文件。

## 0. 前置

- [ ] 读 `.trellis/spec/backend/index.md`、`frontend/index.md` 与本任务 `design.md`。
- [ ] `git status` 干净；基线 `cd src-tauri && cargo test` + `bun run test` 通过。

## 1. 清理 demo 占位（可独立提交）

- [ ] 删除 `src/tools/demo/` 整目录。
- [ ] `src/tools/registry.ts`：移除 demo import，`modules` 暂为空数组（下一步填入 clipboard）。
- [ ] `src/tools/icons.ts`：删除仅 demo 使用且 clipboard 不用的图标登记（保留 `puzzle` 回退与后续要用的）。
- [ ] 验证：`bun run lint && bun run test && bun run build`（网格为空是预期中间态）。

## 2. 后端：依赖与错误

- [ ] `Cargo.toml`：新增 `arboard`、`sqlx`、`image`、`blake3`（通用）；`windows` 追加 `Win32_System_DataExchange`、`Win32_System_LibraryLoader`、`Win32_UI_Input_KeyboardAndMouse`（`cfg(windows)`）。每项上方中文注释（用途 + 选型原因，见 design §2）。`cargo tree -i windows` 确认仍只有一份 `windows` crate。
- [ ] `error.rs`：新增 `Database` / `Clipboard` / `Unsupported` 变体 + 文案测试。
- [ ] 验证：`cargo check`（首次编译 libsqlite3-sys 较慢）。

## 3. 后端：数据层

- [ ] `src-tauri/migrations/0001_clipboard.sql`（design §3.2）。
- [ ] `src/clipboard.rs`：常量、`ClipboardKind`、`ClipboardItem`、`ListQuery`、`Captured`、`ClipboardStore`、`CLIPBOARD_CHANGED`、`record()`；`mod store; #[cfg(windows)] mod windows;`。
- [ ] `src/clipboard/store.rs`：`open(db_path) -> SqlitePool`（`create_if_missing` + `migrate!`）、`upsert`、`trim`、`list -> Vec<ClipboardItem>`（kind × favorite_only × query 叠加 + `before` 游标，`ORDER BY copied_at DESC, id DESC LIMIT ?`）、`get_captured`、`get_text(id) -> String`（非 text 类型 / 不存在 → `InvalidInput`）、`delete`、`set_favorite`、`clear`。搜索转义 `%`/`_`。`list` 中 text 条目填 `truncated = char_count > PREVIEW_CHARS`，files 条目逐路径填 `ClipboardFile { path, name, exists }`。
- [ ] 单测（`#[cfg(test)]`，`sqlite::memory:` + `#[tokio::test]` 需 `tokio` dev-dependency `features=["macros","rt"]`）：upsert 去重上浮、trim 保留 favorite、list 三维度叠加过滤且排序不受 favorite 影响、同 copied_at 两条按 id 倒序、游标分页在两页之间插入新条目后第二页无重复、文件搜索命中文件名但不命中所在目录名（文件名入库时写进 `text` 列）、图片 `text` 为 NULL 时任何关键字都不命中、`%`/`_`/`\\` 转义、set_favorite 不改 copied_at、clear 保留 favorite、get_text 返回未截断全文且对 image/files 条目报 InvalidInput、preview 截断时 truncated=true 且短文本为 false。
- [ ] 验证：`cargo test`。

## 4. 后端：剪贴板读写与 Windows 平台层

- [ ] `src/clipboard/backend.rs`（跨平台，arboard）：`read_snapshot() -> Option<Captured>`（优先级 file_list > image > text，被占用重试 3×50ms）、`write(&Captured) -> Result<(), AppError>`；图片 RGBA ↔ PNG 用 `image`。纯函数部分（阈值判定、哈希）单测。
- [ ] `src/clipboard/windows.rs`：`run_monitor(app, watcher_state)`（`CreateWindowExW(HWND_MESSAGE)` + `AddClipboardFormatListener` + `GetMessageW` 循环，`WM_CLIPBOARDUPDATE` 时比对 `GetClipboardSequenceNumber` 后调 `backend::read_snapshot`，`WM_CLOSE` 时清理退出）、`stop_monitor(hwnd)`（`PostMessageW WM_CLOSE`）、`send_paste()`（SendInput Ctrl+V）。每处 `unsafe` 写「安全:」注释。
- [ ] `src/launcher.rs`：`#[cfg(windows)] pub struct PreviousForeground(AtomicIsize)`；`show()` 在 `set_focus` 前记录；`pub fn previous_foreground(app) -> Option<HWND>`。`src/launcher/windows.rs`：`current_foreground()`、`activate(hwnd) -> bool`（IsWindow + SetForegroundWindow）。
- [ ] `src/clipboard.rs`：`paste(app, id)` 编排（design §4.2）。
- [ ] 验证：`cargo clippy --all-targets -- -D warnings`。

## 5. 后端：命令与装配

- [ ] `src/commands/clipboard.rs` 6 个命令（含 `get_clipboard_text`）（命令层不 emit 事件，`clipboard://changed` 只由 `record()` 发）；`commands.rs` 加 `pub mod clipboard;`；`lib.rs` `generate_handler!` 追加。
- [ ] `lib.rs`：`setup_desktop` 内新增 `setup_clipboard(app)`（`app.path().app_local_data_dir()` → 建目录、连接、manage、`#[cfg(windows)]` 启动 watcher；**不用 `app_data_dir()`**）；Builder 改 `.build()?.run(|app, event| if let RunEvent::Exit … 停止 watcher)`。
- [ ] `tauri.conf.json5`：`assetProtocol` 配置（scope `$APPLOCALDATA/clipboard/images/**`）+ 注释。
- [ ] 验证：`cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`；`bun run tauri dev` 手测：复制文本/截图/文件，日志出现录入；`%LOCALAPPDATA%\com.example.tauri-vue-starter\clipboard\` 下有 db 与图片，且 `%APPDATA%\com.example.tauri-vue-starter\`（Roaming）未被创建。特别验证 Chrome 右键「复制图片」与 Win+Shift+S 截图都能被 arboard 读到（design §7 风险项），读不到则在 `windows.rs` 补读 PNG 注册格式。

## 6. 前端：契约层

- [ ] `src/types/clipboard.ts` 镜像（文件头注明 Rust 路径）。
- [ ] `src/lib/api/clipboard.ts`：6 个封装 + `toAssetUrl`；非 Tauri 假数据（3 类各 1-2 条，标「(浏览器预览)」；含一条 `truncated: true` 的多行长文本、一条含 `exists: false` 路径的多文件条目，`getClipboardText` 假数据返回一段 20 行文本，便于预览展开态）。
- [ ] `src/lib/events.ts`：`CLIPBOARD_CHANGED`。
- [ ] 验证：`bun run lint`；`grep -rn "@tauri-apps/api/core" src` 仅 `lib/api*`。

## 7. 前端：工具模块

- [ ] `src/tools/clipboard/lib/format.ts` + `format.test.ts`（相对时间、字节、文件摘要 `summarizeFiles(files)`；文件名由后端给出，前端不解析路径；`isExpandable(item)`：text 看 `truncated || 含换行`、image 恒 true、files 看 `length > 1`，各分支进单测）。
- [ ] `composables/useClipboardHistory.ts`（状态 + 防抖 + 事件订阅 + loadMore；remove / toggleFavorite / clear 成功后**本地改 items**，不重拉、**不碰 hasMore**；`expandedId` + `toggleExpanded(id)`，`refresh()` / 删除展开项 / clear 后展开项不在时置 null，与 `selectedIndex` 独立；`hasMore` 只在响应到达时写，判据唯一：`page.length < PAGE_SIZE` → false，否则 true，游标 `loadMore` 时从 `items.at(-1)` 现取；底部哨兵 `IntersectionObserver` + 键盘 `↓` 到末条触发 `loadMore`；仅监听器事件与筛选变化 `refresh`，递增序号丢弃过期响应）。
- [ ] `components/ClipboardTabs.vue`、`components/ClipboardItemRow.vue`（行内动作只有两个：常显星标按钮（favorite 实心 / 否则空心，`@click.stop` 切换收藏）与右侧 chevron（仅 `isExpandable` 时渲染，否则等宽占位）；两者均 `@mousedown.prevent` 不夺焦点；**无**悬停收藏 / 删除按钮）、`components/ClipboardItemDetail.vue`（text 自行 `getClipboardText` + loading / error；image 原图；files 逐行路径，`!exists` 标灰 + 「不存在」；`max-h-72 overflow-auto`；点击不冒泡到行的粘贴）。
- [ ] `ClipboardPage.vue`（单根、keymap 6 组：↑↓ / Enter / Delete / Ctrl+P / Ctrl+F / Ctrl+←→、空态/加载/错误、清空确认）。
- [ ] `index.ts` 模块定义；`icons.ts` 登记图标；`registry.ts` `modules = [clipboardTool]`。
- [ ] 验证：`bun run format && bun run format:check && bun run lint && bun run test && bun run build`；`bun run dev` 浏览器预览显示假数据。

## 8. 集成手测（Windows，`bun run tauri dev`）

- [ ] 记事本复制文本 → 唤出 → 剪贴板工具 → 列表首项为该文本；Enter → 面板收起、记事本光标处出现粘贴。
- [ ] Win+Shift+S 截图 → 列表出现缩略图与尺寸；Enter 粘贴到画图。
- [ ] 资源管理器复制 2 个文件 → 列表显示「xxx 等 2 项」；Enter 粘贴到另一文件夹。
- [ ] 重复复制同一文本 → 不新增，上浮到首位。
- [ ] Ctrl+P 收藏 → 位置不变、实心星标；鼠标点行内空心星标 → 变实心且面板不收起，再点变回；Ctrl+F / 点「收藏」按钮 → 只剩收藏，再切 Tab / 输关键字叠加生效；清空历史 → 收藏保留。
- [ ] 搜索关键字过滤文本 / 文件名；Ctrl+←/→ 切 Tab。
- [ ] 复制 10+ 行文本 → 行只显两行 + chevron；点 chevron 展开全文（换行保留、可滚动）与字符数，再点收起；点另一条前一条自动收起；展开图片条目见原图 + 尺寸；删掉一个已复制文件后展开该条目，仅该路径标灰「不存在」；展开期间 ↑/↓ 正常移动选中，切 Tab 后无展开项；单行短文本 / 单文件无 chevron。
- [ ] 重启应用 → 历史保留。
- [ ] 关掉记事本后 Enter → 只复制收起，无报错。

## 9. 收尾

- [ ] 全量门禁：`bun run format:check && bun run lint && bun run test && bun run build && (cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)`。
- [ ] 更新规范：`backend/directory-structure.md`（新模块）、`backend/state-events-async.md`（首个托管状态 / 事件 / 后台任务实例）、`backend/config-and-permissions.md`（assetProtocol、新依赖、**新增「落盘目录约定」一节：所有持久化统一 `app_local_data_dir()` / `$APPLOCALDATA`，禁用 `app_data_dir()` / `$APPDATA`，并列出当前目录下的内容清单 `EBWebView/`、`logs/`、`clipboard/`**）、`frontend/tool-module-guidelines.md`（demo 已删，示例改指 clipboard）、`frontend/ipc-guidelines.md`（`api/<domain>.ts` 首例、`convertFileSrc` 唯一入口）、`guides/ipc-contract.md`（tagged enum 现例）。
- [ ] 提交（中文 commit message），`task.py archive`。

## 风险文件 / 回滚点

- `lib.rs`（Builder 改 build+run）、`launcher.rs`（show 时序）——每次改后跑 `tauri dev` 确认唤出/收起不回归。
- 步骤 1 单独提交，便于其余步骤失败时保留清理成果。
