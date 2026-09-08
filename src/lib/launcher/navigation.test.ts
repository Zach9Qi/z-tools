// 测试 chunkRows() / nextIndex():磁贴网格的分行与方向键移动规则
import { describe, expect, it } from "vitest";
import type { ToolItem } from "@/types/tool";
import type { Section } from "./search";
import { chunkRows, nextIndex } from "./navigation";

/** 构造指定条目数的分区,全局下标从 start 开始连续编号 */
function section(id: Section["id"], count: number, start: number): Section {
  const items = Array.from({ length: count }, (_, i) => {
    const item: ToolItem = {
      id: `${id}-${i}`,
      title: `${id}-${i}`,
      icon: "puzzle",
      keywords: [],
      action: "launch",
    };
    return { item, index: start + i, sectionId: id };
  });
  return { id, title: id, items };
}

describe("chunkRows", () => {
  it("单分区按 columns 分行,最后一行装剩余条目", () => {
    const rows = chunkRows([section("all", 10, 0)], 8);
    expect(rows).toEqual([
      [0, 1, 2, 3, 4, 5, 6, 7],
      [8, 9],
    ]);
  });

  it("不同分区各自分行,不共用一行", () => {
    // 第一个分区 3 个、第二个 2 个;若共行会变成一行 5 个
    const rows = chunkRows([section("matches", 3, 0), section("named", 2, 3)], 8);
    expect(rows).toEqual([
      [0, 1, 2],
      [3, 4],
    ]);
  });

  it("分区条目数恰好整除 columns 时不产生空行", () => {
    const rows = chunkRows([section("all", 8, 0)], 8);
    expect(rows).toEqual([[0, 1, 2, 3, 4, 5, 6, 7]]);
  });

  it("空分区列表返回空数组", () => {
    expect(chunkRows([], 8)).toEqual([]);
  });
});

describe("nextIndex", () => {
  // 10 个条目按 8 列:第一行 0~7,第二行 8~9
  const rows = chunkRows([section("all", 10, 0)], 8);

  it("右移 +1、左移 -1", () => {
    expect(nextIndex("right", 3, rows)).toBe(4);
    expect(nextIndex("left", 3, rows)).toBe(2);
  });

  it("右移到末尾后回绕到 0,左移到 0 后回绕到末尾", () => {
    expect(nextIndex("right", 9, rows)).toBe(0);
    expect(nextIndex("left", 0, rows)).toBe(9);
  });

  it("左右移动跨行:第一行末尾右移进入第二行开头", () => {
    expect(nextIndex("right", 7, rows)).toBe(8);
    expect(nextIndex("left", 8, rows)).toBe(7);
  });

  it("下移保留列号", () => {
    expect(nextIndex("down", 1, rows)).toBe(9);
  });

  it("下移到短行时贴行尾", () => {
    // 第 6 列在第二行不存在,落到第二行最后一个(下标 9)
    expect(nextIndex("down", 5, rows)).toBe(9);
  });

  it("上移保留列号", () => {
    expect(nextIndex("up", 9, rows)).toBe(1);
  });

  it("底行下移回绕到顶行,顶行上移回绕到底行", () => {
    expect(nextIndex("down", 8, rows)).toBe(0);
    expect(nextIndex("up", 0, rows)).toBe(8);
    // 顶行第 7 列上移回绕到底行,底行只有 2 个则贴行尾
    expect(nextIndex("up", 7, rows)).toBe(9);
  });

  it("跨分区时上下移动按分区行边界跳转", () => {
    const multi = chunkRows([section("matches", 3, 0), section("named", 2, 3)], 8);
    expect(nextIndex("down", 0, multi)).toBe(3);
    expect(nextIndex("down", 2, multi)).toBe(4);
    expect(nextIndex("up", 4, multi)).toBe(1);
  });

  it("只有一行时上下移动停在同一行(回绕到自身)", () => {
    const single = chunkRows([section("all", 3, 0)], 8);
    expect(nextIndex("up", 1, single)).toBe(1);
    expect(nextIndex("down", 2, single)).toBe(2);
  });

  it("rows 为空时返回 -1(无可选条目)", () => {
    expect(nextIndex("right", 0, [])).toBe(-1);
    expect(nextIndex("down", 0, [])).toBe(-1);
  });

  it("current 不在任何行时回到 0(选中项已被过滤掉)", () => {
    expect(nextIndex("right", 42, rows)).toBe(0);
    expect(nextIndex("up", -1, rows)).toBe(0);
  });
});
