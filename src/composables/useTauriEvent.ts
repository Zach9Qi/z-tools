// 整个 src/ 只允许这里 import `@tauri-apps/api/event`;组件 / store 通过本 composable 订阅 Rust 事件。
import { onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { EventPayloads } from "@/lib/events";
import { isTauriRuntime } from "@/lib/runtime";

/**
 * 监听一个 Rust 事件,组件卸载时自动取消订阅。
 * - 浏览器预览没有事件总线,`listen` 会报错,降级为不订阅;
 * - `listen` 是异步的,可能在组件已卸载后才 resolve,此时立刻 unlisten 避免泄漏;
 * - 订阅失败只记日志不抛出,少收一个事件不应让 UI 进入错误态。
 */
export function useTauriEvent<K extends keyof EventPayloads>(
  name: K,
  handler: (payload: EventPayloads[K]) => void,
): void {
  if (!isTauriRuntime()) return;
  /** 取消订阅函数;listen resolve 前为 undefined */
  let unlisten: UnlistenFn | undefined;
  /** 组件是否已卸载;用于处理 listen 晚于卸载 resolve 的竞态 */
  let disposed = false;
  listen<EventPayloads[K]>(name, (e) => handler(e.payload))
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    .catch((e) => console.error("监听事件失败:", e));
  onUnmounted(() => {
    disposed = true;
    unlisten?.();
  });
}
