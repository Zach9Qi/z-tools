// 剪贴板列表的展示用纯函数:相对时间、字节数、文件摘要、可展开判定。
// 不依赖 Vue;组件只做绑定,规则集中在这里便于单测。
import type { ClipboardFile, ClipboardItem } from "@/types/clipboard";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
/** 超过这个天数不再显示「N 天前」,改显示日期——「30 天前」不如日期直观 */
const RELATIVE_DAYS_LIMIT = 7;

/**
 * 把复制时间格式化为相对文案:1 分钟内「刚刚」、1 小时内「N 分钟前」、1 天内「N 小时前」、
 * 7 天内「N 天前」,更早显示 `YYYY-MM-DD`。`ms` 晚于 `now`(时钟回拨)按「刚刚」处理。
 */
export function formatRelativeTime(ms: number, now: number = Date.now()): string {
  const diff = now - ms;
  if (diff < MINUTE) return "刚刚";
  if (diff < HOUR) return `${Math.floor(diff / MINUTE)} 分钟前`;
  if (diff < DAY) return `${Math.floor(diff / HOUR)} 小时前`;
  if (diff < RELATIVE_DAYS_LIMIT * DAY) return `${Math.floor(diff / DAY)} 天前`;
  return formatDate(ms);
}

/** 本地时区 `YYYY-MM-DD`;不用 toLocaleDateString,避免不同 WebView 语言设置下格式漂移 */
function formatDate(ms: number): string {
  const d = new Date(ms);
  const pad = (n: number): string => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

const BYTE_UNITS = ["B", "KB", "MB", "GB"] as const;

/**
 * 字节数 → 人类可读:`0 B`、`512 B`、`1.5 KB`、`20.0 MB`。
 * 1024 进制;B 不带小数,更大单位保留一位。负数 / NaN 视为 0。
 */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit === 0 ? `${Math.round(value)} B` : `${value.toFixed(1)} ${BYTE_UNITS[unit]}`;
}

/**
 * 文件条目的收起态摘要:单个显示文件名,多个显示「首文件名 等 N 项」。
 * 文件名直接用后端给的 `name`,前端不解析路径(分隔符 / 含换行的文件名等平台差异留给 Rust)。
 * 空列表理论上不会入库,给一个兜底文案而不是抛错。
 */
export function summarizeFiles(files: ClipboardFile[]): string {
  const first = files[0];
  if (first === undefined) return "(空文件列表)";
  if (files.length === 1) return first.name;
  return `${first.name} 等 ${files.length} 项`;
}

/**
 * 条目是否值得展开(决定行右侧是否渲染 chevron):
 * - text:被截断、或预览含换行(多行短文本收起态只显两行);单行短文本不给。
 *   超长单行但未截断的文本会被 line-clamp 截断却不给 chevron,这是接受的近似——前端无法便宜地判断两行是否溢出。
 * - image:恒可展开(缩略图与原图不同)。
 * - files:多于一个文件;单文件收起态已显示文件名,完整路径用 title 提示即可。
 */
export function isExpandable(item: ClipboardItem): boolean {
  switch (item.kind) {
    case "text":
      return item.truncated || item.preview.includes("\n");
    case "image":
      return true;
    case "files":
      return item.files.length > 1;
    default:
      return false;
  }
}
