// 剪贴板工具的模块定义;在 src/tools/registry.ts 登记后出现在主页网格。
import type { ViewToolModule } from "@/types/tool";
import ClipboardPage from "./ClipboardPage.vue";

export const clipboardTool: ViewToolModule = {
  item: {
    id: "clipboard",
    title: "剪贴板",
    icon: "clipboard",
    // jtb = 「剪贴板」拼音首字母;「剪切板」是常见别称
    keywords: ["clipboard", "jtb", "剪切板", "粘贴", "历史", "paste"],
    action: "view",
  },
  page: ClipboardPage,
  placeholder: "搜索剪贴板历史…",
};
