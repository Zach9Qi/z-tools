//! 剪贴板历史领域层：常量、IPC DTO、内部快照类型 `Captured`、哈希 / 可搜索文本等纯函数，
//! 以及编排用例 `record()`（监听器录入）、`paste()`（写回 + 切前台 + 模拟 Ctrl+V）、`delete_item`。
//!
//! 边界：
//! - 历史存储（`ClipboardStore`：SQLite 行 + `images/` 下的图片文件）整体在 `clipboard/store.rs`，这里只 `pub use`；
//!   剪贴板读写（arboard，跨平台）在 `clipboard/backend.rs`；监听消息窗口与 `SendInput` 在 `clipboard/windows.rs`（仅 Windows 编译）。
//! - 本模块不处理 IPC 参数校验（命令层的事）；`clipboard://changed` 事件**只**由这里的 `record()` 发出，
//!   命令层的删除 / 收藏由前端自己更新本地列表，不再广播。
//!
//! 可见性：本模块与 `backend` / `store` 子模块都是 `pub`（同 `pub mod error` 先例）——跨平台的读写 / 存储层是 crate 的公开契约，
//! 监听与粘贴按平台接入；若为私有，非 Windows 下 `record` / `backend::*` 等只被 `windows.rs` 引用的项会被 dead_code 在 `-D warnings` 下拦下。

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Runtime};

use crate::error::AppError;

pub mod backend;
pub mod store;
#[cfg(windows)]
mod windows;

pub use store::ClipboardStore;
#[cfg(windows)]
pub use windows::{ClipboardWatcher, run_monitor, stop_monitor};

/// 非收藏条目上限，超出按最近复制时间淘汰（收藏不计入、不淘汰）
pub const MAX_ITEMS: i64 = 500;
/// 文本超过此字节数不记录（UTF-8 字节）
pub const MAX_TEXT_BYTES: usize = 1024 * 1024;
/// 图片解码后 RGBA8 字节数（宽 × 高 × 4）超过此值不记录，约等于 2300×2300 图
pub const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
/// 缩略图最长边（像素）
pub const THUMB_MAX_EDGE: u32 = 256;
/// 列表文本预览按 char 截断长度；超出即 `truncated = true`，全文经 `get_clipboard_text` 按需拉
pub const PREVIEW_CHARS: usize = 300;
/// 监听器录入新内容或上浮旧内容后广播；前端 `src/lib/events.ts` 的 `EVENTS.CLIPBOARD_CHANGED` 与此一一对应，无 payload
pub const CLIPBOARD_CHANGED: &str = "clipboard://changed";

/// 条目类型；与 TS `ClipboardKind = "text" | "image" | "files"` 镜像，也是 `kind` 列的取值。
/// `sqlx::Type` 让它直接作为列值绑定 / 解码（小写），库里有 CHECK 约束保证只有这三种值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[serde(rename_all = "camelCase")]
#[sqlx(rename_all = "lowercase")]
pub enum ClipboardKind {
    /// 纯文本（`CF_UNICODETEXT`）
    Text,
    /// 位图，落盘为 PNG 原图 + 缩略图
    Image,
    /// 文件路径列表（`CF_HDROP`），只记路径不复制文件本体
    Files,
}

impl ClipboardKind {
    /// `kind` 列存的字符串，同时也是 hash 的类型域前缀
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Files => "files",
        }
    }
}

/// 列表 DTO（出参）；`kind` 作判别字段，与 TS 判别联合 `ClipboardItem` 镜像（`src/types/clipboard.ts`）。
///
/// 全文**不**进列表：文本只带 `preview`，展开时经 `get_clipboard_text` 拉；图片带绝对路径由前端转 `asset://`。
// rename_all 只作用于变体名（"text" / "image" / "files"），字段名要靠 rename_all_fields 转 camelCase
#[derive(Debug, Clone, serde::Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum ClipboardItem {
    /// 文本条目；`truncated` = 全文 char 数是否超过 `PREVIEW_CHARS`，前端据此决定是否显示展开 chevron
    Text {
        id: i64,
        favorite: bool,
        copied_at: i64,
        size: u64,
        preview: String,
        char_count: u64,
        truncated: bool,
    },
    /// 图片条目；`image_path` / `thumb_path` 为 `images/` 下的绝对路径
    Image {
        id: i64,
        favorite: bool,
        copied_at: i64,
        size: u64,
        image_path: String,
        thumb_path: String,
        width: u32,
        height: u32,
    },
    /// 文件条目自带全部路径与逐个存在性，展开态不需要再请求后端
    Files {
        id: i64,
        favorite: bool,
        copied_at: i64,
        files: Vec<ClipboardFile>,
    },
}

/// 文件条目中的一项；`name` 由 `path` 取 `file_name()` 推导，`exists` 在 list 时逐个 `Path::exists`
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardFile {
    /// 完整路径（原样，用于粘贴写回）
    pub path: String,
    /// 文件名含扩展名；取不到（如根目录）时退回整个路径
    pub name: String,
    /// list 时该路径是否仍存在；不存在的条目保留但前端标灰
    pub exists: bool,
}

/// 列表筛选（入参）：三个维度可任意叠加（kind × favorite_only × query）+ 游标分页。
/// `default` 让老前端少传字段时不报错。
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ListQuery {
    /// 分类 Tab；`None` = 全部
    pub kind: Option<ClipboardKind>,
    /// 只看收藏
    pub favorite_only: bool,
    /// 关键字（对 `text` 列 LIKE；空字串 = 不过滤）
    pub query: String,
    /// 游标：上一页最后一条的 (copied_at, id)；`None` = 从头拉
    pub before: Option<ListCursor>,
    /// 每页条数；命令层校验 1..=200
    pub limit: u32,
}

/// 游标分页的锚点；用 `(copied_at, id)` 行值比较而非 OFFSET，避免滚动期间新条目插入导致重复 / 漏项
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListCursor {
    /// 上一页末条的 `copied_at`
    pub copied_at: i64,
    /// 上一页末条的 `id`
    pub id: i64,
}

/// 从剪贴板读到 / 写回剪贴板的内部快照；不序列化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Captured {
    /// 文本全文（已确认 trim 后非空且未超 `MAX_TEXT_BYTES`）
    Text(String),
    /// 图片：PNG 原图 + 缩略图 + 尺寸 + 像素哈希。
    /// `thumb_png` 只在采集路径有值；从库里回读用于粘贴时为空（写回只需要原图）。
    Image {
        png: Vec<u8>,
        thumb_png: Vec<u8>,
        width: u32,
        height: u32,
        rgba_hash: String,
    },
    /// 文件完整路径列表（非空）
    Files(Vec<String>),
}

impl Captured {
    /// 对应的条目类型
    pub fn kind(&self) -> ClipboardKind {
        match self {
            Self::Text(_) => ClipboardKind::Text,
            Self::Image { .. } => ClipboardKind::Image,
            Self::Files(_) => ClipboardKind::Files,
        }
    }

    /// 去重哈希（blake3 hex）：text = 全文字节；image = 解码像素哈希（由 backend 计算）；files = 路径以 `\n` 拼接。
    /// 三种都以类型名作域前缀（见 [`domain_hash`]），否则文本 `D:\a.txt` 与单文件列表 `[D:\a.txt]` 会碰撞，后者永远录不进去。
    pub fn hash(&self) -> String {
        match self {
            Self::Text(text) => domain_hash(ClipboardKind::Text, &[text.as_bytes()]),
            Self::Image { rgba_hash, .. } => rgba_hash.clone(),
            Self::Files(paths) => domain_hash(ClipboardKind::Files, &[paths.join("\n").as_bytes()]),
        }
    }
}

/// 带类型域的 blake3：先喂入 `"<kind>\0"` 再依次喂入 `parts`，输出 hex。
/// `hash` 列 UNIQUE 跨三种类型，域前缀保证不同类型的同字节内容不会被当成同一条。backend 的像素哈希也走这里。
pub fn domain_hash(kind: ClipboardKind, parts: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update(b"\0");
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

/// `text` 列的唯一产出点：该条目「可搜索 / 可展示的文本」。
///
/// text → 全文；files → 各文件名（不含目录，否则搜文件夹名会误命中）以 `\n` 拼接；
/// image → `None`（剪贴板图片无名，将来 OCR 结果填此列即可被搜到）。
pub fn searchable_text(captured: &Captured) -> Option<String> {
    match captured {
        Captured::Text(text) => Some(text.clone()),
        Captured::Files(paths) => Some(
            paths
                .iter()
                .map(|p| file_name_of(p))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Captured::Image { .. } => None,
    }
}

/// 从完整路径取文件名（含扩展名）；取不到时（根目录、空串）退回原路径，保证前端总有可显示的名字。
pub fn file_name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// 监听器录入：图片先落盘 → upsert（重复内容上浮）→ 淘汰超额条目并删其图片 → 广播 `clipboard://changed`。
///
/// 失败只记日志：监听器没有调用方可以处理错误，漏记一条不影响后续采集。
pub async fn record<R: Runtime>(app: &AppHandle<R>, store: &ClipboardStore, captured: Captured) {
    if let Err(e) = record_inner(store, &captured).await {
        log::warn!("记录剪贴板内容失败: {e}");
        return;
    }
    if let Err(e) = app.emit(CLIPBOARD_CHANGED, ()) {
        log::warn!("发送 {CLIPBOARD_CHANGED} 事件失败: {e}");
    }
}

async fn record_inner(store: &ClipboardStore, captured: &Captured) -> Result<(), AppError> {
    if matches!(captured, Captured::Image { .. }) {
        // 文件 IO 放 blocking 线程；store 是 Arc 级句柄，captured 里的 PNG 字节需要拷一份进线程
        let (store, captured) = (store.clone(), captured.clone());
        tauri::async_runtime::spawn_blocking(move || store.save_image(&captured)).await??;
    }
    store.upsert(captured, now_ms()).await?;
    let evicted = store.trim().await?;
    store.remove_image_files(&evicted);
    Ok(())
}

/// 删除一条（含图片文件）；id 不存在 → `InvalidInput("记录不存在")`。不广播事件：发起者是前端自己。
pub async fn delete_item(store: &ClipboardStore, id: i64) -> Result<(), AppError> {
    let image_file = store.delete(id).await?;
    store.remove_image_files(&Vec::from_iter(image_file));
    Ok(())
}

/// 粘贴编排（Windows）：写回剪贴板 → 激活唤出前的前台窗口 → 收起面板 → 模拟 Ctrl+V。
///
/// 兜底：没有前台记录 / 窗口已关闭 / 激活失败时只写回并收起，仍返回 `Ok`（内容已在剪贴板，用户手动粘贴即可）。
/// 写回后监听器会再捕获同一内容并上浮，是期望行为。
#[cfg(windows)]
pub async fn paste<R: Runtime>(
    app: &AppHandle<R>,
    store: &ClipboardStore,
    id: i64,
) -> Result<(), AppError> {
    let captured = store.get_captured(id).await?;
    // arboard::Clipboard 非 Send，在 blocking 线程的同步块内用完
    tauri::async_runtime::spawn_blocking(move || backend::write(&captured)).await??;

    // 必须在 hide 之前切前台：hide 后本进程可能失去前台进程资格，SetForegroundWindow 会静默失败
    let activated = crate::launcher::previous_foreground(app).is_some_and(|hwnd| {
        let ok = crate::launcher::activate_window(hwnd);
        if !ok {
            log::debug!("原前台窗口已关闭或无法激活，只复制不粘贴");
        }
        ok
    });
    crate::launcher::hide(app);

    if activated {
        // 等目标窗口真正拿到焦点再按键；失败只 warn（内容已在剪贴板）
        tauri::async_runtime::spawn_blocking(|| {
            std::thread::sleep(std::time::Duration::from_millis(50));
            if let Err(e) = windows::send_paste() {
                log::warn!("模拟 Ctrl+V 失败: {e}");
            }
        });
    }
    Ok(())
}

/// 非 Windows 平台尚未实现监听与粘贴模拟，明确报错而不是静默 no-op。
#[cfg(not(windows))]
pub async fn paste<R: Runtime>(
    _app: &AppHandle<R>,
    _store: &ClipboardStore,
    _id: i64,
) -> Result<(), AppError> {
    Err(AppError::Unsupported("剪贴板粘贴".into()))
}

/// 当前 unix 毫秒；系统时钟早于 1970 时退化为 0（不 panic）
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(hash: &str) -> Captured {
        Captured::Image {
            png: vec![1, 2, 3],
            thumb_png: vec![],
            width: 2,
            height: 2,
            rgba_hash: hash.into(),
        }
    }

    #[test]
    fn searchable_text_of_text_is_full_content() {
        let captured = Captured::Text("  hello\nworld  ".into());
        assert_eq!(
            searchable_text(&captured).as_deref(),
            Some("  hello\nworld  ")
        );
    }

    #[test]
    fn searchable_text_of_files_joins_file_names_only() {
        let captured = Captured::Files(vec![
            r"C:\Users\me\Documents\report.pdf".into(),
            "/home/me/photos/cat.png".into(),
        ]);
        assert_eq!(
            searchable_text(&captured).as_deref(),
            Some("report.pdf\ncat.png")
        );
    }

    #[test]
    fn searchable_text_of_image_is_none() {
        assert_eq!(searchable_text(&image("abc")), None);
    }

    #[test]
    fn hash_is_stable_per_kind() {
        let a = Captured::Text("same".into());
        let b = Captured::Text("same".into());
        assert_eq!(a.hash(), b.hash());
        assert_ne!(a.hash(), Captured::Text("other".into()).hash());
        // 图片直接沿用 backend 算出的像素哈希
        assert_eq!(image("deadbeef").hash(), "deadbeef");
        // 文件列表按路径顺序敏感
        let f1 = Captured::Files(vec!["a".into(), "b".into()]);
        let f2 = Captured::Files(vec!["b".into(), "a".into()]);
        assert_ne!(f1.hash(), f2.hash());
        // 同字节内容不同类型不碰撞：文本 "a" vs 单文件列表 ["a"]，文本 "a\nb" vs 文件列表 ["a","b"]
        assert_ne!(
            Captured::Text("a".into()).hash(),
            Captured::Files(vec!["a".into()]).hash()
        );
        assert_ne!(Captured::Text("a\nb".into()).hash(), f1.hash());
        // 域前缀进入哈希：与裸 blake3 不同
        assert_ne!(
            Captured::Text("a".into()).hash(),
            blake3::hash(b"a").to_hex().to_string()
        );
        assert_eq!(domain_hash(ClipboardKind::Text, &[b"a"]).len(), 64);
    }

    #[test]
    fn file_name_falls_back_to_path_when_missing() {
        assert_eq!(file_name_of(r"C:\dir\a.txt"), "a.txt");
        assert_eq!(file_name_of("/"), "/");
        assert_eq!(file_name_of(""), "");
    }

    #[test]
    fn list_query_defaults_and_camel_case() {
        let q: ListQuery = serde_json::from_str(r#"{"favoriteOnly":true,"limit":10}"#)
            .expect("ListQuery 应接受缺省字段");
        assert!(q.favorite_only);
        assert_eq!(q.limit, 10);
        assert_eq!(q.kind, None);
        assert!(q.before.is_none());
        let q: ListQuery =
            serde_json::from_str(r#"{"kind":"files","before":{"copiedAt":5,"id":3},"limit":1}"#)
                .expect("ListQuery 应解析游标");
        assert_eq!(q.kind, Some(ClipboardKind::Files));
        let cursor = q.before.expect("应有游标");
        assert_eq!((cursor.copied_at, cursor.id), (5, 3));
    }

    #[test]
    fn item_serializes_with_kind_tag() {
        let item = ClipboardItem::Files {
            id: 1,
            favorite: false,
            copied_at: 2,
            files: vec![ClipboardFile {
                path: "/a/b.txt".into(),
                name: "b.txt".into(),
                exists: false,
            }],
        };
        let json = serde_json::to_string(&item).expect("ClipboardItem 应能序列化");
        assert_eq!(
            json,
            r#"{"kind":"files","id":1,"favorite":false,"copiedAt":2,"files":[{"path":"/a/b.txt","name":"b.txt","exists":false}]}"#
        );
    }
}
