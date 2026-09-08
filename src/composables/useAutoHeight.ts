// 让窗口高度跟随面板内容:ResizeObserver 观察面板根元素,高度变化时同步给窗口。
// 非 Tauri 环境同样观察,由 lib/window 内部降级为 no-op,避免分支分散在两处。
import { onMounted, onUnmounted, type Ref } from "vue";
import { resizeLauncherToContent } from "@/lib/window";

/**
 * 观察 target 的尺寸变化并把 offsetHeight 同步到窗口;挂载时立即同步一次,卸载时断开观察。
 * 用 Math.ceil 取整:小数高度传给窗口会被截断,导致底边少 1px 被裁切。
 */
export function useAutoHeight(target: Ref<HTMLElement | null>): void {
  /** 当前的观察者;null 表示尚未挂载或已断开 */
  let observer: ResizeObserver | null = null;

  function sync(): void {
    const el = target.value;
    if (el === null) return;
    void resizeLauncherToContent(Math.ceil(el.offsetHeight));
  }

  onMounted(() => {
    const el = target.value;
    if (el === null) return;
    observer = new ResizeObserver(sync);
    observer.observe(el);
    sync();
  });

  onUnmounted(() => {
    observer?.disconnect();
    observer = null;
  });
}
