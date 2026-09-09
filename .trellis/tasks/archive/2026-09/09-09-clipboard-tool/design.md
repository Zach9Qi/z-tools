# 技术设计：剪切板工具（Windows）

> 对应 `prd.md`。本文件只写技术方案：边界、契约、数据流、取舍。执行清单见 `implement.md`。

## 1. 总体架构

```text
Windows 剪贴板 ──WM_CLIPBOARDUPDATE──▶ clipboard/windows.rs (自建消息窗口监听, spawn_blocking)
                                            │ 触发 clipboard/backend.rs::read_snapshot()（arboard，跨平台）
                                            │ 得到 Captured{Text|Image|Files}
                                            ▼
                                     clipboard.rs  record()  ──▶ clipboard/store.rs (sqlx SQLite)
                                            │                        + images/<hash>.png / .thumb.png
                                            │ emit clipboard://changed
                                            ▼
                                 前端 useTauriEvent → useClipboardHistory.refresh()
                                            │ listClipboardItems / getText / paste / delete / setFavorite
                                            ▼
                                 commands/clipboard.rs ──▶ clipboard.rs ──▶ store / windows(写回 + SendInput)
```

三段式分层沿用 `backend/directory-structure.md`：命令层薄、领域层纯业务、平台代码拆文件。

**跨平台分工原则**（用户目标是后续做全平台）：
- **读 / 写剪贴板**用 `arboard`（1Password 维护，主流跨平台库，三平台一致 API），放在平台无关的 `clipboard/backend.rs`。
- **监听变化**与**粘贴模拟**是 arboard 不提供的能力，按平台拆文件 `clipboard/{windows,macos,linux}.rs`，暴露统一签名 `run_monitor` / `stop_monitor` / `send_paste`。本版只实现 `windows.rs`；后续平台各补一份几十行的薄实现，不动数据层与命令层。

## 2. 依赖选型

| 依赖 | 版本 | 位置 | 理由 |
|---|---|---|---|
| `arboard` | `3.6`，默认 feature（`image-data`） | 通用 | 跨平台剪贴板**读写**：`get().text()/image()/file_list()`、`set().text()/image()/file_list()`（3.6.1 已含 file_list 读写，已核实 tag 源码）。1Password 维护、下载量 4400 万+，是 Rust 生态主流选择。Windows 侧内部依赖 `windows-sys`（仓库已有多版本共存）+ `clipboard-win`，**不引入第二份 `windows` crate**。不选 `clipboard-rs`：小众（下载量 1/50、个人维护）且依赖 `windows 0.59` 与仓库锁定的 0.61.3 冲突；不选官方 `tauri-plugin-clipboard-manager`：无文件列表、无监听 |
| `windows` | 现有 `0.61`，追加 feature `Win32_System_DataExchange`、`Win32_System_LibraryLoader`、`Win32_UI_Input_KeyboardAndMouse` | `[target.'cfg(windows)'.dependencies]` | 自建监听：`CreateWindowExW(HWND_MESSAGE)` + `AddClipboardFormatListener` + `GetMessageW` 循环收 `WM_CLIPBOARDUPDATE`（arboard 无监听能力，这是各平台必须自写的部分）；`SendInput` 模拟 Ctrl+V；`GetForegroundWindow` / `SetForegroundWindow` / `IsWindow` 已在 `Win32_UI_WindowsAndMessaging` |
| `sqlx` | `0.8`，`default-features=false`，features `["runtime-tokio", "sqlite", "migrate"]` | 通用 | 用户指定；SQLite bundled 由 `sqlite` feature 自带 libsqlite3-sys。**只用运行时 `sqlx::query*`，不用 `query!` 宏**，避免编译期依赖 DATABASE_URL。0.9.0 刚发布，暂用 0.8 稳定线 |
| `image` | `0.25`，`default-features=false`，features `["png"]` | 通用 | arboard 给出的是 RGBA8 像素，用 `image` 编码 PNG 原图与缩略图、粘贴时把 PNG 解码回 RGBA；与 arboard 内部依赖同一大版本，不重复编译 |
| `blake3` | `1` | 通用 | 内容哈希去重，需跨版本稳定（std `DefaultHasher` 不保证） |
| `serde_json` | 现有 | 通用 | `files` 列 JSON 编解码 |

`tokio` 已由 tauri 间接引入（Cargo.lock 1.53），不直接依赖。

## 3. 后端模块

```text
src-tauri/src/
├── clipboard.rs            # 领域层：常量、ClipboardKind、ClipboardItem DTO、Captured、ClipboardStore 装配、record()、
│                           # paste()、事件常量 CLIPBOARD_CHANGED；#[cfg(windows)] mod windows; mod store;
├── clipboard/
│   ├── store.rs            # sqlx 访问：init(pool, migrate)、upsert、list(kind × favorite_only × query)、delete、set_favorite、trim；纯数据，可单测(内存库)
│   ├── backend.rs          # 跨平台读写（arboard）：read_snapshot() -> Option<Captured>、write(&Captured)；无 cfg，三平台共用
│   └── windows.rs          # 平台专属：run_monitor()（消息窗口 + WM_CLIPBOARDUPDATE）、stop_monitor(hwnd)、send_paste()（SendInput）
├── commands/clipboard.rs   # 5 个命令（list / get_text / paste / delete / set_favorite）
├── launcher.rs             # +PreviousForeground 托管状态；show() 内 #[cfg(windows)] 记录前台 HWND
├── launcher/windows.rs     # +foreground_hwnd() / activate(hwnd)
└── migrations/0001_clipboard.sql  (src-tauri/migrations/，sqlx::migrate!() 编译期内嵌)
```

### 3.1 常量（`clipboard.rs`）

| 常量 | 值 | 说明 |
|---|---|---|
| `MAX_ITEMS` | 500 | 非收藏条目上限，超出按 `copied_at` 淘汰 |
| `MAX_TEXT_BYTES` | 1 MiB | 超过不记录 |
| `MAX_IMAGE_BYTES` | 20 MiB | PNG/DIB 原始字节超过不记录 |
| `THUMB_MAX_EDGE` | 256 | 缩略图最长边 |
| `PREVIEW_CHARS` | 300 | 列表文本预览截断长度（按 char） |
| `CLIPBOARD_CHANGED` | `"clipboard://changed"` | 无 payload 事件 |

### 3.2 数据模型（SQLite，`migrations/0001_clipboard.sql`）

```sql
CREATE TABLE clipboard_items (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  kind          TEXT    NOT NULL CHECK (kind IN ('text','image','files')),
  hash          TEXT    NOT NULL UNIQUE,   -- blake3 hex；text=blake3(utf8)；image=blake3(w,h,rgba8)；files=blake3(paths.join("\n"))
  text          TEXT,                      -- 可搜索 / 可展示的文本，含义随 kind：text=全文；files=各文件名（含扩展名）以 "\n" 拼接；image=NULL（无名，将来 OCR 文字填此列）
  image_file    TEXT,                      -- kind=image：images/ 下文件名 <hash>.png（缩略图 <hash>.thumb.png 同名推导）
  image_width   INTEGER,
  image_height  INTEGER,
  files         TEXT,                      -- kind=files：完整路径 JSON string[]，用于粘贴写回与 missing 检查，不参与搜索
  size          INTEGER NOT NULL,          -- text: utf8 字节；image: png 字节；files: 0
  favorite      INTEGER NOT NULL DEFAULT 0, -- 收藏标记：只是筛选维度 + 保护不被淘汰，**不影响排序**
  created_at    INTEGER NOT NULL,          -- unix ms 首次
  copied_at     INTEGER NOT NULL           -- unix ms 最近一次；主排序键，并列时由 id 兜底
);
-- 不建额外索引：表最大约 500 非收藏 + 收藏，千行量级全扫 + 排序是微秒级，且列表查询几乎都带 LIKE（必然全扫）。
-- 去重所需的索引由 hash UNIQUE 隐式提供。MAX_ITEMS 提到万级时再加 (copied_at DESC, id DESC)。
```

- 去重：`INSERT ... ON CONFLICT(hash) DO UPDATE SET copied_at = excluded.copied_at`（上浮，不新增）。
- 排序：一律 `ORDER BY copied_at DESC, id DESC`，收藏**不**置顶。`id` 是并列兜底：粘贴写回 + 监听器回捕、连续 Ctrl+C 都可能落在同一毫秒，SQLite 对并列行不保证顺序，没有兜底会导致刷新后选中项跳行。`id` 单调递增（`AUTOINCREMENT`），并列时后录入者在前。排序不建索引（见上方 SQL 注释）。
- 分页：**游标（keyset）**而非 `OFFSET`。追加页传上一页末条的 `(copied_at, id)`，SQL 为 `WHERE ... AND (copied_at, id) < (?, ?)`（SQLite ≥ 3.15 行值比较，bundled 版本满足）。理由：剪贴板列表在用户滚动期间会实时插入新条目，`OFFSET` 会让第二页重复返回第一页末尾的条目；游标分页不受影响。
- 淘汰：每次 upsert 后 `DELETE WHERE favorite = 0 AND id NOT IN (SELECT id ... WHERE favorite = 0 ORDER BY copied_at DESC LIMIT MAX_ITEMS)`；被删的 image 行先查出文件名再删文件（失败只 warn）。
- 存储位置：`app_local_data_dir()/clipboard/history.db`、`app_local_data_dir()/clipboard/images/`（Windows 上即 `%LOCALAPPDATA%\<identifier>\clipboard\`）。目录在 setup 内 `create_dir_all`。
- **项目级约定（本任务确立，回写规范）**：应用所有落盘数据统一放在 `app_local_data_dir()` 下，不用 `app_data_dir()`（Windows 上是 Roaming）。理由：用户希望删一个目录就能完整清理；WebView2 数据（`EBWebView/`）与 `tauri-plugin-log` 的 `LogDir`（`logs/`）在 Windows 上本就落在 `%LOCALAPPDATA%\<identifier>\`，剪贴板数据跟随同一目录。后续设置文件、缓存等也遵守此约定。
- 搜索列：**统一搜 `text`**，它的语义是「该条目可搜索 / 可展示的文本」：文本类型 = 全文；文件类型 = 各文件名（不含目录，否则文件夹名会误命中）；图片 = NULL（剪贴板图片无名，`W×H` 或 hash 都不是有意义的搜索对象；将来 OCR 结果填此列即可搜）。`text` 列的产出只在 `clipboard.rs` 的 `fn searchable_text(&Captured) -> Option<String>` 一处（text → 全文；files → 文件名 `\n` 拼接；image → None），规则不散落到 store / 命令层，由单测锁定；不用 CHECK 约束入库（那会把「图片 text 为 NULL」这条当前实现细节固化进 schema，将来 OCR 填列时还得迁移去掉）。Rust 读到 text 行 `text IS NULL` 时按空字串处理而非报错。一列一个 `LIKE`，不按 kind 分支；图片因 `text IS NULL` 天然不命中，「有搜索词时图片 Tab 为空 / 全部 Tab 排除图片」无需额外代码。文本类型的 `text` 本就是内容，不存在重复存储。`LIKE` 对 ASCII 默认大小写不敏感；非 ASCII 大小写折叠与 FTS5 留给后续。
- 图片哈希基于解码像素而非 PNG 字节：我们写回剪贴板后系统只留 DIB 时，重新编码的 PNG 字节会变，但像素不变 → 仍命中同一条。

### 3.3 领域类型（Rust ↔ TS 镜像 `src/types/clipboard.ts`）

```rust
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ClipboardKind { Text, Image, Files }

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ClipboardItem {
    /// `truncated`：preview 是否比全文短；前端据此决定是否显示展开 chevron（多行短文本也需要展开，故还要看 preview 是否含换行，见 §5）
    Text  { id: i64, favorite: bool, copied_at: i64, size: u64, preview: String, char_count: u64, truncated: bool },
    Image { id: i64, favorite: bool, copied_at: i64, size: u64, image_path: String, thumb_path: String, width: u32, height: u32 },
    /// 文件条目自带全部路径与逐个存在性，展开态不需要再请求后端
    Files { id: i64, favorite: bool, copied_at: i64, files: Vec<ClipboardFile> },
}

/// 文件条目中的一项；`name` 由 `path` 取 `file_name()` 推导，`exists` 在 list 时 `Path::exists` 逐个检查
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFile { pub path: String, pub name: String, pub exists: bool }

/// 列表筛选：三个维度可任意叠加（kind × favorite_only × query）
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    pub kind: Option<ClipboardKind>,
    pub favorite_only: bool,
    pub query: String,
    /// 游标：上一页最后一条的 (copied_at, id)；None = 从头拉
    pub before: Option<ListCursor>,
    pub limit: u32,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ListCursor { pub copied_at: i64, pub id: i64 }
```

- `image_path` / `thumb_path` 是**绝对路径**，前端经 `convertFileSrc()`（仅在 `src/lib/api/clipboard.ts` 内调用）转成 `asset://` URL。
- `preview`：`text` 列按 `PREVIEW_CHARS` 截断、首尾 trim；不折叠换行（前端 `line-clamp-2`）。`truncated = char_count > PREVIEW_CHARS`。全文**不**进列表 DTO（100 条 × 最大 1 MiB 不可接受），展开时用 `get_clipboard_text(id)` 按需拉。`ClipboardFile.name` 由 `files` 列的路径逐个取 `Path::file_name()` 推导，**不**反向拆 `text` 列（Linux 文件名可含换行，拆行会错；`text` 只服务搜索）。图片展开态直接用已有的 `image_path`（asset:// 原图），无新字段。
- `Captured` 为内部枚举（不序列化）：`Text(String)` / `Image { png: Vec<u8>, width, height, rgba_hash }` / `Files(Vec<String>)`。

### 3.4 命令（`commands/clipboard.rs`）

| Rust | TS (`src/lib/api/clipboard.ts`) | 说明 |
|---|---|---|
| `list_clipboard_items(query: ListQuery) -> Result<Vec<ClipboardItem>, AppError>` | `listClipboardItems(query)` | `limit` 校验 1..=200，超出 → `InvalidInput`。WHERE 由三个可选条件 AND 拼接：`kind = ?`（有 kind 时）、`favorite = 1`（`favorite_only` 时）、关键字（`query` 非空时 `text LIKE ? ESCAPE '\\'`，关键字两侧加 `%` 并转义 `%`/`_`/`\\`；图片 `text IS NULL` 自然不命中，不需要按 kind 特判）、游标 `(copied_at, id) < (?, ?)`（有 `before` 时）。`ORDER BY copied_at DESC, id DESC LIMIT ?` |
| `get_clipboard_text(id: i64) -> Result<String, AppError>` | `getClipboardText(id)` | 返回文本条目 `text` 列全文（不 trim，原样），供展开态显示；id 不存在 → `InvalidInput("记录不存在")`，kind 不是 text → `InvalidInput("该条目不是文本")`。不带副作用（不动 `copied_at`） |
| `paste_clipboard_item(id: i64) -> Result<(), AppError>` | `pasteClipboardItem(id)` | 见 §4.2 |
| `delete_clipboard_item(id: i64) -> Result<(), AppError>` | `deleteClipboardItem(id)` | 删行 + 删图片文件；id 不存在 → `InvalidInput("记录不存在")` |
| `set_clipboard_item_favorite(id: i64, favorite: bool) -> Result<(), AppError>` | `setClipboardItemFavorite(id, favorite)` | 只改标记，不动 `copied_at`（收藏不改变位置） |

- 全部 `async fn`（sqlx）。`State<'_, ClipboardStore>`。共 6 个命令。
- **命令层不 emit 事件**。delete / set_favorite 是前端自己发起的，单窗口应用里发命令的前端就是唯一消费者，命令返回 Ok 它就知道结果，直接改本地列表（见 §4.3）；再 emit 会导致本地已改 + 重拉双重刷新。`CLIPBOARD_CHANGED` **只由领域层 `record()` emit**，语义固定为「监听器录入了新内容或上浮了旧内容」——这是前端唯一无法自知的变化（含粘贴写回后的回捕）。
- 非 Windows 平台：`paste_clipboard_item` 返回 `AppError::Unsupported("当前平台暂不支持剪贴板粘贴")`；监听器在非 Windows 不启动（setup 中 `#[cfg(windows)]`），列表命令仍可用（空库）。工具在前端**始终注册**，非 Windows 显示空态即可（本版不做平台探测 UI）。

### 3.5 错误（`error.rs` 新增）

```rust
#[error("数据库错误: {0}")]  Database(#[from] sqlx::Error),
#[error("剪贴板错误: {0}")]  Clipboard(String),        // arboard::Error / Win32 监听窗口创建失败 / 图片编解码失败 → to_string
#[error("当前平台暂不支持: {0}")] Unsupported(String),
```

### 3.6 托管状态与生命周期

- `ClipboardStore { pool: SqlitePool, images_dir: PathBuf }` —— `app.manage`，在 `setup_desktop` 中新增 `setup_clipboard(app)`（`block_on` 仅此处：`app.path().app_local_data_dir()?` → 建目录、连接、`migrate!().run()`）。
- `ClipboardWatcher { hwnd: AtomicIsize }` —— `#[cfg(windows)]`，`app.manage`；`tauri::async_runtime::spawn_blocking(run_monitor)`，监听线程创建消息窗口后把 HWND 写入该状态；`RunEvent::Exit` 时 `stop_monitor(hwnd)`（`PostMessageW(hwnd, WM_CLOSE)` → 线程 `RemoveClipboardFormatListener` + `DestroyWindow` + 退出消息循环）。`lib.rs` 改为 `.build()` + `.run(|app, event| ...)` 覆盖所有退出路径。
- `PreviousForeground(AtomicIsize)` —— `launcher.rs`，`#[cfg(windows)]`，`app.manage`；`launcher::show()` 在 `set_focus()` 之前 `store(GetForegroundWindow())`。仅当该 HWND 不是本窗口时记录。
- setup 顺序：插件 → 快捷键 → **clipboard store + watcher**（在托盘前，依赖 `app_local_data_dir`；不依赖窗口）→ 平台钩子 → 托盘 → `init_hidden`。

## 4. 关键数据流

### 4.1 采集（Windows）

```text
windows.rs 消息循环收到 WM_CLIPBOARDUPDATE →
  GetClipboardSequenceNumber() 与上次相同则跳过（同一次复制可能触发多次）
  backend::read_snapshot()（arboard::Clipboard::new()，失败 warn 跳过；被占用时重试 3 次 × 50ms）
  优先级：file_list() > image() > text()
    - 文件：路径列表为空 → 跳过
    - 图片：ImageData{width,height,bytes(RGBA8)}；w*h*4 超 MAX_IMAGE_BYTES → 跳过；hash(w,h,bytes)；image 编码 PNG 原图 + thumbnail(THUMB_MAX_EDGE) PNG
    - 文本：trim 后为空 / 超 MAX_TEXT_BYTES → 跳过
  arboard::Clipboard 在同步块结束时 drop
  → tauri::async_runtime::spawn(store.record(captured)) → 写文件 + upsert + trim → emit clipboard://changed
```

- 监听线程为 `spawn_blocking`：消息窗口必须在创建它的线程上跑 `GetMessageW` 循环，且是阻塞循环；DB 写入回到异步运行时，避免在监听线程 `block_on`。
- `arboard::Clipboard` 非 `Send`，全部在同步块内用完（`state-events-async.md` §3）。
- `MAX_IMAGE_BYTES` 语义调整为解码后 RGBA 字节数（arboard 不暴露原始字节）；20 MiB ≈ 2300×2300 图。
- `read_snapshot` / `write` 是跨平台代码（arboard 在 macOS / Linux 同 API），后续平台只需补 `run_monitor` / `send_paste`。

### 4.2 粘贴（`paste_clipboard_item`）

```text
1. store.get(id) → Captured（image 读 images/<hash>.png；files 直接路径列表；文本全文）
2. spawn_blocking(backend::write(captured))：
     text  → arboard set().text()
     image → 读 images/<hash>.png → image 解码 RGBA8 → set().image(ImageData)（arboard 在 Windows 写 CF_DIBV5 + PNG 注册格式，兼顾新旧应用）
     files → set().file_list()
3. 取 PreviousForeground；IsWindow 且 != 自身 → SetForegroundWindow(hwnd)（此时本进程仍是前台进程，有权限）
4. launcher::hide(app)
5. 若第 3 步成功：spawn_blocking { sleep 50ms; SendInput(Ctrl↓ V↓ V↑ Ctrl↑) }  —— 用 scan-code 无关的 VK 方式；失败只 warn（B 兜底：内容已在剪贴板）
6. 监听器随即收到自己写入的更新 → hash 命中 → copied_at 上浮，无需命令层再更新
```

- 兜底路径：无前台记录 / 窗口已关闭 / SetForegroundWindow 返回 false → 跳过 3/5，只复制并收起。命令仍返回 Ok（对用户而言复制成功）。
- 文件被删除后再粘贴：`set_files` 仍写路径，目标应用自行报错；本版不校验（PRD 要求保留条目并标灰）。后端 list 时对每个路径做 `Path::exists` 填入 `ClipboardFile.exists`（§3.3）；前端收起态任一 `!exists` 即整行标灰，展开态逐行标灰。

### 4.3 列表刷新：本地变更 vs 重拉

两类变化分开处理，**自己发起的不重拉**：

| 变化 | 前端做法 | 理由 |
|---|---|---|
| `remove(id)` | `await deleteClipboardItem(id)` 成功后 `items.splice(i,1)`，`selectedIndex` 钳到相邻项 | 结果确定，重拉只会丢掉已加载的后续页、让滚动位置跳回顶部 |
| `toggleFavorite(id)` | 成功后本地翻转 `favorite`；若 `favoriteOnly` 开且变为未收藏，本地移除该行 | 收藏不改位置，不需要后端重新排序 |
| `paste(id)` | 不动列表；面板随即隐藏，上浮由监听器回捕的事件触发重拉 | 粘贴后用户已离开面板 |
| `clipboard://changed`（监听器录入） | 防抖 150ms → 从头重拉首页（`refresh()`：`before` 为空，响应替换 `items` 与 `hasMore`） | 新条目是否匹配当前 kind × 收藏 × 关键字，前端无法复现（只有 300 字 preview，`LIKE` 需要全文），重拉是唯一不会错的做法；且面板失焦即隐藏，用户无法在面板可见时去别处复制，这类事件几乎总在隐藏期间到达，重拉的成本与滚动重置用户看不到 |

其余触发：
- 工具页挂载 → `refresh()`。
- `query` prop 变化 → 防抖 200ms → `refresh()`。Tab / 只看收藏 切换 → 立即 `refresh()`（筛选变了，本地数据不再成立）。
- **分页状态只有两样**：`items` 与 `hasMore`。`hasMore` 只有一个判据，且只在每次 list 响应到达时写：`hasMore = !(page.length < PAGE_SIZE)`——返回不满一页（含空数组）即到底。本地变更（remove / toggleFavorite）**不碰它**，也不从 `items.length` 推（否则删一条后从 100 变 99 会被误判为到底）。不做 `limit+1` 探测。游标不存状态，`loadMore()` 时从 `items.at(-1)` 现取 `{copiedAt, id}`：本地删掉末条后自动退到新末条，被删行已不在库里不会重复；`items` 为空时退化为 `refresh()`。
- `loadMore()` 触发：列表底部哨兵元素进入视口（`IntersectionObserver`）且 `hasMore && !loading`。本地删到不满一屏时哨兵露出，自动补页，不需要额外逻辑。键盘 `↓` 到末条且 `hasMore` 时也调 `loadMore()`，避免键盘用户卡在第 100 条。
- 并发保护：本地变更与一次正在进行的 `refresh()` 交叉时，以 refresh 结果为准（后端是真相，它已包含该变更）；refresh 用递增序号丢弃过期响应。

## 5. 前端结构

```text
src/types/clipboard.ts               # ClipboardKind / ClipboardItem / ListQuery 镜像
src/lib/api/clipboard.ts             # 6 个封装 + toAssetUrl(path)（唯一 convertFileSrc 调用点）；非 Tauri 返回带「(浏览器预览)」的假数据
src/lib/events.ts                    # +CLIPBOARD_CHANGED: null
src/tools/clipboard/
├── index.ts                         # ViewToolModule：id "clipboard"、title "剪贴板"、icon "clipboard"、keywords ["clipboard","jtb","剪切板","粘贴","历史","paste"]、placeholder "搜索剪贴板历史…"
├── ClipboardPage.vue                # 单根 section；Tabs + 列表 + 空态 + 错误条；useKeymap 登记
├── components/
│   ├── ClipboardTabs.vue            # 左：全部 / 文本 / 图片 / 文件 Tab；右：「★ 收藏」切换按钮（独立筛选，与 Tab 叠加）；@mousedown.prevent
│   ├── ClipboardItemRow.vue         # 收起态一行：类型图标 + 摘要（text: line-clamp-2；image: 缩略图 + WxH；files: 首文件名 + 「等 N 项」，任一文件不存在则整行标灰）+ 相对时间 + 星标按钮（常显：favorite 实心 `star` fill / 否则空心 `star`；@click.stop 切换收藏不触发粘贴，@mousedown.prevent 不夺焦点）+ 右侧 chevron（可展开时常显，展开后旋转；@click.stop 不触发粘贴，@mousedown.prevent 不夺焦点）；展开时在下方渲染 ClipboardItemDetail
│   └── ClipboardItemDetail.vue      # 展开态：text → 头部「{charCount} 字 · {size}」+ <pre> 全文（自己调 getClipboardText 拉取，有 loading / error 态）；image → 原图 <img> + 「W×H · {size}」；files → 逐行完整路径，!exists 的行标灰 + 「不存在」。点击详情区不触发粘贴（@click.stop），但允许选中文本
├── composables/useClipboardHistory.ts  # items / kind / favoriteOnly / selectedIndex / expandedId / loading / error / refresh / loadMore / paste / remove / toggleFavorite / toggleFavoriteOnly / toggleExpanded(id)
└── lib/
    ├── format.ts + format.test.ts   # formatRelativeTime(ms, now)、formatBytes、summarizeFiles(files)（文件名由后端 DTO `ClipboardFile.name` 给出，前端不解析路径）、isExpandable(item)（§5.1.1）
    └── search.ts (可选)             # 无——搜索在后端
src/tools/icons.ts                   # +clipboard, +star, +chevron-down, +image, +file-text, +files（检查已有；无 trash / pin 图标——行内不放删除按钮）
src/tools/registry.ts                # modules = [clipboardTool]，删除 demo import
src/tools/demo/                      # 整目录删除
```

### 5.1 键位（`useKeymap`，工具页态）

| 键 | 行为 | 页脚 |
|---|---|---|
| `ArrowUp` / `ArrowDown` | 移动选中（循环不换 Tab；滚动到可见） | 「选择」 |
| `Enter` | 粘贴选中项 | 「粘贴」 |
| `Delete` | 删除选中项 | 「删除」 |
| `Ctrl+P` | 收藏 / 取消收藏选中项（只改星标，位置不变） | 「收藏」 |
| `Ctrl+F` | 切换「只看收藏」筛选（与 Tab / 搜索词叠加） | 「只看收藏」 |
| `Ctrl+ArrowLeft` / `Ctrl+ArrowRight` | 切换分类 Tab | 「切换分类」 |

筛选三维度（Tab × 只看收藏 × 搜索词）任一变化都从头重拉。**展开 / 收起也只经鼠标点 chevron，不登记快捷键**（用户决定；焦点常驻搜索框，Space / 单独方向键会与输入冲突，Ctrl 组合键已够多）。不登记 `Escape` / `Backspace` / `Tab`。

### 5.1.1 展开态（expandedId）

- `expandedId: number | null` 与 `selectedIndex` **独立**：展开不改选中，`↑`/`↓` 移动选中也不收起；同时只有一项展开（`toggleExpanded(id)`：相同则置 null，不同则替换）。
- 以下时机置 null：`refresh()` 被调用时（覆盖 Tab / 只看收藏 / 搜索词变化与监听器事件重拉）；`remove(id)` 删的正是展开项。`loadMore()` 、`toggleFavorite` 不动它。
- 可展开判定（决定是否渲染 chevron，前端纯函数 `isExpandable(item)`，进 `format.test.ts`）：
  - text：`truncated || preview.includes("\n")`（前端无法便宜地知道 2 行是否溢出；单行且未截断的短文本不给 chevron，超长单行但 < 300 字的文本会被 line-clamp 截断也不给，接受这个近似）。
  - image：总是可展开（缩略图与原图不同）。
  - files：`files.length > 1`（单文件时收起态已显示文件名，完整路径用 `title` 属性提示即可）。
- 文本全文拉取在 `ClipboardItemDetail` 内部：`onMounted` 调 `getClipboardText(id)`，组件随收起卸载即丢弃，不缓存（重新展开再拉，本地 SQLite 微秒级）。失败在详情区内显示错误文案，不进页面级 error。
- 展开后调 `scrollIntoView({ block: "nearest" })` 让详情区顶部可见；不影响底部哨兵（展开可能把哨兵推出视口，这是正常的）。

### 5.2 样式约束

- 遵守 `styling-guidelines.md`：语义 token（`bg-accent` 选中、`text-muted-foreground` 次要、`text-destructive` 错误），不写十六进制。
- 列表容器 `min-h-0 flex-1 overflow-y-auto`；收起态行高固定（文本 2 行 / 图片缩略图 56px）保证键盘滚动定位可预期；展开态附加在行下方，选中高亮覆盖整行（含详情区）。键盘定位用 `scrollIntoView({ block: "nearest" })`，不依赖固定行高计算，所以展开一项不破坏定位。
- 图片 `<img loading="lazy">`，`object-contain`，尺寸标签 `W×H`。
- 详情区高度上限 `max-h-72`（288px，约列表可用高度 496px 的一半，保证展开时上下郻能露出邻行），内部 `overflow-auto`：文本 `<pre class="whitespace-pre-wrap break-all font-mono text-xs">`（保留换行 / 缩进，长行折行）；图片 `max-h-72 w-auto object-contain`；文件路径 `break-all` 逐行，`!exists` 的行 `text-muted-foreground line-through` + 尾部「不存在」。
- chevron 用 lucide `chevron-down`，展开时 `rotate-180`，`transition-transform`；不可展开的行用等宽占位保持对齐。
- 星标用 lucide `star`：favorite 时 `fill-current text-primary`，否则空心 `text-muted-foreground`，hover 加深；行内唯一的动作按钮，无悬停显隐逻辑（常显，避免鼠标用户不知道能点）。删除没有鼠标入口，只走 `Delete` 键（页脚有提示）。

## 6. 配置变更

- `tauri.conf.json5`：`app.security.assetProtocol: { enable: true, scope: ["$APPLOCALDATA/clipboard/images/**"] }`（每项加中文注释；`$APPLOCALDATA` = `app_local_data_dir`，与 Rust 侧存储目录同源，不用 `$APPDATA`）。CSP 保持 `null`（收紧时需加 `img-src 'self' asset: http://asset.localhost`，把这句写进注释）。
- `capabilities/default.json`：无新增（asset 协议不走 ACL；命令是自定义命令不需权限）。
- `Cargo.toml`：§2 依赖，每项中文注释。
- `src-tauri/migrations/`：新目录，`sqlx::migrate!("./migrations")`。

## 7. 取舍与风险

| 决策 | 备选 | 为什么 |
|---|---|---|
| `arboard` 读写 + 自写平台监听 | `clipboard-rs` 全包 / `clipboard-win` 仅 Windows | 用户目标全平台：arboard 是主流跨平台库（1Password 维护），读写三平台一致；监听是各平台几十行薄代码，隔离在平台文件里。`clipboard-rs` 虽自带三平台监听但小众（下载量 1/50、个人维护）且引入第二份 `windows` crate；`clipboard-win` 只覆盖 Windows，后续平台读写要另找库 |
| 监听自写而非找 crate | — | Rust 生态没有主流的跨平台剪贴板监听库（arboard 明确不做）；Windows `AddClipboardFormatListener` 消息窗口约 80 行，可控 |
| 缩略图落盘 + asset 协议 | base64 内嵌 DTO | 列表 100 项 × 数十 KB base64 payload 过大；asset 协议由 webview 缓存、懒加载 |
| 文本全文展开时按需拉（`get_clipboard_text`） | 列表 DTO 直接带全文 | 单条上限 1 MiB，首屏 100 条最坏情况 100 MiB IPC；展开是低频动作，多一次微秒级本地查询无感。图片 / 文件展开不需新请求（原图走 asset，路径列表本就很小） |
| 展开为行内手风琴 + 仅鼠标触发 | 右侧预览面板 / 快捷键展开 | 面板宽度有限，右侧预览面板已列为 Out of Scope；焦点常驻搜索框，可用的无冲突键只剩 Ctrl 组合，用户选择不再增加快捷键 |
| 像素哈希 | PNG 字节哈希 | 写回后格式往返会改字节，像素不变；代价是解码一次（本来就要解码生成缩略图） |
| 事件只发「变了」，且只由监听器发 | 事件带新条目前端本地插入 / 命令也 emit | 前端无法复现后端的 `LIKE` 筛选判断新条目该不该插入；监听器事件几乎总在面板隐藏时到达，重拉无感。用户自己的删除 / 收藏则本地直接改，不重拉、不丢分页（§4.3） |
| 淘汰在 upsert 后同步执行 | 定时任务 | 简单且写入频率低（人手复制） |
| `SetForegroundWindow` 在 hide 前调用 | hide 后调用 | hide 后本进程可能失去前台进程资格，`SetForegroundWindow` 会静默失败（只闪烁任务栏） |
| 自身写回不做 owner 过滤 | 按 `seq_num` 忽略自身写入 | 上浮到顶正是期望行为（uTools 同款） |

风险：
- **sqlx + bundled SQLite 首次编译时间长**（libsqlite3-sys C 编译）；接受。
- `SendInput` 对以管理员权限运行的目标窗口无效（UIPI）；记录为已知限制，不处理。
- 某些应用（Excel）延迟渲染剪贴板格式，`WM_CLIPBOARDUPDATE` 到达时读取可能失败或为空；重试 3 次后跳过空快照。
- arboard 在 Windows 上读图片走 `CF_DIBV5`/`CF_DIB`，部分应用只放 PNG 注册格式时可能读不到；若手测发现，备选是在 `windows.rs` 用现有 `windows` crate 补读 `PNG` 格式（不引入新依赖）。
- 回滚：本任务全部为新增文件 + 少量 `lib.rs` / `launcher.rs` / `error.rs` / 配置行的追加，`git revert` 单 commit 即可回滚；DB 文件位于 `%LOCALAPPDATA%\<identifier>\clipboard\`，回滚后残留无害，删整个 `<identifier>` 目录即彻底清理。

## 变更记录

- 已移除「一键清空历史」（用户决定，实现后砍掉）：删除 `clear_clipboard_history` 命令、`clear_history` 编排、`store.clear`、前端 `clearClipboardHistory` / `useClipboardHistory.clear` / Tab 栏「清空」按钮与二次确认。收藏的语义只剩「筛选维度 + 免于 trim 淘汰」。
