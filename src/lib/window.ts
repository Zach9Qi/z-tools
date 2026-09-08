// 启动器窗口控制的唯一入口:整个 src/ 只允许这里 import `@tauri-apps/api/window`。
// 与 lib/api.ts 同一套规则:浏览器预览(无 Tauri 运行时)降级为 no-op,失败只记日志不抛出。
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { isTauriRuntime } from "@/lib/runtime";

/**
 * 隐藏启动器窗口(主页 Esc)。
 * 非 Tauri 运行时没有窗口可隐藏,直接 resolve;失败时 `console.error` 并吞掉,
 * 因为隐藏失败最多是窗口留在屏幕上,不应让 UI 进入错误态。
 */
export async function hideLauncher(): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await getCurrentWindow().hide();
  } catch (error) {
    console.error("隐藏启动器失败:", error);
  }
}

/**
 * 把窗口内容区高度设为 height(逻辑像素),宽度保持当前 `window.innerWidth` 不变。
 * 非 Tauri 运行时 no-op(浏览器无法改标签页尺寸);失败时 `console.error` 并吞掉,
 * 尺寸没同步只是留白 / 裁切,不应打断交互。
 */
export async function resizeLauncherToContent(height: number): Promise<void> {
  if (!isTauriRuntime()) return;
  try {
    await getCurrentWindow().setSize(new LogicalSize(window.innerWidth, height));
  } catch (error) {
    console.error("调整启动器窗口失败:", error);
  }
}
