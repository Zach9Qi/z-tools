// 磁贴网格的选中状态与方向键 / Enter 绑定;下标计算全部交给 lib/launcher/navigation 纯函数。
import { computed, ref, watch } from "vue";
import { useKeymap } from "@/composables/useKeymap";
import { chunkRows, nextIndex, type Direction } from "@/lib/launcher/navigation";
import { flattenSections, type Section, type StampedItem } from "@/lib/launcher/search";

/** 方向键 key → 移动方向;只登记这四个键,其余按键不进入本 composable */
const DIRECTIONS: Record<string, Direction> = {
  ArrowUp: "up",
  ArrowDown: "down",
  ArrowLeft: "left",
  ArrowRight: "right",
};

export function useRowNavigation(options: {
  /** 当前分区列表;用 getter 而非 ref,让调用方传 computed 时不必解包 */
  getSections: () => Section[];
  /** 每行磁贴数,与 grid-cols-* 保持一致,否则上下移动会错行 */
  columns: number;
  /** Enter / 点击激活选中项时的回调 */
  onActivate: (item: StampedItem) => void;
}) {
  const { getSections, columns, onActivate } = options;

  /** 当前选中的全局下标;搜索词变化时回到 0,与 hover / 方向键共同维护 */
  const selectedIndex = ref(0);
  /** 行矩阵(每行装全局下标),随分区变化重算 */
  const rows = computed(() => chunkRows(getSections(), columns));

  function select(index: number): void {
    selectedIndex.value = index;
  }

  function reset(): void {
    selectedIndex.value = 0;
  }

  function move(direction: Direction): void {
    selectedIndex.value = nextIndex(direction, selectedIndex.value, rows.value);
  }

  function activateSelected(): void {
    const target = flattenSections(getSections())[selectedIndex.value];
    if (target !== undefined) onActivate(target);
  }

  useKeymap([
    {
      keys: Object.keys(DIRECTIONS),
      label: "选择",
      onPress: (e) => {
        const direction = DIRECTIONS[e.key];
        if (direction !== undefined) move(direction);
      },
    },
    { keys: ["Enter"], label: "打开", onPress: activateSelected },
  ]);

  // 分区变了(搜索词变化)原下标可能已不存在,统一回到第一项
  watch(getSections, reset);

  return { selectedIndex, select, reset };
}
