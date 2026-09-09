//! 剪贴板历史存储 `ClipboardStore`：SQLite 行（sqlx 运行时查询）+ `images/` 目录下的图片文件。
//! 一条 image 记录 = 一行 + 原图 / 缩略图两个文件，两者的命名、读写、删除都在这一个文件里。
//!
//! 边界：不碰剪贴板、不发事件、不调系统 API；也不决定阻塞 IO 跑在哪个线程（`save_image` 是同步函数，由 `clipboard.rs::record`
//! 放 `spawn_blocking`）。`text` 列的产出规则在父模块 `searchable_text`，这里只存取。
//!
//! 文件内两个 `impl` 块：前一个是 SQL（行 → DTO 整形），后一个是图片文件 IO。

use std::path::{Path, PathBuf};
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::types::Json;

use super::{
    Captured, ClipboardFile, ClipboardItem, ClipboardKind, ListQuery, MAX_ITEMS, PREVIEW_CHARS,
    file_name_of, searchable_text,
};
use crate::error::AppError;

/// 迁移脚本编译期内嵌（`src-tauri/migrations/`），首次连接时执行
const MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// 剪贴板历史存储：sqlx 连接池 + 图片目录。`app.manage` 托管；`Clone` 只是克隆 Arc 级句柄，供后台任务持有。
#[derive(Debug, Clone)]
pub struct ClipboardStore {
    pool: SqlitePool,
    images_dir: PathBuf,
}

/// `clipboard_items` 的一行，字段名 = 列名（`FromRow` 按名匹配）。
///
/// 这是「行 → DTO」的唯一中间层：`SELECT *` 经 `query_as` 解码到这里，再由 `ClipboardStore::to_item` /
/// `get_captured` 转成对外类型。列名只在这一个 struct 里出现，加列 / 改名只改这里。
///
/// 类型专属列（text / image_* / files）在 schema 上可空是因为三种 kind 共用一张表，不是因为对应 kind 下该值可选：
/// 唯一写入口 `upsert` 保证本 kind 需要的列一定非空，读回时用 `unwrap_or_default` 取值即可，不再逐列防御。
#[derive(Debug, sqlx::FromRow)]
struct ItemRow {
    id: i64,
    kind: ClipboardKind,
    hash: String,
    text: Option<String>,
    image_file: Option<String>,
    image_width: Option<i64>,
    image_height: Option<i64>,
    /// kind = files 时为完整路径 JSON string[]，sqlx 直接解码
    files: Option<Json<Vec<String>>>,
    size: i64,
    favorite: bool,
    copied_at: i64,
}

/// 记录不存在时的统一文案
const NOT_FOUND: &str = "记录不存在";

impl ClipboardStore {
    /// 打开（不存在则创建）`db_path` 并执行迁移；`images_dir` 由调用方保证已存在。
    ///
    /// 迁移按文件 checksum 校验：`migrations/` 下已提交的脚本（含注释）不得再改，否则已有用户库会报
    /// “previously applied but has been modified” 导致启动失败；schema 变更一律新增 `000N_xxx.sql`。
    pub async fn open(db_path: &Path, images_dir: PathBuf) -> Result<Self, AppError> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            // 监听器写入与列表读取可能并发，遇锁等待而不是立刻报 busy
            .busy_timeout(Duration::from_secs(5));
        // 本地单用户库，少量连接足够；多了只会争 SQLite 的单写锁
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        MIGRATOR.run(&pool).await.map_err(sqlx::Error::from)?;
        Ok(Self { pool, images_dir })
    }

    /// 写入一条快照：同 `hash` 已存在则只把 `copied_at` 上浮为 `now`（不新增、不改收藏），否则插入新行。
    pub async fn upsert(&self, captured: &Captured, now: i64) -> Result<(), AppError> {
        let text = searchable_text(captured);
        let (image_file, width, height, files, size) = match captured {
            Captured::Text(t) => (None, None, None, None, t.len() as i64),
            Captured::Image {
                png,
                width,
                height,
                rgba_hash,
                ..
            } => (
                Some(image_file_name(rgba_hash)),
                Some(i64::from(*width)),
                Some(i64::from(*height)),
                None,
                png.len() as i64,
            ),
            Captured::Files(paths) => (None, None, None, Some(Json(paths)), 0),
        };
        sqlx::query(
            "INSERT INTO clipboard_items \
             (kind, hash, text, image_file, image_width, image_height, files, size, favorite, created_at, copied_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?) \
             ON CONFLICT(hash) DO UPDATE SET copied_at = excluded.copied_at",
        )
        .bind(captured.kind())
        .bind(captured.hash())
        .bind(text)
        .bind(image_file)
        .bind(width)
        .bind(height)
        .bind(files)
        .bind(size)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 淘汰超出 `MAX_ITEMS` 的非收藏条目（按 `copied_at`、`id` 保留最新），返回被删图片条目的原图文件名。
    pub async fn trim(&self) -> Result<Vec<String>, AppError> {
        // RETURNING 单列用 query_scalar 直接解码为 Option<String>（非图片行为 NULL）
        let image_files = sqlx::query_scalar::<_, Option<String>>(
            "DELETE FROM clipboard_items WHERE favorite = 0 AND id NOT IN \
             (SELECT id FROM clipboard_items WHERE favorite = 0 ORDER BY copied_at DESC, id DESC LIMIT ?) \
             RETURNING image_file",
        )
        .bind(MAX_ITEMS)
        .fetch_all(&self.pool)
        .await?;
        Ok(image_files.into_iter().flatten().collect())
    }

    /// 列表：kind × favorite_only × query 三条件 AND 叠加 + `(copied_at, id) < 游标`，
    /// `ORDER BY copied_at DESC, id DESC LIMIT limit`。`limit` 的范围校验在命令层。
    pub async fn list(&self, query: &ListQuery) -> Result<Vec<ClipboardItem>, AppError> {
        let mut conditions: Vec<&str> = Vec::new();
        if query.kind.is_some() {
            conditions.push("kind = ?");
        }
        if query.favorite_only {
            conditions.push("favorite = 1");
        }
        let pattern = (!query.query.is_empty()).then(|| like_pattern(&query.query));
        if pattern.is_some() {
            // 图片 text 为 NULL，LIKE 天然不命中，不需要按 kind 特判
            conditions.push("text LIKE ? ESCAPE '\\'");
        }
        if query.before.is_some() {
            conditions.push("(copied_at, id) < (?, ?)");
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };
        let sql = format!(
            "SELECT * FROM clipboard_items{where_clause} ORDER BY copied_at DESC, id DESC LIMIT ?"
        );

        // bind 顺序必须与上面 push 条件的顺序一致
        let mut q = sqlx::query_as::<_, ItemRow>(&sql);
        if let Some(kind) = query.kind {
            q = q.bind(kind);
        }
        if let Some(pattern) = &pattern {
            q = q.bind(pattern);
        }
        if let Some(cursor) = query.before {
            q = q.bind(cursor.copied_at).bind(cursor.id);
        }
        q = q.bind(i64::from(query.limit));

        let rows = q.fetch_all(&self.pool).await?;
        rows.into_iter().map(|row| self.to_item(row)).collect()
    }

    /// 读回一条用于粘贴的快照；图片会从 `images_dir` 读入原图 PNG（缩略图留空）。不存在 → `InvalidInput`。
    pub async fn get_captured(&self, id: i64) -> Result<Captured, AppError> {
        let row = self.fetch_row(id).await?;
        match row.kind {
            ClipboardKind::Text => Ok(Captured::Text(row.text.unwrap_or_default())),
            ClipboardKind::Files => Ok(Captured::Files(row.files.unwrap_or_default().0)),
            ClipboardKind::Image => {
                let image_file = row.image_file.unwrap_or_default();
                // 原图是本地小文件（PNG 压缩后远小于 20 MiB 上限），同步读取毫秒级，不值得多一次线程切换
                let png = std::fs::read(self.images_dir.join(&image_file))?;
                Ok(Captured::Image {
                    png,
                    thumb_png: Vec::new(),
                    width: row.image_width.unwrap_or_default() as u32,
                    height: row.image_height.unwrap_or_default() as u32,
                    rgba_hash: row.hash,
                })
            }
        }
    }

    /// 文本条目全文（原样，不 trim）。不存在 → `InvalidInput("记录不存在")`；非文本 → `InvalidInput("该条目不是文本")`。
    pub async fn get_text(&self, id: i64) -> Result<String, AppError> {
        let row = self.fetch_row(id).await?;
        if row.kind != ClipboardKind::Text {
            return Err(AppError::InvalidInput("该条目不是文本".into()));
        }
        Ok(row.text.unwrap_or_default())
    }

    /// 删除一条，返回其图片文件名（非图片为 `None`）。不存在 → `InvalidInput`。
    pub async fn delete(&self, id: i64) -> Result<Option<String>, AppError> {
        sqlx::query_scalar::<_, Option<String>>(
            "DELETE FROM clipboard_items WHERE id = ? RETURNING image_file",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::InvalidInput(NOT_FOUND.into()))
    }

    /// 只改收藏标记，不动 `copied_at`（收藏不改变位置）。不存在 → `InvalidInput`。
    pub async fn set_favorite(&self, id: i64, favorite: bool) -> Result<(), AppError> {
        let result = sqlx::query("UPDATE clipboard_items SET favorite = ? WHERE id = ?")
            .bind(i64::from(favorite))
            .bind(id)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(AppError::InvalidInput(NOT_FOUND.into()));
        }
        Ok(())
    }

    async fn fetch_row(&self, id: i64) -> Result<ItemRow, AppError> {
        sqlx::query_as::<_, ItemRow>("SELECT * FROM clipboard_items WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::InvalidInput(NOT_FOUND.into()))
    }

    /// 行 → 列表 DTO：文本算预览、图片拼绝对路径、文件逐个查存在性。不碰 sqlx，只依赖 `ItemRow` 与 `images_dir`。
    fn to_item(&self, row: ItemRow) -> Result<ClipboardItem, AppError> {
        let ItemRow {
            id,
            kind,
            text,
            image_file,
            image_width,
            image_height,
            files,
            size,
            favorite,
            copied_at,
            ..
        } = row;
        let size = size as u64;
        match kind {
            ClipboardKind::Text => {
                let (preview, char_count, truncated) = preview_of(&text.unwrap_or_default());
                Ok(ClipboardItem::Text {
                    id,
                    favorite,
                    copied_at,
                    size,
                    preview,
                    char_count,
                    truncated,
                })
            }
            ClipboardKind::Image => {
                let (image_path, thumb_path) = self.image_paths(&image_file.unwrap_or_default());
                Ok(ClipboardItem::Image {
                    id,
                    favorite,
                    copied_at,
                    size,
                    image_path: image_path.to_string_lossy().into_owned(),
                    thumb_path: thumb_path.to_string_lossy().into_owned(),
                    width: image_width.unwrap_or_default() as u32,
                    height: image_height.unwrap_or_default() as u32,
                })
            }
            ClipboardKind::Files => {
                let files = files
                    .unwrap_or_default()
                    .0
                    .into_iter()
                    .map(|path| ClipboardFile {
                        name: file_name_of(&path),
                        // 同步 exists() 只针对本地路径设计；断开的 UNC / 网络盘路径可能阻塞数秒，已知风险，本版不处理
                        exists: Path::new(&path).exists(),
                        path,
                    })
                    .collect();
                Ok(ClipboardItem::Files {
                    id,
                    favorite,
                    copied_at,
                    files,
                })
            }
        }
    }
}

/// 图片文件：`images_dir/<hash>.png` 原图 + `<hash>.thumb.png` 缩略图。库里只存原图文件名，缩略图名由它推导。
impl ClipboardStore {
    /// 把图片快照的原图与缩略图写进 `images_dir`；非图片快照直接返回。
    /// 同一像素内容再次复制时两个文件已存在，跳过重写。同步阻塞，调用方负责放 blocking 线程。
    pub fn save_image(&self, captured: &Captured) -> Result<(), AppError> {
        let Captured::Image {
            png,
            thumb_png,
            rgba_hash,
            ..
        } = captured
        else {
            return Ok(());
        };
        let (image_path, thumb_path) = self.image_paths(&image_file_name(rgba_hash));
        if image_path.exists() && thumb_path.exists() {
            return Ok(());
        }
        write_atomically(&image_path, png)?;
        write_atomically(&thumb_path, thumb_png)?;
        Ok(())
    }

    /// 删除一组图片条目对应的原图与缩略图（传入 `trim` / `delete` 返回的文件名）；
    /// 文件不存在视为成功，其它失败只 warn（行已删，文件残留无害）。
    pub fn remove_image_files(&self, image_files: &[String]) {
        for name in image_files {
            let (image_path, thumb_path) = self.image_paths(name);
            for path in [image_path, thumb_path] {
                if let Err(e) = std::fs::remove_file(&path) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        log::warn!("删除剪贴板图片文件 {} 失败: {e}", path.display());
                    }
                }
            }
        }
    }

    /// 原图文件名 → (原图绝对路径, 缩略图绝对路径)
    fn image_paths(&self, image_file: &str) -> (PathBuf, PathBuf) {
        (
            self.images_dir.join(image_file),
            self.images_dir.join(thumb_file_name(image_file)),
        )
    }
}

/// 图片原图文件名 `<hash>.png`（存进 `image_file` 列的就是它）
fn image_file_name(hash: &str) -> String {
    format!("{hash}.png")
}

/// 由原图文件名推导缩略图文件名 `<hash>.thumb.png`
fn thumb_file_name(image_file: &str) -> String {
    let stem = image_file.strip_suffix(".png").unwrap_or(image_file);
    format!("{stem}.thumb.png")
}

/// 先写 `<path>.tmp` 再重命名覆盖：写入中途崩溃不会留下半个文件被 `exists()` 当成完整图片。
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 把关键字转成 `LIKE ... ESCAPE '\'` 的模式：先转义 `\` / `%` / `_`，再两侧加 `%` 做包含匹配。
fn like_pattern(keyword: &str) -> String {
    let mut escaped = String::with_capacity(keyword.len() + 2);
    escaped.push('%');
    for ch in keyword.chars() {
        if matches!(ch, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('%');
    escaped
}

/// 列表预览：按 char 截断到 `PREVIEW_CHARS` 再 trim；返回 (preview, 全文 char 数, 是否截断)。
fn preview_of(text: &str) -> (String, u64, bool) {
    let char_count = text.chars().count();
    let truncated = char_count > PREVIEW_CHARS;
    let preview: String = text.chars().take(PREVIEW_CHARS).collect();
    (preview.trim().to_owned(), char_count as u64, truncated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ListCursor;

    /// 内存库：连接数固定为 1 且永不回收，否则每个连接各有一份独立的 `:memory:` 数据
    async fn memory_store() -> ClipboardStore {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect("sqlite::memory:")
            .await
            .expect("内存库应能连接");
        MIGRATOR.run(&pool).await.expect("迁移应成功");
        ClipboardStore {
            pool,
            images_dir: PathBuf::from("/images"),
        }
    }

    fn text(s: &str) -> Captured {
        Captured::Text(s.into())
    }

    fn image(hash: &str) -> Captured {
        Captured::Image {
            png: vec![0; 10],
            thumb_png: vec![],
            width: 4,
            height: 3,
            rgba_hash: hash.into(),
        }
    }

    fn files(paths: &[&str]) -> Captured {
        Captured::Files(paths.iter().map(|p| (*p).to_owned()).collect())
    }

    fn query(limit: u32) -> ListQuery {
        ListQuery {
            limit,
            ..ListQuery::default()
        }
    }

    fn id_of(item: &ClipboardItem) -> i64 {
        match item {
            ClipboardItem::Text { id, .. }
            | ClipboardItem::Image { id, .. }
            | ClipboardItem::Files { id, .. } => *id,
        }
    }

    fn copied_at_of(item: &ClipboardItem) -> i64 {
        match item {
            ClipboardItem::Text { copied_at, .. }
            | ClipboardItem::Image { copied_at, .. }
            | ClipboardItem::Files { copied_at, .. } => *copied_at,
        }
    }

    fn favorite_of(item: &ClipboardItem) -> bool {
        match item {
            ClipboardItem::Text { favorite, .. }
            | ClipboardItem::Image { favorite, .. }
            | ClipboardItem::Files { favorite, .. } => *favorite,
        }
    }

    async fn all(store: &ClipboardStore) -> Vec<ClipboardItem> {
        store.list(&query(200)).await.expect("列表应成功")
    }

    async fn count(store: &ClipboardStore, favorite: bool) -> i64 {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM clipboard_items WHERE favorite = ?")
            .bind(i64::from(favorite))
            .fetch_one(&store.pool)
            .await
            .expect("计数应成功")
    }

    #[tokio::test]
    async fn upsert_same_content_bumps_copied_at_without_new_row() {
        let store = memory_store().await;
        store.upsert(&text("hello"), 1).await.expect("upsert");
        store.upsert(&text("other"), 2).await.expect("upsert");
        store.upsert(&text("hello"), 3).await.expect("upsert");
        let items = all(&store).await;
        assert_eq!(items.len(), 2);
        assert_eq!(copied_at_of(&items[0]), 3);
        assert!(matches!(&items[0], ClipboardItem::Text { preview, .. } if preview == "hello"));
        // created_at 保持首次时间
        let created =
            sqlx::query_scalar::<_, i64>("SELECT created_at FROM clipboard_items WHERE id = ?")
                .bind(id_of(&items[0]))
                .fetch_one(&store.pool)
                .await
                .expect("查询");
        assert_eq!(created, 1);
    }

    #[tokio::test]
    async fn trim_evicts_oldest_non_favorites_and_keeps_favorites() {
        let store = memory_store().await;
        // 三条最旧的收藏，其中一条是图片
        store.upsert(&text("fav-1"), 1).await.expect("upsert");
        store.upsert(&image("fav-img"), 2).await.expect("upsert");
        store.upsert(&text("fav-3"), 3).await.expect("upsert");
        for item in all(&store).await {
            store
                .set_favorite(id_of(&item), true)
                .await
                .expect("set_favorite");
        }
        // 两条最旧的非收藏（一条图片）会被淘汰
        store.upsert(&image("old-img"), 10).await.expect("upsert");
        store.upsert(&text("old-text"), 11).await.expect("upsert");
        for i in 0..MAX_ITEMS {
            store
                .upsert(&text(&format!("item-{i}")), 100 + i)
                .await
                .expect("upsert");
        }
        let evicted = store.trim().await.expect("trim");
        assert_eq!(evicted, vec!["old-img.png".to_owned()]);
        assert_eq!(count(&store, false).await, MAX_ITEMS);
        assert_eq!(count(&store, true).await, 3);
        let remaining = store
            .list(&ListQuery {
                query: "old".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert!(remaining.is_empty(), "最旧的非收藏条目应被淘汰");
        // 再 trim 一次不再删任何东西
        assert!(store.trim().await.expect("trim").is_empty());
    }

    #[tokio::test]
    async fn list_filters_by_kind_favorite_and_query_together() {
        let store = memory_store().await;
        store.upsert(&text("apple pie"), 1).await.expect("upsert");
        store.upsert(&text("apple juice"), 2).await.expect("upsert");
        store.upsert(&text("banana"), 3).await.expect("upsert");
        store
            .upsert(&files(&["/x/apple.txt"]), 4)
            .await
            .expect("upsert");
        store.upsert(&image("img"), 5).await.expect("upsert");
        let items = all(&store).await;
        assert_eq!(items.len(), 5);
        // 收藏 "apple pie"（最旧）与 files
        store
            .set_favorite(id_of(&items[4]), true)
            .await
            .expect("fav");
        store
            .set_favorite(id_of(&items[1]), true)
            .await
            .expect("fav");

        // 只 kind
        let texts = store
            .list(&ListQuery {
                kind: Some(ClipboardKind::Text),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(texts.len(), 3);
        // kind + query
        let apples = store
            .list(&ListQuery {
                kind: Some(ClipboardKind::Text),
                query: "apple".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(apples.len(), 2);
        // kind + favorite + query → 只剩 "apple pie"
        let fav_apples = store
            .list(&ListQuery {
                kind: Some(ClipboardKind::Text),
                favorite_only: true,
                query: "apple".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(fav_apples.len(), 1);
        assert!(
            matches!(&fav_apples[0], ClipboardItem::Text { preview, .. } if preview == "apple pie")
        );
        // favorite + query 跨类型：files 的文件名也命中
        let fav_any = store
            .list(&ListQuery {
                favorite_only: true,
                query: "apple".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(fav_any.len(), 2);
        // 有关键字时图片 Tab 为空
        let images = store
            .list(&ListQuery {
                kind: Some(ClipboardKind::Image),
                query: "a".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert!(images.is_empty());
    }

    #[tokio::test]
    async fn ordering_ignores_favorite_and_breaks_ties_by_id_desc() {
        let store = memory_store().await;
        store.upsert(&text("first"), 5).await.expect("upsert");
        store.upsert(&text("second"), 5).await.expect("upsert");
        store.upsert(&text("newest"), 9).await.expect("upsert");
        let items = all(&store).await;
        let previews: Vec<String> = items
            .iter()
            .map(|i| match i {
                ClipboardItem::Text { preview, .. } => preview.clone(),
                _ => unreachable!(),
            })
            .collect();
        // 同 copied_at 时后录入者（id 更大）在前
        assert_eq!(previews, vec!["newest", "second", "first"]);
        // 收藏最旧的一条不改变顺序
        store
            .set_favorite(id_of(&items[2]), true)
            .await
            .expect("fav");
        let after = all(&store).await;
        assert_eq!(
            after.iter().map(id_of).collect::<Vec<_>>(),
            items.iter().map(id_of).collect::<Vec<_>>()
        );
        assert!(favorite_of(&after[2]));
    }

    #[tokio::test]
    async fn cursor_pagination_survives_new_items_inserted_between_pages() {
        let store = memory_store().await;
        for i in 1..=10 {
            store
                .upsert(&text(&format!("t{i}")), i)
                .await
                .expect("upsert");
        }
        let page1 = store.list(&query(4)).await.expect("list");
        assert_eq!(page1.len(), 4);
        let mut seen: Vec<i64> = page1.iter().map(id_of).collect();
        // 用户滚动期间新复制了一条
        store.upsert(&text("brand-new"), 99).await.expect("upsert");
        let mut last = page1.last().expect("非空");
        let mut cursor = ListCursor {
            copied_at: copied_at_of(last),
            id: id_of(last),
        };
        let page2 = store
            .list(&ListQuery {
                before: Some(cursor),
                ..query(4)
            })
            .await
            .expect("list");
        assert_eq!(page2.len(), 4);
        seen.extend(page2.iter().map(id_of));
        last = page2.last().expect("非空");
        cursor = ListCursor {
            copied_at: copied_at_of(last),
            id: id_of(last),
        };
        let page3 = store
            .list(&ListQuery {
                before: Some(cursor),
                ..query(4)
            })
            .await
            .expect("list");
        assert_eq!(page3.len(), 2, "第三页只剩最后两条，说明到底");
        seen.extend(page3.iter().map(id_of));
        // 原有 10 条全部出现且各一次；新条目不在任何追加页里（刷新首页时才出现）
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 10);
        assert_eq!(seen.len(), 10);
    }

    #[tokio::test]
    async fn cursor_pagination_with_equal_copied_at_uses_id() {
        let store = memory_store().await;
        for i in 0..5 {
            store
                .upsert(&text(&format!("same-{i}")), 7)
                .await
                .expect("upsert");
        }
        let page1 = store.list(&query(2)).await.expect("list");
        let last = page1.last().expect("非空");
        let page2 = store
            .list(&ListQuery {
                before: Some(ListCursor {
                    copied_at: copied_at_of(last),
                    id: id_of(last),
                }),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(page2.len(), 3);
        assert!(page2.iter().all(|i| id_of(i) < id_of(last)));
    }

    #[tokio::test]
    async fn file_search_matches_file_name_but_not_directory() {
        let store = memory_store().await;
        store
            .upsert(
                &files(&[r"C:\Projects\report.pdf", "/home/me/photos/cat.png"]),
                1,
            )
            .await
            .expect("upsert");
        let hit = store
            .list(&ListQuery {
                query: "report".into(),
                ..query(10)
            })
            .await
            .expect("list");
        assert_eq!(hit.len(), 1);
        match &hit[0] {
            ClipboardItem::Files { files, .. } => {
                assert_eq!(files.len(), 2);
                assert_eq!(files[0].name, "report.pdf");
                assert_eq!(files[0].path, r"C:\Projects\report.pdf");
                assert_eq!(files[1].name, "cat.png");
                assert!(!files[0].exists && !files[1].exists);
            }
            other => panic!("应为文件条目，得到 {other:?}"),
        }
        for dir_word in ["Projects", "photos", "home"] {
            let miss = store
                .list(&ListQuery {
                    query: dir_word.into(),
                    ..query(10)
                })
                .await
                .expect("list");
            assert!(miss.is_empty(), "目录名 {dir_word} 不应命中");
        }
    }

    #[tokio::test]
    async fn image_never_matches_keyword_but_lists_without_one() {
        let store = memory_store().await;
        store.upsert(&image("abc123"), 1).await.expect("upsert");
        for keyword in ["a", "abc123", "png", "%", "4"] {
            let hit = store
                .list(&ListQuery {
                    query: keyword.into(),
                    ..query(10)
                })
                .await
                .expect("list");
            assert!(hit.is_empty(), "关键字 {keyword} 不应命中图片");
        }
        let items = all(&store).await;
        match &items[0] {
            ClipboardItem::Image {
                image_path,
                thumb_path,
                width,
                height,
                size,
                ..
            } => {
                assert!(image_path.ends_with("abc123.png"));
                assert!(thumb_path.ends_with("abc123.thumb.png"));
                assert_eq!((*width, *height, *size), (4, 3, 10));
            }
            other => panic!("应为图片条目，得到 {other:?}"),
        }
    }

    #[tokio::test]
    async fn like_wildcards_in_keyword_are_escaped() {
        let store = memory_store().await;
        store.upsert(&text("100%"), 1).await.expect("upsert");
        store.upsert(&text("100x"), 2).await.expect("upsert");
        store.upsert(&text("a_b"), 3).await.expect("upsert");
        store.upsert(&text("aXb"), 4).await.expect("upsert");
        store.upsert(&text(r"back\slash"), 5).await.expect("upsert");
        store.upsert(&text("backslash"), 6).await.expect("upsert");
        let cases: [(&str, &[&str]); 3] =
            [("%", &["100%"]), ("_", &["a_b"]), (r"\", &[r"back\slash"])];
        for (keyword, expected) in cases {
            let hit = store
                .list(&ListQuery {
                    query: keyword.into(),
                    ..query(10)
                })
                .await
                .expect("list");
            let previews: Vec<String> = hit
                .iter()
                .map(|i| match i {
                    ClipboardItem::Text { preview, .. } => preview.clone(),
                    _ => unreachable!(),
                })
                .collect();
            assert_eq!(previews, expected, "关键字 {keyword}");
        }
        assert_eq!(like_pattern(r"a%b_c\d"), r"%a\%b\_c\\d%");
    }

    #[tokio::test]
    async fn set_favorite_keeps_copied_at_and_rejects_missing_id() {
        let store = memory_store().await;
        store.upsert(&text("x"), 42).await.expect("upsert");
        let id = id_of(&all(&store).await[0]);
        store.set_favorite(id, true).await.expect("fav");
        let item = &all(&store).await[0];
        assert!(favorite_of(item));
        assert_eq!(copied_at_of(item), 42);
        store.set_favorite(id, false).await.expect("unfav");
        assert!(!favorite_of(&all(&store).await[0]));
        let err = store
            .set_favorite(9999, true)
            .await
            .expect_err("不存在的 id 应报错");
        assert_eq!(err.to_string(), "参数错误: 记录不存在");
    }

    #[tokio::test]
    async fn delete_returns_image_file_and_rejects_missing_id() {
        let store = memory_store().await;
        store.upsert(&image("img"), 1).await.expect("upsert");
        store.upsert(&text("t"), 2).await.expect("upsert");
        let items = all(&store).await;
        assert_eq!(store.delete(id_of(&items[0])).await.expect("delete"), None);
        assert_eq!(
            store.delete(id_of(&items[1])).await.expect("delete"),
            Some("img.png".to_owned())
        );
        assert!(all(&store).await.is_empty());
        let err = store.delete(1).await.expect_err("已删除的 id 应报错");
        assert_eq!(err.to_string(), "参数错误: 记录不存在");
    }

    #[tokio::test]
    async fn get_text_returns_full_untrimmed_text_and_rejects_non_text() {
        let store = memory_store().await;
        let long = format!("  {}  \n", "字".repeat(400));
        store.upsert(&text(&long), 1).await.expect("upsert");
        store.upsert(&image("img"), 2).await.expect("upsert");
        store
            .upsert(&files(&["/a/b.txt"]), 3)
            .await
            .expect("upsert");
        let items = all(&store).await;
        // items[2] 是文本
        let full = store.get_text(id_of(&items[2])).await.expect("get_text");
        assert_eq!(full, long);
        for non_text in &items[..2] {
            let err = store
                .get_text(id_of(non_text))
                .await
                .expect_err("非文本应报错");
            assert_eq!(err.to_string(), "参数错误: 该条目不是文本");
        }
        let err = store.get_text(9999).await.expect_err("不存在应报错");
        assert_eq!(err.to_string(), "参数错误: 记录不存在");
    }

    #[tokio::test]
    async fn preview_is_truncated_and_trimmed() {
        let store = memory_store().await;
        let long = format!("  {}", "x".repeat(400));
        store.upsert(&text(&long), 1).await.expect("upsert");
        store
            .upsert(&text("  short\nline  "), 2)
            .await
            .expect("upsert");
        let items = all(&store).await;
        match &items[0] {
            ClipboardItem::Text {
                preview,
                char_count,
                truncated,
                size,
                ..
            } => {
                assert_eq!(preview, "short\nline");
                assert_eq!(*char_count, 14);
                assert!(!truncated);
                assert_eq!(*size, 14);
            }
            other => panic!("应为文本，得到 {other:?}"),
        }
        match &items[1] {
            ClipboardItem::Text {
                preview,
                char_count,
                truncated,
                ..
            } => {
                assert_eq!(
                    preview.chars().count(),
                    PREVIEW_CHARS - 2,
                    "截断后再 trim 去掉前导空格"
                );
                assert_eq!(*char_count, 402);
                assert!(truncated);
            }
            other => panic!("应为文本，得到 {other:?}"),
        }
        let (p, n, t) = preview_of("é".repeat(PREVIEW_CHARS).as_str());
        assert_eq!(
            (p.chars().count(), n, t),
            (PREVIEW_CHARS, PREVIEW_CHARS as u64, false)
        );
    }

    #[test]
    fn image_file_names_are_derived_from_hash() {
        assert_eq!(image_file_name("abc"), "abc.png");
        assert_eq!(thumb_file_name("abc.png"), "abc.thumb.png");
    }

    #[tokio::test]
    async fn get_captured_round_trips_text_files_and_image() {
        let dir =
            std::env::temp_dir().join(format!("z-tools-clipboard-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        let store = ClipboardStore {
            images_dir: dir.clone(),
            ..memory_store().await
        };
        // save_image 落盘原图 + 缩略图，不留 .tmp；非图片快照是 no-op
        let img = Captured::Image {
            png: vec![0; 10],
            thumb_png: vec![7; 3],
            width: 4,
            height: 3,
            rgba_hash: "roundtrip".into(),
        };
        store.save_image(&img).expect("save_image");
        store.save_image(&text("hi")).expect("非图片应直接返回");
        assert_eq!(
            std::fs::read(dir.join("roundtrip.png")).expect("原图应存在"),
            vec![0; 10]
        );
        assert_eq!(
            std::fs::read(dir.join("roundtrip.thumb.png")).expect("缩略图应存在"),
            vec![7; 3]
        );
        assert!(!dir.join("roundtrip.png.tmp").exists());
        // 回读时缩略图不读，与 upsert 时的快照比较要用空 thumb
        let img = image("roundtrip");
        store.upsert(&text("hi"), 1).await.expect("upsert");
        store
            .upsert(&files(&["/a", "/b"]), 2)
            .await
            .expect("upsert");
        store.upsert(&img, 3).await.expect("upsert");
        let items = all(&store).await;
        assert_eq!(
            store.get_captured(id_of(&items[0])).await.expect("image"),
            img.clone()
        );
        assert_eq!(
            store.get_captured(id_of(&items[1])).await.expect("files"),
            files(&["/a", "/b"])
        );
        assert_eq!(
            store.get_captured(id_of(&items[2])).await.expect("text"),
            text("hi")
        );
        let err = store.get_captured(9999).await.expect_err("不存在应报错");
        assert_eq!(err.to_string(), "参数错误: 记录不存在");
        // remove_image_files 两个文件一起删；不存在的文件名不报错
        store.remove_image_files(&["roundtrip.png".to_owned(), "missing.png".to_owned()]);
        assert!(!dir.join("roundtrip.png").exists());
        assert!(!dir.join("roundtrip.thumb.png").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
