// 测试 formatKeyLabel():把 KeyboardEvent.key 转成页脚键帽上的短标签
import { describe, expect, it } from "vitest";
import { KEY_LABELS, formatKeyLabel } from "./keyLabels";

describe("formatKeyLabel", () => {
  it("四个方向键映射为箭头符号", () => {
    expect(formatKeyLabel("ArrowUp")).toBe("↑");
    expect(formatKeyLabel("ArrowDown")).toBe("↓");
    expect(formatKeyLabel("ArrowLeft")).toBe("←");
    expect(formatKeyLabel("ArrowRight")).toBe("→");
  });

  it("Enter / Delete / Escape / Backspace 映射为约定缩写", () => {
    expect(formatKeyLabel("Enter")).toBe("↵");
    expect(formatKeyLabel("Delete")).toBe("Del");
    expect(formatKeyLabel("Escape")).toBe("Esc");
    expect(formatKeyLabel("Backspace")).toBe("⌫");
  });

  it("未登记的单字母键转大写(KeyboardEvent.key 对字母键给的是小写)", () => {
    expect(formatKeyLabel("k")).toBe("K");
    expect(formatKeyLabel("A")).toBe("A");
  });

  it("未登记的多字符键原样返回", () => {
    expect(formatKeyLabel("Tab")).toBe("Tab");
    expect(formatKeyLabel("F1")).toBe("F1");
    expect(formatKeyLabel(" ")).toBe(" ");
  });

  it("KEY_LABELS 表里的每个键都能通过 formatKeyLabel 拿到同一标签", () => {
    for (const [key, label] of Object.entries(KEY_LABELS)) {
      expect(formatKeyLabel(key)).toBe(label);
    }
  });
});
