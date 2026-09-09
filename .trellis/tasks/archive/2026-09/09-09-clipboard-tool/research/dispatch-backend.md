# 后端实现指令(implement.md 步骤 2 ~ 5)

> 本文件是主会话派发给后端实现子代理的指令;契约以 `design.md` 为准,本文件只是把步骤拆细。

## 边界

- 只允许改动:`src-tauri/**`(`Cargo.toml`、`src/`、新建 `migrations/`)与 `src-tauri/tauri.conf.json5`。
- **不要碰 `src/`(前端)**——另一个子代理正在并行实现前端,两边按 design.md §3.1 / §3.3 / §3.4 各自独立镜像契约。
- 步骤 1(删 demo)已完成并提交,不用管。

## 必读(编码前)

1. `.trellis/tasks/09-09-clipboard-tool/prd.md`、`design.md`、`implement.md`
2. 规范:`.trellis/spec/backend/index.md` 及其下 `directory-structure.md`、`state-events-async.md`、`command-guidelines.md`、`error-handling.md`、`config-and-permissions.md`、`quality-guidelines.md`;`.trellis/spec/guides/ipc-contract.md`
3. 现有代码:`src-tauri/src/lib.rs`、`launcher.rs`、`launcher/windows.rs`、`error.rs`、`commands.rs`、`commands/launcher.rs`、`tray.rs`、`tauri.conf.json5`

## 步骤 2:依赖与错误

- `Cargo.toml`:新增 `arboard = "3.6"`、`sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "migrate"] }`、`image = { version = "0.25", default-features = false, features = ["png"] }`、`blake3 = "1"`;`windows` 追加 features `Win32_System_DataExchange`、`Win32_System_LibraryLoader`、`Win32_UI_Input_KeyboardAndMouse`(其他需要的 feature 按编译报错补)。`[dev-dependencies]` 加 `tokio = { version = "1", features = ["macros", "rt"] }`。每项上方写中文注释(用途 + 选型原因,见 design §2)。跑 `cargo tree -i windows` 确认仍只有一份 `windows` crate(`windows-sys` 多版本可接受)。
- `error.rs`:新增 `Database(#[from] sqlx::Error)`、`Clipboard(String)`、`Unsupported(String)` 变体 + 文案测试,遵循 error-handling.md。

## 步骤 3:数据层

- `src-tauri/migrations/0001_clipboard.sql`(design §3.2 原文 schema)。
- `src/clipboard.rs`:常量(`MAX_ITEMS=500`、`MAX_TEXT_BYTES=1MiB`、`MAX_IMAGE_BYTES=20MiB`、`THUMB_MAX_EDGE=256`、`PREVIEW_CHARS=300`、`CLIPBOARD_CHANGED = "clipboard://changed"`)、`ClipboardKind`、`ClipboardItem`(tagged enum,`#[serde(rename_all = "camelCase", tag = "kind")]`,变体 `Text{id,favorite,copied_at,size,preview,char_count,truncated}` / `Image{id,favorite,copied_at,size,image_path,thumb_path,width,height}` / `Files{id,favorite,copied_at,files: Vec<ClipboardFile>}`)、`ClipboardFile{path,name,exists}`、`ListQuery{kind,favorite_only,query,before,limit}`(Deserialize + Default + camelCase)、`ListCursor{copied_at,id}`、`Captured`(内部枚举)、`ClipboardStore{pool, images_dir}`、`searchable_text(&Captured) -> Option<String>`(text→全文;files→文件名 `\n` 拼接;image→None,单测锁定)、`record(app, store, captured)`(写图片文件 + upsert + trim + 删被淘汰图片 + emit `CLIPBOARD_CHANGED`)。`mod store; mod backend; #[cfg(windows)] mod windows;`。
- `src/clipboard/store.rs`:`open(db_path) -> Result<SqlitePool>`(`create_if_missing` + `sqlx::migrate!("./migrations")`)、`upsert`(`INSERT ... ON CONFLICT(hash) DO UPDATE SET copied_at = excluded.copied_at`)、`trim`(非 favorite 超 `MAX_ITEMS` 淘汰,返回被删图片文件名)、`list(&ListQuery) -> Vec<ClipboardItem>`(kind × favorite_only × query 三条件 AND 拼接 + `before` 游标 `(copied_at, id) < (?, ?)`;`text LIKE ? ESCAPE '\'` 转义 `%`/`_`/`\`;`ORDER BY copied_at DESC, id DESC LIMIT ?`;text 条目 preview 按 `PREVIEW_CHARS` 按 char 截断 + trim,`truncated = char_count > PREVIEW_CHARS`;files 条目逐路径 `Path::file_name()` 推导 name、`Path::exists()` 填 exists;image 条目 image_path/thumb_path 为绝对路径 `images_dir.join(...)`)、`get_captured(id)`、`get_text(id)`(不存在→`InvalidInput("记录不存在")`,非 text→`InvalidInput("该条目不是文本")`)、`delete(id)`(返回可能需删的图片文件名)、`set_favorite(id, bool)`(不动 copied_at)、`clear()`(只删 favorite=0,返回图片文件名列表)。只用运行时 `sqlx::query*`,**不用 `query!` 宏**。
- 单测(`sqlite::memory:` + `#[tokio::test]`),必须覆盖 prd AC4 全部条目:upsert 去重上浮、trim 保留 favorite、三维度叠加过滤且排序不受 favorite 影响、同 copied_at 按 id 倒序、游标分页中途插入新条目后无重复无漏项、文件搜索只命中文件名不命中目录名、图片 text NULL 任何关键字不命中、`%`/`_`/`\` 转义、set_favorite 不改 copied_at、clear 保留 favorite、get_text 全文 + 非 text 报 InvalidInput、preview 截断 truncated 判定。

## 步骤 4:剪贴板读写与 Windows 平台层

- `src/clipboard/backend.rs`(跨平台,无 cfg,arboard):`read_snapshot() -> Option<Captured>`(优先级 file_list > image > text;`Clipboard::new()` 失败 / 被占用重试 3×50ms;文件列表空→跳过;图片 `w*h*4 > MAX_IMAGE_BYTES`→跳过,像素 hash = blake3(w,h,rgba8),用 `image` 编码 PNG 原图 + 缩略图(最长边 `THUMB_MAX_EDGE`);文本 trim 空 / 超 `MAX_TEXT_BYTES`→跳过)、`write(&Captured) -> Result<(), AppError>`(image:PNG 解码回 RGBA8 → `set().image(ImageData)`;files → `set().file_list()`)。`arboard::Clipboard` 非 Send,全部在同步块内用完。纯函数(阈值判定、哈希、缩略图尺寸计算)写单测。
- `src/clipboard/windows.rs`:`pub struct ClipboardWatcher { hwnd: AtomicIsize }`;`run_monitor(app: AppHandle)`——`RegisterClassW` + `CreateWindowExW(HWND_MESSAGE)`(窗口过程 `DefWindowProcW`)+ `AddClipboardFormatListener` + `GetMessageW` 循环;`WM_CLIPBOARDUPDATE` 时比对 `GetClipboardSequenceNumber` 去重后调 `backend::read_snapshot`,得到 Captured 后 `tauri::async_runtime::spawn(record(...))`;`WM_CLOSE` 时 `RemoveClipboardFormatListener` + `DestroyWindow` + 退出循环。`stop_monitor(hwnd)`(`PostMessageW WM_CLOSE`)。`send_paste()`(`SendInput` VK_CONTROL↓ V↓ V↑ VK_CONTROL↑)。每处 `unsafe` 写「安全:」注释。
- `src/launcher.rs`:`#[cfg(windows)] pub struct PreviousForeground(AtomicIsize)`(app.manage);`show()` 在 `set_focus()` 之前记录 `GetForegroundWindow()`,仅当不是本窗口时记录;`pub fn previous_foreground(app) -> Option<isize>`。`src/launcher/windows.rs`:`current_foreground() -> isize`、`activate(hwnd: isize) -> bool`(IsWindow + SetForegroundWindow)。**改完 launcher.rs 要保证唤出 / 收起逻辑不回归**,改动最小化。
- `src/clipboard.rs`:`pub async fn paste(app, store, id)` 编排(design §4.2):get_captured → spawn_blocking(backend::write) → 取 PreviousForeground 并 activate → `launcher::hide(app)` → 若激活成功 spawn_blocking{sleep 50ms; send_paste()}(失败只 warn)。非 Windows:`paste` 返回 `AppError::Unsupported("当前平台暂不支持剪贴板粘贴")`。

## 步骤 5:命令与装配

- `src/commands/clipboard.rs`:6 个 `async` 命令 `list_clipboard_items(query: ListQuery)`(limit 校验 1..=200,否则 InvalidInput)、`get_clipboard_text(id)`、`paste_clipboard_item(id)`、`delete_clipboard_item(id)`、`set_clipboard_item_favorite(id, favorite)`、`clear_clipboard_history()`;全部 `Result<T, AppError>`,`State<'_, ClipboardStore>`。**命令层不 emit 事件**。`commands.rs` 加 `pub mod clipboard;`;`lib.rs` `generate_handler!` 追加 6 个。
- `lib.rs`:`setup_desktop` 内新增 `setup_clipboard(app)`(`app.path().app_local_data_dir()?.join("clipboard")` → `create_dir_all` 与 `images/`、`block_on(store::open)`、`app.manage(ClipboardStore)`;`#[cfg(windows)]` `app.manage(ClipboardWatcher)` + `spawn_blocking(run_monitor)`);顺序:插件 → 快捷键 → clipboard → 平台钩子 → 托盘 → init_hidden。**绝不用 `app_data_dir()`**。Builder 改为 `.build(tauri::generate_context!())?` + `.run(|app, event| { if let RunEvent::Exit = event { #[cfg(windows)] stop_monitor } })`。
- `tauri.conf.json5`:`app.security.assetProtocol: { enable: true, scope: ["$APPLOCALDATA/clipboard/images/**"] }`,每项中文注释;CSP 保持 null,注释里写明收紧时需加 `img-src 'self' asset: http://asset.localhost`。

## 完成标准(必须全部通过后再汇报)

```
cd src-tauri && cargo fmt && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && cargo build
```

汇报内容:新增/修改文件清单、`cargo test` 通过数、`cargo tree -i windows` 结果、任何偏离 design.md 的地方及理由、需要手测确认的风险项(arboard 能否读到 Win+Shift+S 截图与 Chrome 复制图片)。
