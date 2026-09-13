// Rust → 前端事件的名字与 payload 类型,集中在这里;
// 与 `src-tauri/src/launcher.rs` 的 `LAUNCHER_OPENED` / `LAUNCHER_CLOSED`、
// `src-tauri/src/clipboard.rs` 的 `CLIPBOARD_CHANGED` 常量一一对应,
// Rust 侧改名 / 改 payload 这里必须同步,否则前端静默收不到事件或读到 undefined 且无报错。
import type { ClipboardItem } from "@/types/clipboard";

/** 事件名常量;格式 `domain://action` */
export const EVENTS = {
  /** 启动器窗口已显示并抢到焦点(后端 show 之后 emit) */
  LAUNCHER_OPENED: "launcher://open",
  /** 启动器窗口已隐藏(后端 hide 之后 emit) */
  LAUNCHER_CLOSED: "launcher://close",
  /**
   * 剪贴板监听器录入了新内容或上浮了旧内容(只由领域层 record() emit;前端自己发起的删除 / 收藏不发)。
   * payload 是落库后的条目;重复复制同一内容时以同 id、新 copiedAt 重发,消费方按 id 去重(已存在则上浮而非重复插入)。
   */
  CLIPBOARD_CHANGED: "clipboard://changed",
} as const;

/** 事件名 → payload 类型;无 payload 的事件 Rust 侧 emit `()`,前端收到 null */
export interface EventPayloads {
  [EVENTS.LAUNCHER_OPENED]: null;
  [EVENTS.LAUNCHER_CLOSED]: null;
  [EVENTS.CLIPBOARD_CHANGED]: ClipboardItem;
}
