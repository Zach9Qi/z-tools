// 剪贴板领域类型:手写镜像 `src-tauri/src/clipboard.rs` 的 `ClipboardKind` / `ClipboardItem` /
// `ClipboardFile` / `ListQuery` / `ListCursor`(全部 `#[serde(rename_all = "camelCase")]`,
// `ClipboardItem` 以 `#[serde(tag = "kind")]` 序列化为判别联合)。
// Rust 侧改字段这里必须同步,否则前端读到 undefined 且无报错。

/** 条目类型;Rust `enum ClipboardKind { Text, Image, Files }` */
export type ClipboardKind = "text" | "image" | "files";

/** 三种条目共有的字段 */
interface ClipboardItemBase {
  /** SQLite 自增主键 */
  id: number;
  /** 收藏标记:只是筛选维度 + 免于淘汰,不影响排序 */
  favorite: boolean;
  /** 最近一次复制时间,unix ms;列表主排序键 */
  copiedAt: number;
}

/** 文本条目;全文不进列表,展开时用 `getClipboardText(id)` 按需拉 */
export interface ClipboardTextItem extends ClipboardItemBase {
  kind: "text";
  /** utf8 字节数 */
  size: number;
  /** 按 PREVIEW_CHARS(300 字)截断、首尾 trim 的预览;保留换行 */
  preview: string;
  /** 全文字符数 */
  charCount: number;
  /** preview 是否比全文短 */
  truncated: boolean;
}

/** 图片条目;两个路径都是绝对路径,前端经 `toAssetUrl()` 转成 asset:// URL */
export interface ClipboardImageItem extends ClipboardItemBase {
  kind: "image";
  /** PNG 原图字节数 */
  size: number;
  /** 原图绝对路径 */
  imagePath: string;
  /** 缩略图(最长边 256px)绝对路径 */
  thumbPath: string;
  width: number;
  height: number;
}

/** 文件列表条目;自带全部路径与逐个存在性,展开态不需要再请求后端 */
export interface ClipboardFilesItem extends ClipboardItemBase {
  kind: "files";
  files: ClipboardFile[];
}

/** 列表条目,以 `kind` 判别 */
export type ClipboardItem = ClipboardTextItem | ClipboardImageItem | ClipboardFilesItem;

/** 文件条目中的一项;`name` 由后端从 `path` 推导,`exists` 在 list 时逐个检查 */
export interface ClipboardFile {
  path: string;
  name: string;
  exists: boolean;
}

/** 游标分页:上一页最后一条的 (copiedAt, id) */
export interface ListCursor {
  copiedAt: number;
  id: number;
}

/** 列表筛选:kind × favoriteOnly × query 三维度可任意叠加;Rust 侧 `#[serde(default)]`,可选字段省略即 None */
export interface ListQuery {
  /** 不给 = 全部类型 */
  kind?: ClipboardKind;
  favoriteOnly: boolean;
  /** 关键字,空串 = 不过滤;后端按 `text` 列 LIKE */
  query: string;
  /** 不给 = 从头拉 */
  before?: ListCursor;
  /** 每页条数,后端校验 1..=200 */
  limit: number;
}
