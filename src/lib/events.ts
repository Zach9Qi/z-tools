// Rust → 前端事件的名字与 payload 类型,集中在这里;
// 与 `src-tauri/src/launcher.rs` 的 `LAUNCHER_OPENED` / `LAUNCHER_CLOSED` 常量一一对应,
// Rust 侧改名这里必须同步,否则前端静默收不到事件且无报错。

/** 事件名常量;格式 `domain://action` */
export const EVENTS = {
  /** 启动器窗口已显示并抢到焦点(后端 show 之后 emit) */
  LAUNCHER_OPENED: "launcher://open",
  /** 启动器窗口已隐藏(后端 hide 之后 emit) */
  LAUNCHER_CLOSED: "launcher://close",
} as const;

/** 事件名 → payload 类型;两个启动器事件都无 payload,Rust 侧 emit 的是 `()`,前端收到 null */
export interface EventPayloads {
  [EVENTS.LAUNCHER_OPENED]: null;
  [EVENTS.LAUNCHER_CLOSED]: null;
}
