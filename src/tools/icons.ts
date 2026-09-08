// lucide 图标名 → Vue 组件。
// unplugin-icons 只能静态 import(编译期内联 SVG),无法按运行时字符串加载,
// 所以工具用到的图标必须在这里手工登记;ToolItem.icon 写的就是本表的键。
import type { Component } from "vue";
import IconCalculator from "~icons/lucide/calculator";
import IconClock from "~icons/lucide/clock";
import IconFileText from "~icons/lucide/file-text";
import IconFolder from "~icons/lucide/folder";
import IconGlobe from "~icons/lucide/globe";
import IconHash from "~icons/lucide/hash";
import IconImage from "~icons/lucide/image";
import IconLayoutGrid from "~icons/lucide/layout-grid";
import IconLink from "~icons/lucide/link";
import IconPalette from "~icons/lucide/palette";
import IconPuzzle from "~icons/lucide/puzzle";
import IconSettings from "~icons/lucide/settings";
import IconSparkles from "~icons/lucide/sparkles";
import IconTerminal from "~icons/lucide/terminal";
import IconZap from "~icons/lucide/zap";

/** 已登记的图标表;键与 lucide 图标名一致,便于对照 lucide 官网挑图标 */
const ICONS: Record<string, Component> = {
  calculator: IconCalculator,
  clock: IconClock,
  "file-text": IconFileText,
  folder: IconFolder,
  globe: IconGlobe,
  hash: IconHash,
  image: IconImage,
  "layout-grid": IconLayoutGrid,
  link: IconLink,
  palette: IconPalette,
  puzzle: IconPuzzle,
  settings: IconSettings,
  sparkles: IconSparkles,
  terminal: IconTerminal,
  zap: IconZap,
};

/**
 * 按 lucide 图标名取组件。
 * 未登记的名字回退为 puzzle(「未知工具」的通用意象),而不是抛错——
 * 图标缺失只是视觉问题,不应让整个网格渲染失败。
 */
export function iconOf(name: string): Component {
  return ICONS[name] ?? IconPuzzle;
}
