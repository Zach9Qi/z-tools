import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "@/lib/runtime";

/**
 * 调用后端 `greet` 命令,返回问候文案。
 * 非 Tauri 运行时(`bun run dev` 浏览器预览)没有 IPC 层,直接返回降级文案,保证页面可打开;
 * 后端出错时 `AppError` 序列化出来的就是用户可读中文,调用方直接展示即可。
 */
export function greet(name: string): Promise<string> {
  if (!isTauriRuntime()) {
    return Promise.resolve(`（浏览器预览）你好，${name}！`);
  }
  return invoke<string>("greet", { name });
}
