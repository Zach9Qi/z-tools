import { fileURLToPath, URL } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import tailwindcss from "@tailwindcss/vite";
import icons from "unplugin-icons/vite";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [
    vue(),
    tailwindcss(),
    // 图标:按需把 Iconify 图标编译成 Vue 组件,写法 `import IconX from "~icons/lucide/x"`,
    // 只打包用到的图标。图标集装在 devDependencies(@iconify-json/lucide),不开 autoInstall,缺集时显式安装
    icons({
      compiler: "vue3",
      // 默认尺寸 1em 而非 1.2em,交给 Tailwind 的 size-* 控制
      scale: 1,
    }),
  ],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // 以下选项专为 Tauri 开发准备，仅在 `tauri dev` / `tauri build` 时生效
  //
  // 1. 关闭清屏，避免盖住 Rust 编译错误
  clearScreen: false,
  // 2. Tauri 需要固定端口，被占用时直接失败
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. 不要监听 src-tauri，后端由 cargo 自己编译
      ignored: ["**/src-tauri/**"],
    },
  },
}));
