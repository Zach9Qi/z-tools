// 示例工具,真实工具进来后整目录删除。
// 作用:让主页网格有 ≥9 个磁贴(出现第二行换行)、有一个 view 型工具可进入工具页、
// 有一个 accepts 动态匹配的工具可验证「匹配结果」分区。run() 全部无副作用,提示由面板负责。
import type { LaunchToolModule, ViewToolModule } from "@/types/tool";
import DemoToolPage from "@/tools/demo/DemoToolPage.vue";

/** view 型占位:进入工具页后回显搜索栏输入 */
export const demoViewTool: ViewToolModule = {
  item: {
    id: "demo-view",
    title: "示例工具页",
    icon: "layout-grid",
    keywords: ["demo", "view", "page"],
    action: "view",
  },
  page: DemoToolPage,
  placeholder: "在示例工具里搜索…",
};

/** launch 型占位:激活后什么都不做,面板据 resolved Promise 显示一次性提示 */
export const demoLaunchTools: LaunchToolModule[] = [
  {
    item: {
      id: "demo-launch-1",
      title: "计算器",
      icon: "calculator",
      keywords: ["demo", "calc", "math"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-2",
      title: "时钟",
      icon: "clock",
      keywords: ["demo", "time"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-3",
      title: "打开链接",
      icon: "link",
      keywords: ["url", "open"],
      // 输入以 http 开头时进入「匹配结果」分区,用于验证 accepts 链路
      accepts: (q) => q.startsWith("http"),
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-4",
      title: "终端",
      icon: "terminal",
      keywords: ["demo", "shell", "cmd"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-5",
      title: "文件夹",
      icon: "folder",
      keywords: ["dir", "explorer"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-6",
      title: "取色器",
      icon: "palette",
      keywords: ["demo", "color", "picker"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-7",
      title: "设置",
      icon: "settings",
      keywords: ["config", "preferences"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-8",
      title: "图片压缩",
      icon: "image",
      keywords: ["demo", "compress", "png"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-9",
      title: "文本处理",
      icon: "file-text",
      keywords: ["text", "format"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-10",
      title: "哈希计算",
      icon: "hash",
      keywords: ["demo", "md5", "sha"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-11",
      title: "快捷动作",
      icon: "zap",
      keywords: ["action", "quick"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
  {
    item: {
      id: "demo-launch-12",
      title: "翻译",
      icon: "globe",
      keywords: ["demo", "translate"],
      action: "launch",
    },
    run: () => Promise.resolve(),
  },
];
