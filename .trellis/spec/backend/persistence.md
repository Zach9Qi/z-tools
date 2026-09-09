# 持久化规范(sqlx + SQLite)

> 参考实现:`src-tauri/src/clipboard/store.rs`(`ClipboardStore` 类型 + 全部 SQL + 图片文件读写删)、`src-tauri/src/clipboard.rs`(哈希、`record` 编排)、`src-tauri/migrations/0001_clipboard.sql`、`lib.rs::setup_clipboard`(打开)。本文件写的全部是仓库里真实存在的签名与行为;新增第二张表或第二个库时按同一套约定来。
> 落盘目录约定(统一 `app_local_data_dir()`)见 `config-and-permissions.md` §2。

---

## Scenario: 剪贴板历史存储(`clipboard_items`)

### 1. Scope / Trigger

- Trigger:本仓库第一处数据库 schema / 迁移、第一批可失败 async 命令、第一个基础设施依赖(sqlx),属于「必须写到 code-spec 深度」的三类变更。
- 覆盖:依赖与 feature、迁移文件规则、打开与损坏恢复、查询写法(去重 / 分页 / 搜索)、阻塞 IO 归属、测试与日志。

### 2. Signatures

**依赖(`Cargo.toml`)**

```toml
# 只用运行时 sqlx::query*,不用 query! 宏;macros 只为 sqlx::migrate!(它由 sqlx-macros 提供)
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "migrate", "macros"] }

[dev-dependencies]
# 仅测试:#[tokio::test] 驱动内存库单测;运行时代码只用 tauri::async_runtime
tokio = { version = "1", features = ["macros", "rt"] }
```

- `runtime-tokio` 对齐 Tauri 内置运行时;`sqlite` 自带 bundled `libsqlite3-sys`(首次编译慢,接受)。
- **禁止 `sqlx::query!` / `query_as!` / `query_scalar!`**:它们在编译期连库校验 SQL,需要 `DATABASE_URL` 或 `.sqlx/` 离线数据,会让 clone 下来直接 `cargo build` 失败;本仓库 SQL 全部用运行时 API,行解码走 **`#[derive(sqlx::FromRow)]` 行结构体 + `query_as`**(单列用 `query_scalar`),不手写 `row.try_get("列名")`(见下文「行 → DTO」)。

**迁移(`store.rs`)**

```rust
/// 迁移脚本编译期内嵌(`src-tauri/migrations/`),首次连接时执行
const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
```

- 文件放 `src-tauri/migrations/000N_xxx.sql`(路径相对 `Cargo.toml`);`cargo build` 会把目录内容内嵌进二进制,新增文件后不需要额外注册。
- 现有:`0001_clipboard.sql`,建表 `clipboard_items`(列语义见 §3)。

**打开 / 状态(`store.rs` / `lib.rs`)**

`ClipboardStore` 的 struct 定义与全部 `impl` 都在 `store.rs`(父模块只 `pub use store::ClipboardStore`)。它管的是「历史存储」这一件事 = SQLite 行 + `images/` 目录里的图片文件:一条 image 记录是一行加两个文件,命名 / 读写 / 删除的知识只放一处,不再把 struct 与 impl 拆到两个文件。文件内两个 `impl` 块:SQL 块与图片文件块。

```rust
/// 剪贴板历史存储:sqlx 连接池 + 图片目录。`app.manage` 托管;`Clone` 只是克隆 Arc 级句柄
#[derive(Debug, Clone)]
pub struct ClipboardStore { pool: SqlitePool, images_dir: PathBuf }

impl ClipboardStore {
    pub async fn open(db_path: &Path, images_dir: PathBuf) -> Result<Self, AppError>;
    pub async fn upsert(&self, captured: &Captured, now: i64) -> Result<(), AppError>;
    pub async fn trim(&self) -> Result<Vec<String>, AppError>;              // 返回被淘汰图片条目的 image_file
    pub async fn list(&self, query: &ListQuery) -> Result<Vec<ClipboardItem>, AppError>;
    pub async fn get_captured(&self, id: i64) -> Result<Captured, AppError>;
    pub async fn get_text(&self, id: i64) -> Result<String, AppError>;
    pub async fn delete(&self, id: i64) -> Result<Option<String>, AppError>; // 返回 image_file
    pub async fn set_favorite(&self, id: i64, favorite: bool) -> Result<(), AppError>;
    // 图片文件块(同步 fs,不决定跑在哪个线程)
    pub fn save_image(&self, captured: &Captured) -> Result<(), AppError>;   // 非图片 no-op;两文件已存在则跳过
    pub fn remove_image_files(&self, image_files: &[String]);               // 传 trim / delete 返回的文件名
}
```

`open` 的连接参数(真实值):`SqliteConnectOptions::new().filename(db_path).create_if_missing(true).busy_timeout(Duration::from_secs(5))`,`SqlitePoolOptions::new().max_connections(4)`;随后 `MIGRATOR.run(&pool).await.map_err(sqlx::Error::from)?`。

**打开策略(`lib.rs::setup_clipboard`)**

```rust
// 库打不开直接让 setup 失败、应用报错退出:不做改名重建之类的自动恢复
let store = block_on(ClipboardStore::open(&db_path, images_dir))?;
app.manage(store);
```

- `open` 失败(文件损坏 / 迁移 checksum 不符 / 目录无权限)→ `?` 让 `setup` 返回 `Err`,应用启动失败并报错。**不做**自动隔离 / 改名 / 重建(用户决定):静默丢历史比启动失败更糟,用户看到错误后自行处理文件;继续启动也不可行——列表命令会因无托管状态而 panic。

### 3. Contracts

**Schema(`0001_clipboard.sql`,只列约束与语义)**

| 列 | 类型 / 约束 | 语义 |
|---|---|---|
| `id` | `INTEGER PRIMARY KEY AUTOINCREMENT` | 并列 `copied_at` 时的排序兜底;后录入者 id 更大 |
| `kind` | `TEXT NOT NULL CHECK (kind IN ('text','image','files'))` | `ClipboardKind::as_str()` 的取值;读到其它值 → `AppError::Database(sqlx::Error::Decode)` |
| `hash` | `TEXT NOT NULL UNIQUE` | blake3 hex,**带类型域前缀** `"<kind>\0"`(`clipboard::domain_hash`);去重与上浮的唯一依据 |
| `text` | `TEXT` | 可搜索 / 可展示文本:text = 全文;files = 文件名以 `\n` 拼接;image = `NULL`。唯一产出点 `clipboard::searchable_text` |
| `image_file` | `TEXT` | `<hash>.png`;缩略图 `<hash>.thumb.png` 由 `thumb_file_name` 推导,不入库 |
| `image_width` / `image_height` | `INTEGER` | 仅 image |
| `files` | `TEXT` | 完整路径 JSON `string[]`;Rust 侧类型 `Option<Json<Vec<String>>>`(sqlx `json` feature),绑定 / 解码都由 sqlx 做,存储层不手写 `serde_json` |
| `size` | `INTEGER NOT NULL` | text = utf8 字节;image = png 字节;files = 0 |
| `favorite` | `INTEGER NOT NULL DEFAULT 0` | 只是筛选维度 + 免于 trim 淘汰;**不参与排序** |
| `created_at` / `copied_at` | `INTEGER NOT NULL` | unix ms;`copied_at` 是主排序键,upsert 时只更新它 |

不建额外索引(注释写在迁移文件里):表最大约 500 非收藏 + 收藏,列表几乎都带 `LIKE` 必然全扫;`hash UNIQUE` 已隐式给去重索引。

**迁移文件不可变(实机踩过的坑)**

- sqlx 用整个文件的 checksum 记录已应用的迁移。**已提交的 `000N_xxx.sql` 一个字节都不能再改——包括注释、空白、换行符**,否则已有用户的库在 `MIGRATOR.run` 时报 `migration N was previously applied but has been modified`,应用直接启动失败。本任务开发期间改了 `0001` 的一行注释,本机库就立刻打不开了。
- 变更 schema 只能新增 `0002_xxx.sql`;字段废弃留着不删(SQLite `DROP COLUMN` 受限,且不值得)。
- 迁移文件头部保留「【不可变】」提示注释,让下一个人在编辑器里就看到。

**查询写法**

| 需求 | 写法(`store.rs` 真实 SQL) | 为什么 |
|---|---|---|
| 去重上浮 | `INSERT … ON CONFLICT(hash) DO UPDATE SET copied_at = excluded.copied_at` | 一条语句完成「新内容插入 / 旧内容上浮」,不需要先 SELECT;不改 `favorite` / `created_at` |
| 淘汰 | `DELETE … WHERE favorite = 0 AND id NOT IN (SELECT id … WHERE favorite = 0 ORDER BY copied_at DESC, id DESC LIMIT ?) RETURNING image_file` | 收藏不计入上限;`RETURNING` 把要删的图片文件名一次带回,不用二次查询 |
| 排序 | 一律 `ORDER BY copied_at DESC, id DESC` | SQLite 对并列行不保证顺序;粘贴写回 + 回捕可能落在同一毫秒,没有 `id` 兜底刷新后选中项会跳行 |
| 分页 | **keyset 游标** `(copied_at, id) < (?, ?)`,不用 `OFFSET` | 用户滚动期间监听器会插入新行,`OFFSET` 会让第二页重复返回第一页末尾的条目;游标只看「比上一页末条更旧」,不受新行影响。行值比较需 SQLite ≥ 3.15,bundled 版本满足 |
| 关键字 | `text LIKE ? ESCAPE '\'`,模式由 `like_pattern()` 生成:先转义 `\` `%` `_`,再两侧加 `%` | 用户输入的 `%` / `_` 不能变成通配;图片 `text IS NULL` 天然不命中,不需要按 kind 特判 |
| 动态 WHERE | 条件 `push` 进 `Vec<&str>` 后 `join(" AND ")`,**bind 顺序与 push 顺序一致**(注释已写在代码里) | 运行时拼接的唯一风险是 bind 错位,用同一段顺序写两遍并加注释锁住 |
| 单条读取 | `query_as::<_, ItemRow>("SELECT * … WHERE id = ?")` + `fetch_optional` → `ok_or_else(InvalidInput("记录不存在"))` | 列名只在 `ItemRow` 声明一次,加列 / 改名只改 struct;`SELECT *` 靠 `FromRow` 按名匹配 |
| `RETURNING` 单列 | `query_scalar::<_, Option<String>>("DELETE … RETURNING image_file")` | 单列不值得造 struct;`Option` 因非图片行该列为 NULL |
| 删除 / 改标记 | `DELETE … RETURNING image_file` / `UPDATE … SET favorite = ?`,`rows_affected() == 0` → `InvalidInput` | 不存在的 id 必须报错,前端据此发现本地列表已过期 |

**行 → DTO 走 `FromRow` 行结构体,不手写 `try_get`(本任务返工一次的经验)**

`store.rs` 里唯一直接对应表结构的类型是 `#[derive(sqlx::FromRow)] struct ItemRow { id, kind: ClipboardKind, hash, text: Option<String>, files: Option<Json<Vec<String>>>, favorite: bool, … }`,字段名 = 列名。能让 sqlx 直接解码成目标类型的列就直接解码(`ClipboardKind` 派生 `sqlx::Type`、`bool`、`Json<T>`),不要先解成 `String` / `i64` 再手写转换函数。`list` / `fetch_row` 用 `query_as::<_, ItemRow>("SELECT * …")` 解码,再由 `ClipboardStore::to_item(ItemRow) -> ClipboardItem` 与 `get_captured` 转成对外类型。

- 为什么不直接 `query_as::<_, ClipboardItem>`:对外 DTO 是按 `kind` 分支的 tagged enum,且一半字段是**派生值**(`preview` / `char_count` / `truncated` 由 `text` 算,`image_path` / `thumb_path` 要拼 `images_dir`,`files[].name` / `exists` 要拆路径 + 查文件系统),`FromRow` 只能派生到与列 1:1 的 struct。所以中间必须有一层行结构体,派生逻辑放在它到 DTO 的转换函数里。
- 为什么不手写 `try_get`(第一版就是这样,评审后重构):列名字符串会散落在 `row_to_item` / `row_to_captured` 等多处,还要与 `COLUMNS` 常量手工对齐;加一列或改名在**运行时**才炸,报错点还不在改动处。`FromRow` 把列名收敛到一个声明,解码失败集中在 `query_as` 一处报出;转换函数拿到的是普通 struct,不再依赖 sqlx 类型,以后能不开库单测。
- 判据:**一张表一个 `XxxRow`**;对外 DTO 与列不是 1:1 时,`XxxRow -> DTO` 单独写成函数,不要退回 `try_get`。
- 类型专属列(`text` / `image_*` / `files`)在 schema 上可空是因为三种 kind 共用一张表,**不是**因为对应 kind 下该值可选:唯一写入口 `upsert` 保证本 kind 需要的列一定非空,读回用 `unwrap_or_default` 取值即可。不要为「列被外部改坏」写防御分支(第一版有 `kind_of` / `files_of` / `dimension_of` 三个,评审后删除):自己写的数据被改坏了就让它报错,静默变成 0 / 空列表只会把问题藏得更深。

**去重哈希必须带类型域(评审抓到的碰撞)**

```rust
/// 带类型域的 blake3:先喂入 "<kind>\0" 再依次喂入 parts
pub fn domain_hash(kind: ClipboardKind, parts: &[&[u8]]) -> String
```

- `hash` 列 `UNIQUE` 跨三种类型。没有域前缀时 `Captured::Text("a")` 与 `Captured::Files(["a"])` 的字节完全相同 → 同一个 hash → 后者永远录不进去(upsert 只上浮了前者)。三种类型(含 backend 的像素哈希 `blake3("image\0", w, h, rgba8)`)都走 `domain_hash`。
- 哈希用 blake3 而不是 std `DefaultHasher`:后者不保证跨版本稳定,升级 Rust 后所有旧行都会被当成新内容。

**阻塞 IO 归属**

- `list` 里对 files 条目逐路径同步 `Path::exists()`:只针对本地路径设计;断开的 UNC / 网络盘路径可能阻塞数秒,**已知风险,本版不处理**(注释写在 `to_item`)。
- `get_captured` 对图片 `std::fs::read` 同步读:本地小文件毫秒级,不值得多一次线程切换(注释已写)。
- 图片文件写入(`store.rs::save_image` → `write_atomically`):先写 `<path>.tmp` 再 `rename` 覆盖。理由:写入中途崩溃不会留下半个文件被 `exists()` 当成完整图片跳过重写。`save_image` 本身是同步函数,**放 `spawn_blocking` 的决定在 `clipboard.rs::record_inner`**(`store.clone()` 是 Arc 级句柄,`captured.clone()` 拷 PNG 字节进线程):store 不依赖 `tauri::async_runtime`,内存库单测可以直接同步调用。
- 文件删除(`store.rs::remove_image_files`):SQL 方法(`trim` / `delete`)用 `RETURNING image_file` 把文件名带回,编排函数紧接着调 `remove_image_files`——分两步是为了 SQL 方法能在内存库里单测而不碰磁盘,不是因为删文件不属于 store。`NotFound` 视为成功,其它失败只 `warn`(行已删,文件残留无害)。

**日志**

- `lib.rs` 的 `tauri_plugin_log::Builder` 上 `.level_for("sqlx", log::LevelFilter::Warn)`:sqlx 在 Debug 级把每条 SQL(含整段迁移脚本)打一行,会淹没业务日志;只压这一个 target,不整体降级。

### 4. Validation & Error Matrix

| 条件 | 位置 | 结果 |
|---|---|---|
| `limit` 不在 `1..=200` | 命令层 `validate_limit` | `InvalidInput("每页条数必须在 1..=200 之间")` |
| `query.query` 只有空白 | 命令层 `trim()` | 视为不过滤(空串) |
| id 不存在(`get_captured` / `get_text` / `delete` / `set_favorite`) | store | `InvalidInput("记录不存在")`(常量 `NOT_FOUND`) |
| `get_text` 命中非文本条目 | store | `InvalidInput("该条目不是文本")` |
| `kind` 列取值不在三种之内 / `files` 列不是合法 JSON | sqlx `FromRow` 解码 | `AppError::Database(sqlx::Error::Decode(...))`(库被外部改坏,不做兜底) |
| 图片原图文件丢失(`get_captured`) | `std::fs::read` | `AppError::Io` |
| 任何 sqlx 错误(busy 超时、磁盘满、迁移失败) | `#[from] sqlx::Error` | `AppError::Database`,文案「数据库错误: …」 |
| `open` 失败 | `setup_clipboard` | `?` → setup `Err` → 应用启动失败报错;不自动恢复 |
| `record()` 内任何失败 | `clipboard.rs` | 只 `log::warn!`,不 emit 事件,不向上传播(监听器没有调用方) |

### 5. Good/Base/Bad Cases

- Good:`upsert(Text("hello"), 3)` 在已有 `hello`(copied_at 1)时 → 行数不变、`copied_at = 3`、`created_at` 仍为 1、`favorite` 不变。
- Good:第一页 `limit 4` 拉到 `t10..t7`,期间插入 `brand-new`(copied_at 99),第二页用 `(copied_at, id)` 游标仍是 `t6..t3`,无重复无漏项;新条目只在 `refresh()` 首页出现。
- Base:`list(ListQuery::default() + limit 200)` 空库 → `Ok(vec![])`;`trim()` 在不超额时 → `Ok(vec![])`。
- Bad:改动已提交的 `0001_clipboard.sql` 任何字符 → 用户的应用启动失败。
- Bad:用 `OFFSET` 分页 → 滚动期间新复制一条,第二页首条与第一页末条重复。
- Bad:用 `sqlx::query!` → `cargo build` 在没有 `DATABASE_URL` 的机器上失败。
- Bad:哈希不带域前缀 → 文本 `D:\a.txt` 与单文件列表 `[D:\a.txt]` 永远只存一条。

### 6. Tests Required

内存库 + `#[tokio::test]`(`store.rs` 的 `memory_store()`):

```rust
// 连接数固定为 1 且永不回收,否则每个连接各有一份独立的 :memory: 数据
SqlitePoolOptions::new().max_connections(1).idle_timeout(None).max_lifetime(None).connect("sqlite::memory:")
```

以下是 `store.rs` 现有测试覆盖的断言点,改动任何 SQL 后必须全绿;新增表 / 查询按同一粒度补:

| 测试 | 断言点 |
|---|---|
| `upsert_same_content_bumps_copied_at_without_new_row` | 行数不增;`copied_at` 变为最新;`created_at` 保持首次 |
| `trim_evicts_oldest_non_favorites_and_keeps_favorites` | 非收藏恰好剩 `MAX_ITEMS`;收藏全保留;返回被淘汰图片的 `image_file`;再 trim 一次返回空 |
| `list_filters_by_kind_favorite_and_query_together` | kind / kind+query / kind+favorite+query / favorite+query(跨类型命中文件名)四组结果数;有关键字时 image Tab 为空 |
| `ordering_ignores_favorite_and_breaks_ties_by_id_desc` | 同 `copied_at` 时 id 大者在前;收藏最旧一条后顺序不变 |
| `cursor_pagination_survives_new_items_inserted_between_pages` | 三页拉完原 10 条各出现一次;中途插入的新行不在追加页里 |
| `cursor_pagination_with_equal_copied_at_uses_id` | 全部同 `copied_at` 时第二页 id 都 < 第一页末条 |
| `file_search_matches_file_name_but_not_directory` | 文件名命中;目录名(`Projects` / `photos` / `home`)不命中;`ClipboardFile.name` / `path` / `exists` 正确 |
| `image_never_matches_keyword_but_lists_without_one` | 任何关键字(含 `%`)不命中图片;无关键字时图片 `image_path` / `thumb_path` / 宽高 / size 正确 |
| `like_wildcards_in_keyword_are_escaped` | `%` / `_` / `\` 只命中字面量;`like_pattern(r"a%b_c\d") == r"%a\%b\_c\\d%"` |
| `set_favorite_keeps_copied_at_and_rejects_missing_id` | `copied_at` 不变;不存在 id → `"参数错误: 记录不存在"` |
| `delete_returns_image_file_and_rejects_missing_id` | 图片条目返回 `Some(file)`,文本返回 `None`;二次删除报错 |
| `get_text_returns_full_untrimmed_text_and_rejects_non_text` | 全文原样(含首尾空白);image / files → `"参数错误: 该条目不是文本"` |
| `preview_is_truncated_and_trimmed` | `truncated` 与 `char_count` 正确;截断后再 trim;多字节字符按 char 数 |
| `get_captured_round_trips_text_files_and_image` | `save_image` 落盘原图 + 缩略图且不留 `.tmp`、非图片 no-op;三类快照与 upsert 时一致;`remove_image_files` 两文件一起删且忽略不存在(临时目录,结束清理) |
| `image_file_names_are_derived_from_hash` | `<hash>.png` / `<hash>.thumb.png` 推导 |

领域层纯函数(`clipboard.rs` / `backend.rs`,普通 `#[test]`):`searchable_text` 三分支、`hash_is_stable_per_kind`(含跨类型不碰撞、与裸 blake3 不同)、`ListQuery` 缺省字段反序列化、`ClipboardItem` 序列化带 `kind` 标签且字段 camelCase。

### 7. Wrong vs Correct

#### Wrong

```rust
// 编译期宏:没有 DATABASE_URL 的机器直接编译失败
let rows = sqlx::query!("SELECT id FROM clipboard_items WHERE id = ?", id).fetch_all(&pool).await?;

// 手写 try_get:列名散落多处、与 SELECT 列清单手工对齐,加列 / 改名运行时才炸
const COLUMNS: &str = "id, kind, text, …";
let id: i64 = row.try_get("id")?;
let text: Option<String> = row.try_get("text")?;

// OFFSET 分页:滚动期间插入新行后第二页与第一页重叠
"SELECT ... ORDER BY copied_at DESC LIMIT ? OFFSET ?"

// 裸哈希:Text("a") 与 Files(["a"]) 碰撞
blake3::hash(text.as_bytes()).to_hex()

// 已提交迁移里"顺手"改注释 → 用户库 checksum 不符,应用起不来
-- 0001_clipboard.sql(已发布)
-- 修一下这里的措辞
```

#### Correct

```rust
// 运行时查询 + FromRow 行结构体;派生字段在 to_item 里算
#[derive(sqlx::FromRow)]
struct ItemRow { id: i64, kind: String, text: Option<String>, /* … 与列 1:1 */ }
let row = sqlx::query_as::<_, ItemRow>("SELECT * FROM clipboard_items WHERE id = ?")
    .bind(id).fetch_optional(&self.pool).await?
    .ok_or_else(|| AppError::InvalidInput(NOT_FOUND.into()))?;
let item = self.to_item(row)?;

// keyset 游标
conditions.push("(copied_at, id) < (?, ?)");  // bind cursor.copied_at, cursor.id
"... ORDER BY copied_at DESC, id DESC LIMIT ?"

// 类型域哈希
domain_hash(ClipboardKind::Text, &[text.as_bytes()])

// schema 变更:新增文件,旧文件一字不动
-- src-tauri/migrations/0002_add_source_app.sql
ALTER TABLE clipboard_items ADD COLUMN source_app TEXT;
```

## 新增一张表 / 一个库时的固定动作

1. `src-tauri/migrations/000N_<名>.sql`,头部写「【不可变】」注释;只增不改。
2. 存储类型 `XxxStore { pool, … }` 的定义与全部 `impl` 放 `src/<domain>/store.rs`,父模块 `pub use`;它负责该领域的**全部持久化**(SQL + 行 → DTO 整形 + 随行落盘的文件)。事件 / 剪贴板 / 系统 API / 「阻塞 IO 放哪个线程」留在领域编排函数。
3. 全部运行时 API:`#[derive(sqlx::FromRow)] struct XxxRow`(字段 = 列)+ `query_as`;单列 `query_scalar`;无返回 `query`。动态 WHERE 用 `Vec<&str>` + 顺序 bind 并注释。DTO 与列不 1:1 时写 `XxxRow -> DTO` 转换函数,不手写 `try_get`。
4. 排序键带唯一列兜底;列表分页用 keyset 游标。
5. `open` 走 `create_if_missing` + `busy_timeout`;打开失败直接 `?` 让 setup 失败,不做自动恢复(用户决定,见 §2)。
6. 内存库 `#[tokio::test]` 覆盖:去重、过滤叠加、排序并列、游标跨插入、不存在 id 报错文案。
7. 新的第三方 target 若刷屏,在 `lib.rs` 的日志插件上 `.level_for(...)` 压低。
