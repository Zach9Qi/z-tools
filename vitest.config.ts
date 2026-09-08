import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vitest/config";

// Vitest 独立配置,不复用 vite.config.ts:
// 那份带 Tauri dev server 与 Vue/Tailwind 插件,纯函数测试用不上,隔离开互不影响
export default defineConfig({
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    // 测试文件与被测源码同目录,命名 xxx.test.ts
    include: ["src/**/*.test.ts"],
  },
});
