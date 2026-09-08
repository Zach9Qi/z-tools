// 测试 buildSections() / flattenSections():主页与搜索态的分区划分与全局下标编号
import { describe, expect, it } from "vitest";
import type { ToolItem } from "@/types/tool";
import { buildSections, flattenSections } from "./search";

/** 构造最小 ToolItem,只关心搜索相关字段 */
function tool(id: string, title: string, extra: Partial<ToolItem> = {}): ToolItem {
  return { id, title, icon: "puzzle", keywords: [], action: "launch", ...extra };
}

const catalog: ToolItem[] = [
  tool("calc", "计算器", { keywords: ["calc", "math"] }),
  tool("clock", "时钟", { keywords: ["Time"] }),
  tool("url", "打开链接", { accepts: (q) => q.startsWith("http") }),
  tool("demo", "示例工具", { keywords: ["demo"] }),
];

describe("buildSections", () => {
  it("空 query 返回单一「全部工具」分区,包含全部条目", () => {
    const sections = buildSections(catalog, "");
    expect(sections).toHaveLength(1);
    expect(sections[0]?.id).toBe("all");
    expect(sections[0]?.title).toBe("全部工具");
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["calc", "clock", "url", "demo"]);
  });

  it("只有空白字符的 query 视同空 query", () => {
    const sections = buildSections(catalog, "   ");
    expect(sections).toHaveLength(1);
    expect(sections[0]?.id).toBe("all");
  });

  it("title 子串命中进入「搜索结果」分区", () => {
    const sections = buildSections(catalog, "计算");
    expect(sections).toHaveLength(1);
    expect(sections[0]?.id).toBe("named");
    expect(sections[0]?.title).toBe("搜索结果");
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["calc"]);
  });

  it("keywords 命中且大小写不敏感", () => {
    const sections = buildSections(catalog, "tIme");
    expect(sections.map((s) => s.id)).toEqual(["named"]);
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["clock"]);
  });

  it("accepts 命中进入「匹配结果」分区且排在「搜索结果」之前", () => {
    // "http" 同时命中 url 的 accepts;不命中任何 title / keywords
    const sections = buildSections(catalog, "http://a.b");
    expect(sections.map((s) => s.id)).toEqual(["matches"]);
    expect(sections[0]?.title).toBe("匹配结果");
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["url"]);
  });

  it("同时命中名称与 accepts 时只进「搜索结果」,不重复出现在「匹配结果」", () => {
    const both = [tool("http-tool", "HTTP 请求", { accepts: (q) => q.startsWith("http") })];
    const sections = buildSections([...both, ...catalog], "http");
    expect(sections.map((s) => s.id)).toEqual(["matches", "named"]);
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["url"]);
    expect(sections[1]?.items.map((s) => s.item.id)).toEqual(["http-tool"]);
  });

  it("空分区被剔除,两者皆空时返回空数组", () => {
    expect(buildSections(catalog, "不存在的工具")).toEqual([]);
  });

  it("多分区时全局下标跨分区连续编号", () => {
    const both = [tool("http-tool", "HTTP 请求", { accepts: (q) => q.startsWith("http") })];
    const sections = buildSections([...both, ...catalog], "http");
    const indices = sections.flatMap((s) => s.items.map((x) => x.index));
    expect(indices).toEqual([0, 1]);
  });

  it("每个条目标记自己所属的分区 id", () => {
    const both = [tool("http-tool", "HTTP 请求", { accepts: (q) => q.startsWith("http") })];
    const flat = flattenSections(buildSections([...both, ...catalog], "http"));
    expect(flat.map((s) => [s.item.id, s.sectionId])).toEqual([
      ["url", "matches"],
      ["http-tool", "named"],
    ]);
    expect(buildSections(catalog, "")[0]?.items[0]?.sectionId).toBe("all");
  });

  it("空 query 时全局下标与 catalog 顺序一致", () => {
    const sections = buildSections(catalog, "");
    expect(sections[0]?.items.map((s) => s.index)).toEqual([0, 1, 2, 3]);
  });

  it("query 前后空白会被 trim 后再匹配", () => {
    const sections = buildSections(catalog, "  demo  ");
    expect(sections[0]?.items.map((s) => s.item.id)).toEqual(["demo"]);
  });
});

describe("flattenSections", () => {
  it("按分区顺序展平为一维数组,下标与位置一致", () => {
    const both = [tool("http-tool", "HTTP 请求", { accepts: (q) => q.startsWith("http") })];
    const flat = flattenSections(buildSections([...both, ...catalog], "http"));
    expect(flat.map((s) => s.item.id)).toEqual(["url", "http-tool"]);
    flat.forEach((s, i) => expect(s.index).toBe(i));
  });

  it("空分区列表展平为空数组", () => {
    expect(flattenSections([])).toEqual([]);
  });
});
