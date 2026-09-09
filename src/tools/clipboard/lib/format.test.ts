// 测试剪贴板列表的展示纯函数:相对时间分档、字节单位、文件摘要、可展开判定的每个分支
import { describe, expect, it } from "vitest";
import type { ClipboardFile, ClipboardItem } from "@/types/clipboard";
import { formatBytes, formatRelativeTime, isExpandable, summarizeFiles } from "./format";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
/** 固定「现在」,让相对时间可复现 */
const NOW = Date.UTC(2025, 0, 15, 12, 0, 0);

describe("formatRelativeTime", () => {
  it("1 分钟内显示「刚刚」", () => {
    expect(formatRelativeTime(NOW, NOW)).toBe("刚刚");
    expect(formatRelativeTime(NOW - 59_000, NOW)).toBe("刚刚");
  });

  it("时间晚于 now(时钟回拨)也按「刚刚」处理", () => {
    expect(formatRelativeTime(NOW + HOUR, NOW)).toBe("刚刚");
  });

  it("1 小时内显示 N 分钟前,向下取整", () => {
    expect(formatRelativeTime(NOW - MINUTE, NOW)).toBe("1 分钟前");
    expect(formatRelativeTime(NOW - 59 * MINUTE - 59_000, NOW)).toBe("59 分钟前");
  });

  it("1 天内显示 N 小时前", () => {
    expect(formatRelativeTime(NOW - HOUR, NOW)).toBe("1 小时前");
    expect(formatRelativeTime(NOW - 23 * HOUR, NOW)).toBe("23 小时前");
  });

  it("7 天内显示 N 天前", () => {
    expect(formatRelativeTime(NOW - DAY, NOW)).toBe("1 天前");
    expect(formatRelativeTime(NOW - 6 * DAY - 23 * HOUR, NOW)).toBe("6 天前");
  });

  it("7 天及以上显示 YYYY-MM-DD 日期", () => {
    const ms = NOW - 7 * DAY;
    const d = new Date(ms);
    const expected = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    expect(formatRelativeTime(ms, NOW)).toBe(expected);
    expect(formatRelativeTime(ms, NOW)).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  });

  it("不传 now 时以当前时间为基准", () => {
    expect(formatRelativeTime(Date.now())).toBe("刚刚");
  });
});

describe("formatBytes", () => {
  it("0、负数、NaN 都显示 0 B", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(-5)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });

  it("不足 1 KB 用 B 且不带小数", () => {
    expect(formatBytes(1)).toBe("1 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });

  it("KB / MB / GB 保留一位小数", () => {
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(20 * 1024 * 1024)).toBe("20.0 MB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB");
  });

  it("超过 GB 不再进位,仍用 GB 表示", () => {
    expect(formatBytes(2048 * 1024 ** 3)).toBe("2048.0 GB");
  });
});

function file(name: string, exists = true): ClipboardFile {
  return { path: `C:\\dir\\${name}`, name, exists };
}

describe("summarizeFiles", () => {
  it("单个文件只显示文件名", () => {
    expect(summarizeFiles([file("报告.docx")])).toBe("报告.docx");
  });

  it("多个文件显示首文件名加「等 N 项」", () => {
    expect(summarizeFiles([file("a.txt"), file("b.txt"), file("c.txt")])).toBe("a.txt 等 3 项");
  });

  it("文件名直接取 DTO 的 name,不从 path 解析", () => {
    const f: ClipboardFile = { path: "/tmp/real-name.txt", name: "给定的名字", exists: true };
    expect(summarizeFiles([f])).toBe("给定的名字");
  });

  it("空列表给兜底文案而不抛错", () => {
    expect(summarizeFiles([])).toBe("(空文件列表)");
  });
});

const base = { id: 1, favorite: false, copiedAt: NOW };

describe("isExpandable", () => {
  it("text:单行且未截断不可展开", () => {
    const item: ClipboardItem = {
      ...base,
      kind: "text",
      size: 5,
      preview: "hello",
      charCount: 5,
      truncated: false,
    };
    expect(isExpandable(item)).toBe(false);
  });

  it("text:被截断可展开", () => {
    const item: ClipboardItem = {
      ...base,
      kind: "text",
      size: 400,
      preview: "x".repeat(300),
      charCount: 400,
      truncated: true,
    };
    expect(isExpandable(item)).toBe(true);
  });

  it("text:未截断但预览含换行可展开", () => {
    const item: ClipboardItem = {
      ...base,
      kind: "text",
      size: 7,
      preview: "第一行\n第二行",
      charCount: 7,
      truncated: false,
    };
    expect(isExpandable(item)).toBe(true);
  });

  it("image:恒可展开", () => {
    const item: ClipboardItem = {
      ...base,
      kind: "image",
      size: 1000,
      imagePath: "C:\\a.png",
      thumbPath: "C:\\a.thumb.png",
      width: 10,
      height: 10,
    };
    expect(isExpandable(item)).toBe(true);
  });

  it("files:单文件不可展开,多文件可展开", () => {
    const single: ClipboardItem = { ...base, kind: "files", files: [file("a.txt")] };
    const multi: ClipboardItem = { ...base, kind: "files", files: [file("a.txt"), file("b.txt")] };
    expect(isExpandable(single)).toBe(false);
    expect(isExpandable(multi)).toBe(true);
  });
});
