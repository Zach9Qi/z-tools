import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/runtime";

/**
 * 隐藏启动器窗口(主页 Esc)。
 * 非 Tauri 运行时没有窗口可隐藏,直接 resolve(no-op);后端命令不可失败,
 * reject 只可能是 IPC 层异常,调用方 catch 后 `console.error` 即可,不必进入错误态。
 */
export function hideLauncher(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve();
  return invoke<void>("hide_launcher");
}

/**
 * 读取当前生效的全局唤出快捷键(plugin 语法,如 `"alt+enter"`),前端用 `parseShortcut` 转成键帽提示。
 * 非 Tauri 运行时(浏览器预览)没有后端,回退为默认值——这是前端**唯一**允许出现该字面量的地方,
 * 真实值以后端 `launcher::DEFAULT_TOGGLE_SHORTCUT` / 用户设置为准;后端命令不可失败,reject 只可能是 IPC 层异常。
 */
export function getToggleShortcut(): Promise<string> {
  if (!isTauriRuntime()) return Promise.resolve("alt+enter");
  return invoke<string>("get_toggle_shortcut");
}
