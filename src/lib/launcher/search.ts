// 启动器搜索分区:把 catalog 按搜索词切成若干分区并给每个条目编全局下标。
// 纯函数,不依赖 Vue;组件层只负责渲染这里的结果。
import type { ToolItem } from "@/types/tool";

/** 分区标识:all = 主页无搜索词;matches = accepts 动态命中;named = title / keywords 子串命中 */
export type SectionId = "all" | "matches" | "named";

/** 带全局下标的条目;index 是跨分区展平后的位置,方向键导航与选中高亮都用它 */
export interface StampedItem {
  item: ToolItem;
  index: number;
  /** 所属分区;面板据此判断是否从「匹配结果」激活(要把搜索词带进工具页) */
  sectionId: SectionId;
}

/** 结果区的一个分区 */
export interface Section {
  id: SectionId;
  /** 分区标题行文案 */
  title: string;
  items: StampedItem[];
}

/** 未编号的分区;stampSections 之后才成为 Section */
interface RawSection {
  id: Section["id"];
  title: string;
  items: ToolItem[];
}

/** title 或任一 keyword 的小写形式包含 query(query 已小写) */
function matchesName(item: ToolItem, query: string): boolean {
  if (item.title.toLowerCase().includes(query)) return true;
  return item.keywords.some((keyword) => keyword.toLowerCase().includes(query));
}

/** 按分区顺序给条目编连续的全局下标,同时剔除空分区 */
function stampSections(raw: RawSection[]): Section[] {
  let next = 0;
  const sections: Section[] = [];
  for (const section of raw) {
    if (section.items.length === 0) continue;
    const items = section.items.map((item) => ({ item, index: next++, sectionId: section.id }));
    sections.push({ id: section.id, title: section.title, items });
  }
  return sections;
}

/**
 * 根据搜索词把 catalog 切成分区。
 * - 空(trim 后)搜索词:单一「全部工具」分区,顺序即 catalog 顺序。
 * - 有搜索词:「匹配结果」(accepts 命中且**未**命中名称)排在「搜索结果」(名称命中)之前;
 *   名称命中优先归入「搜索结果」,避免同一工具在两个分区各出现一次。
 * - 空分区不返回;两者皆空返回 `[]`,由组件层渲染空态。
 */
export function buildSections(catalog: ToolItem[], query: string): Section[] {
  const q = query.trim().toLowerCase();
  if (q === "") {
    return stampSections([{ id: "all", title: "全部工具", items: catalog }]);
  }

  const named: ToolItem[] = [];
  const matches: ToolItem[] = [];
  for (const item of catalog) {
    if (matchesName(item, q)) {
      named.push(item);
    } else if (item.accepts?.(q)) {
      matches.push(item);
    }
  }
  return stampSections([
    { id: "matches", title: "匹配结果", items: matches },
    { id: "named", title: "搜索结果", items: named },
  ]);
}

/** 按分区顺序展平为一维数组;结果的数组位置与每项的 index 一致 */
export function flattenSections(sections: Section[]): StampedItem[] {
  return sections.flatMap((section) => section.items);
}
