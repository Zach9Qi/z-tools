/**
 * 当前页面是否跑在 Tauri WebView 里。
 * WebView 会注入 `__TAURI_INTERNALS__`;`vite` / 浏览器预览没有这层 IPC,
 * 窗口与 invoke API 均不可用,各封装层据此降级为 no-op。
 */
export function isTauriRuntime(): boolean {
  return "__TAURI_INTERNALS__" in globalThis;
}
