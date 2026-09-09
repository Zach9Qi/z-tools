// 剪贴板命令封装:与 `src-tauri/src/commands/clipboard.rs` 的 5 个命令一一对应(函数名 = 命令名 camelCase)。
// 也是整个 src/ 唯一允许调用 `convertFileSrc` 的地方(`toAssetUrl`);组件拿到的已是可直接放进 <img src> 的 URL。
// 非 Tauri 运行时(bun run dev 浏览器预览)返回带「(浏览器预览)」标识的假数据,支持 kind / 收藏 / 关键字过滤与游标分页,
// 让工具页在浏览器里也能走完整交互(展开 / 收藏 / 删除)。
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/runtime";
import type {
  ClipboardFilesItem,
  ClipboardImageItem,
  ClipboardItem,
  ClipboardTextItem,
  ListQuery,
} from "@/types/clipboard";

/**
 * 拉一页剪贴板历史(kind × favoriteOnly × query 叠加过滤,`before` 游标分页,按 copiedAt / id 倒序)。
 * 非 Tauri 返回内存里的假数据(同样过滤 / 分页);失败 reject 中文文案(如 limit 越界的「参数错误: …」)。
 */
export function listClipboardItems(query: ListQuery): Promise<ClipboardItem[]> {
  if (!isTauriRuntime()) return Promise.resolve(listFakeItems(query));
  return invoke<ClipboardItem[]>("list_clipboard_items", { query });
}

/**
 * 读文本条目的全文(不 trim,原样),供展开态显示;不动 copiedAt。
 * 非 Tauri 返回一段 20 行示例文本;id 不存在 / 不是文本条目时 reject「记录不存在」/「该条目不是文本」。
 */
export function getClipboardText(id: number): Promise<string> {
  if (!isTauriRuntime()) return getFakeText(id);
  return invoke<string>("get_clipboard_text", { id });
}

/**
 * 把条目写回系统剪贴板 → 激活唤出前的前台窗口 → 收起面板 → 模拟 Ctrl+V;
 * 前台窗口不可用时只复制并收起,命令仍成功。非 Tauri 直接 resolve(浏览器没有系统剪贴板写回与窗口)。
 * 失败 reject 中文文案(非 Windows 平台为「当前平台暂不支持: …」)。
 */
export function pasteClipboardItem(id: number): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve();
  return invoke<void>("paste_clipboard_item", { id });
}

/**
 * 删除一条记录(图片条目连带删文件)。非 Tauri 从假数据里移除;id 不存在 reject「记录不存在」。
 */
export function deleteClipboardItem(id: number): Promise<void> {
  if (!isTauriRuntime())
    return mutateFake(id, (item) => fakeItems.splice(fakeItems.indexOf(item), 1));
  return invoke<void>("delete_clipboard_item", { id });
}

/**
 * 设置收藏标记;只改标记不动 copiedAt(收藏不改变位置)。非 Tauri 改假数据;id 不存在 reject「记录不存在」。
 */
export function setClipboardItemFavorite(id: number, favorite: boolean): Promise<void> {
  if (!isTauriRuntime()) {
    return mutateFake(id, (item) => {
      item.favorite = favorite;
    });
  }
  return invoke<void>("set_clipboard_item_favorite", { id, favorite });
}

/**
 * 把后端给的图片绝对路径转成 WebView 可加载的 `asset://` URL(需 tauri.conf 的 assetProtocol scope 覆盖该目录)。
 * 非 Tauri 没有 asset 协议,返回一张写着「浏览器预览」的占位 SVG data URL,让 <img> 有东西可显示。
 */
export function toAssetUrl(path: string): string {
  if (!isTauriRuntime()) return PLACEHOLDER_IMAGE_URL;
  return convertFileSrc(path);
}

// ---------------------------------------------------------------------------
// 以下全部是浏览器预览用的假数据;真实运行不会走到
// ---------------------------------------------------------------------------

/** 占位图:与真实缩略图一样是位图 URL,便于 <img> 尺寸 / object-fit 样式在预览里成立 */
const PLACEHOLDER_IMAGE_URL = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(
  '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180" viewBox="0 0 320 180">' +
    '<rect width="320" height="180" fill="lightgray"/>' +
    '<text x="160" y="96" font-family="sans-serif" font-size="20" fill="dimgray" text-anchor="middle">浏览器预览</text>' +
    "</svg>",
)}`;

/** 假数据的时间基准:相对「现在」偏移,让相对时间文案覆盖「刚刚 / N 分钟前 / N 小时前 / N 天前」 */
const NOW = Date.now();
const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** 展开态全文假数据:20 行,带缩进,验证 <pre> 保留换行与滚动 */
const FAKE_LONG_TEXT = Array.from(
  { length: 20 },
  (_, i) =>
    `第 ${i + 1} 行:这是一段用于浏览器预览的多行文本 (浏览器预览)${i % 4 === 3 ? "\n    缩进的续行" : ""}`,
).join("\n");

function fakeText(id: number, copiedAt: number, text: string, favorite = false): ClipboardTextItem {
  const chars = [...text];
  const truncated = chars.length > 300;
  return {
    kind: "text",
    id,
    favorite,
    copiedAt,
    size: new TextEncoder().encode(text).length,
    preview: (truncated ? chars.slice(0, 300).join("") : text).trim(),
    charCount: chars.length,
    truncated,
  };
}

function fakeImage(
  id: number,
  copiedAt: number,
  width: number,
  height: number,
  favorite = false,
): ClipboardImageItem {
  return {
    kind: "image",
    id,
    favorite,
    copiedAt,
    size: Math.round(width * height * 0.35),
    imagePath: `C:\\preview\\images\\${id}.png`,
    thumbPath: `C:\\preview\\images\\${id}.thumb.png`,
    width,
    height,
  };
}

function fakeFiles(
  id: number,
  copiedAt: number,
  files: Array<[path: string, exists: boolean]>,
  favorite = false,
): ClipboardFilesItem {
  return {
    kind: "files",
    id,
    favorite,
    copiedAt,
    files: files.map(([path, exists]) => ({
      path,
      name: path.slice(path.lastIndexOf("\\") + 1),
      exists,
    })),
  };
}

/** 假数据表(可变:删除 / 收藏会改它,让预览里的本地变更与「重拉」结果一致);按 copiedAt 倒序排列 */
const fakeItems: ClipboardItem[] = [
  fakeText(8, NOW - 20_000, "单行短文本 (浏览器预览)", true),
  fakeImage(7, NOW - 3 * MINUTE, 1920, 1080),
  fakeFiles(6, NOW - 12 * MINUTE, [
    ["D:\\资料\\报告 (浏览器预览).docx", true],
    ["D:\\资料\\已删除的附件.pdf", false],
    ["D:\\资料\\图表.xlsx", true],
  ]),
  fakeText(5, NOW - 2 * HOUR, FAKE_LONG_TEXT),
  fakeFiles(4, NOW - 5 * HOUR, [["C:\\Users\\preview\\Desktop\\单个文件 (浏览器预览).txt", true]]),
  fakeImage(3, NOW - 2 * DAY, 640, 960, true),
  fakeText(2, NOW - 3 * DAY, "第一行 (浏览器预览)\n第二行\n第三行:多行但不足 300 字"),
  fakeText(1, NOW - 40 * DAY, "https://example.com/very-old-link (浏览器预览)"),
];

/** 假数据的「可搜索文本」,与后端 `searchable_text` 同规则:text = 全文(这里只有 preview);files = 文件名;image = 不命中 */
function fakeSearchableText(item: ClipboardItem): string | null {
  switch (item.kind) {
    case "text":
      return item.preview;
    case "files":
      return item.files.map((f) => f.name).join("\n");
    case "image":
      return null;
    default:
      return null;
  }
}

function listFakeItems(query: ListQuery): ClipboardItem[] {
  const keyword = query.query.trim().toLowerCase();
  const before = query.before;
  return (
    fakeItems
      .filter((item) => query.kind === undefined || item.kind === query.kind)
      .filter((item) => !query.favoriteOnly || item.favorite)
      .filter((item) => {
        if (keyword === "") return true;
        const text = fakeSearchableText(item);
        return text !== null && text.toLowerCase().includes(keyword);
      })
      .filter(
        (item) =>
          before === undefined ||
          item.copiedAt < before.copiedAt ||
          (item.copiedAt === before.copiedAt && item.id < before.id),
      )
      .sort((a, b) => b.copiedAt - a.copiedAt || b.id - a.id)
      .slice(0, query.limit)
      // 返回深拷贝:真实 IPC 经 JSON 反序列化给前端的是全新对象,预览也要一样。
      // 否则 mutateFake 改裸对象后,composable 再对同一对象的 reactive 代理赋同值会被 Vue 视为「没变」而不重渲染
      .map((item) => structuredClone(item))
  );
}

function getFakeText(id: number): Promise<string> {
  const item = fakeItems.find((i) => i.id === id);
  if (item === undefined) return Promise.reject(new Error("参数错误: 记录不存在"));
  if (item.kind !== "text") return Promise.reject(new Error("参数错误: 该条目不是文本"));
  // 只有截断的那条有「全文」;其余条目 preview 就是全文
  return Promise.resolve(item.truncated ? FAKE_LONG_TEXT : item.preview);
}

/** 对假数据做一次变更;id 不存在时与后端一样 reject */
function mutateFake(id: number, apply: (item: ClipboardItem) => void): Promise<void> {
  const item = fakeItems.find((i) => i.id === id);
  if (item === undefined) return Promise.reject(new Error("参数错误: 记录不存在"));
  apply(item);
  return Promise.resolve();
}
