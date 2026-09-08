/**
 * KeyboardEvent.key → 页脚键帽短标签。
 * 页脚空间有限,方向键与功能键用符号 / 缩写;未登记的键走 formatKeyLabel 的通用规则。
 */
export const KEY_LABELS: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Enter: "↵",
  Delete: "Del",
  Escape: "Esc",
  Backspace: "⌫",
};

/**
 * 把 KeyboardEvent.key 转成键帽标签。
 * 命中 KEY_LABELS 用表里的值;单字母键转大写(浏览器对字母键给的是小写 `k`,键帽习惯显示 `K`);
 * 其余(`Tab` / `F1` 等)原样返回。
 */
export function formatKeyLabel(key: string): string {
  const label = KEY_LABELS[key];
  if (label !== undefined) return label;
  return key.length === 1 ? key.toUpperCase() : key;
}
