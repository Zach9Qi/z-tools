// lucide 图标名 → Vue 组件。
// unplugin-icons 只能静态 import(编译期内联 SVG),无法按运行时字符串加载,
// 所以工具用到的图标必须在这里手工登记;ToolItem.icon 写的就是本表的键。
// 只登记经 ToolItem.icon 使用的图标;工具页内部自己用的图标直接在组件里 import,不走这张表。
import type { Component } from "vue";
import IconClipboard from "~icons/lucide/clipboard";
import IconPuzzle from "~icons/lucide/puzzle";

/** 已登记的图标表;键与 lucide 图标名一致,便于对照 lucide 官网挑图标 */
const ICONS: Record<string, Component> = {
  clipboard: IconClipboard,
  puzzle: IconPuzzle,
};

/**
 * 按 lucide 图标名取组件。
 * 未登记的名字回退为 puzzle(「未知工具」的通用意象),而不是抛错——
 * 图标缺失只是视觉问题,不应让整个网格渲染失败。
 */
export function iconOf(name: string): Component {
  return ICONS[name] ?? IconPuzzle;
}
