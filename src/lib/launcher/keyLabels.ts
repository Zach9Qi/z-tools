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

/**
 * 全局快捷键(tauri-plugin-global-shortcut 语法)里修饰键 / 特殊键的可读标签。
 * 与 KEY_LABELS 用途不同:页脚 hints 追求紧凑(Enter → ↵),而唤出键提示要一眼可读,所以 Enter 就写 Enter。
 * super / meta / cmd 统一显示 Meta;后续可按平台映射为 Win / ⌘。
 */
const SHORTCUT_LABELS: Record<string, string> = {
  alt: "Alt",
  ctrl: "Ctrl",
  control: "Ctrl",
  shift: "Shift",
  super: "Meta",
  meta: "Meta",
  cmd: "Meta",
  command: "Meta",
  enter: "Enter",
  space: "Space",
  escape: "Esc",
};

/**
 * 把后端返回的快捷键字符串(plugin 语法,`修饰键+主键`)拆成键帽序列,如 `["Alt", "Enter"]`。
 * 按 `+` 拆分并 trim,大小写不敏感;登记在 SHORTCUT_LABELS 的用表里的值,其余首字母大写(`a → A`、`f1 → F1`);
 * 空段丢弃,空字符串返回 `[]`。
 */
export function parseShortcut(shortcut: string): string[] {
  return shortcut
    .split("+")
    .map((part) => part.trim())
    .filter((part) => part !== "")
    .map((part) => {
      const label = SHORTCUT_LABELS[part.toLowerCase()];
      if (label !== undefined) return label;
      return part.charAt(0).toUpperCase() + part.slice(1);
    });
}
