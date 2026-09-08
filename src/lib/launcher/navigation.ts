// 磁贴网格的方向键导航:先把分区切成行,再在行矩阵上计算移动后的全局下标。
// 纯函数,不依赖 Vue;composable 层只负责持有 selectedIndex 并调用这里。
import type { Section } from "@/lib/launcher/search";

export type Direction = "left" | "right" | "up" | "down";

/**
 * 把分区切成行,每行最多 columns 个,元素是全局下标。
 * 每个分区各自分行、不同分区不共行——与视觉上每个分区独立一个 grid 的布局一致,
 * 这样上下移动时才能落到屏幕上真正相邻的那一行。
 */
export function chunkRows(sections: Section[], columns: number): number[][] {
  const rows: number[][] = [];
  for (const section of sections) {
    const indices = section.items.map((stamped) => stamped.index);
    for (let start = 0; start < indices.length; start += columns) {
      rows.push(indices.slice(start, start + columns));
    }
  }
  return rows;
}

/** 找到 index 所在的行号与列号;不在任何行返回 null */
function locate(index: number, rows: number[][]): { row: number; col: number } | null {
  for (let row = 0; row < rows.length; row++) {
    const col = rows[row]?.indexOf(index) ?? -1;
    if (col !== -1) return { row, col };
  }
  return null;
}

/** 在 0..length-1 范围内做回绕的 ±1 */
function wrap(value: number, length: number): number {
  return (value + length) % length;
}

/**
 * 计算方向键按下后的全局下标。
 * - rows 为空:返回 -1(无可选条目)。
 * - current 不在任何行(如搜索后原选中项被过滤掉):回到 0。
 * - left / right:在展平序列上 ±1,首尾回绕(跨行)。
 * - up / down:在行矩阵上 ±1 行,顶底回绕;列号就近保留,目标行更短时贴行尾。
 */
export function nextIndex(dir: Direction, current: number, rows: number[][]): number {
  if (rows.length === 0) return -1;
  const position = locate(current, rows);
  if (position === null) return 0;

  const flat = rows.flat();
  switch (dir) {
    case "left":
      return flat[wrap(flat.indexOf(current) - 1, flat.length)] ?? 0;
    case "right":
      return flat[wrap(flat.indexOf(current) + 1, flat.length)] ?? 0;
    case "up":
    case "down": {
      const targetRow = rows[wrap(position.row + (dir === "up" ? -1 : 1), rows.length)] ?? [];
      return targetRow[Math.min(position.col, targetRow.length - 1)] ?? 0;
    }
    default:
      // 新增 Direction 而没加 case 时,这行编译报错
      return dir satisfies never;
  }
}
