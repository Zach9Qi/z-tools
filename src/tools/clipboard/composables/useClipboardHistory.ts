// 剪贴板工具页的状态拥有者:列表 / 筛选 / 选中 / 展开 / 分页,以及对 5 个后端命令的编排。
// 两类变化分开处理(design §4.3):
// - 自己发起的删除 / 收藏 → 命令成功后直接改本地 items,不重拉、不碰 hasMore、不丢分页与滚动位置;
// - 筛选变化(kind / 只看收藏 / 搜索词)与监听器事件 `clipboard://changed` → 从头 refresh(),用递增序号丢弃过期响应。
import { computed, onMounted, onUnmounted, ref, toValue, watch, type MaybeRefOrGetter } from "vue";
import { useTauriEvent } from "@/composables/useTauriEvent";
import {
  deleteClipboardItem,
  listClipboardItems,
  pasteClipboardItem,
  setClipboardItemFavorite,
} from "@/lib/api/clipboard";
import { EVENTS } from "@/lib/events";
import type { ClipboardItem, ClipboardKind, ListCursor } from "@/types/clipboard";

/** 每页条数;hasMore 的唯一判据是「响应是否不满一页」 */
export const PAGE_SIZE = 100;
/** 监听器事件防抖:粘贴写回 + 回捕、连续 Ctrl+C 会在短时间内连发多次 */
const CHANGED_DEBOUNCE_MS = 150;
/** 搜索词防抖:每个字符都重拉太浪费,且 LIKE 全扫是后端成本 */
const QUERY_DEBOUNCE_MS = 200;
/** Ctrl+←/→ 循环切换的顺序;null 即「全部」 */
const KIND_CYCLE: Array<ClipboardKind | null> = [null, "text", "image", "files"];

export function useClipboardHistory(query: MaybeRefOrGetter<string>) {
  /** 已加载的条目,按 copiedAt / id 倒序;refresh 整体替换,loadMore 追加,本地变更就地改 */
  const items = ref<ClipboardItem[]>([]);
  /** 分类 Tab;null = 全部 */
  const kind = ref<ClipboardKind | null>(null);
  /** 「只看收藏」筛选,与 kind / query 叠加 */
  const favoriteOnly = ref(false);
  /** 键盘 / 鼠标选中的下标;items 为空时无意义(模板按 items[selectedIndex] 取值即得 undefined) */
  const selectedIndex = ref(0);
  /** 当前展开的条目 id;与 selectedIndex 独立,同时只有一项展开;refresh 时清空 */
  const expandedId = ref<number | null>(null);
  /** 是否有一次 list 请求在途(refresh 或 loadMore) */
  const loading = ref(false);
  /** 上一页是否满页;只在响应到达时写,本地增删不推它 */
  const hasMore = ref(false);
  /** 最近一次命令失败的文案(Rust 已是完整中文句子);下一次成功清空 */
  const error = ref("");

  /** 当前选中项,由下标派生 */
  const selected = computed<ClipboardItem | undefined>(() => items.value[selectedIndex.value]);

  /** list 请求序号;响应到达时与当前值不等即视为过期丢弃(筛选已变或已被新的 refresh 取代) */
  let requestSeq = 0;

  /** 把 selectedIndex 钳到 items 范围内;items 空时归零 */
  function clampSelection(): void {
    const max = Math.max(items.value.length - 1, 0);
    selectedIndex.value = Math.min(Math.max(selectedIndex.value, 0), max);
  }

  /**
   * 拉一页。`before` 为空 = 从头拉并替换 items;否则追加。
   * 只有序号仍是最新的响应才会写 items / hasMore / loading,过期响应整体忽略(连 loading 也不动,由最新请求收尾)。
   */
  async function load(before: ListCursor | undefined): Promise<void> {
    const seq = ++requestSeq;
    loading.value = true;
    try {
      const page = await listClipboardItems({
        kind: kind.value ?? undefined,
        favoriteOnly: favoriteOnly.value,
        query: toValue(query),
        before,
        limit: PAGE_SIZE,
      });
      if (seq !== requestSeq) return;
      items.value = before === undefined ? page : [...items.value, ...page];
      hasMore.value = page.length >= PAGE_SIZE;
      error.value = "";
      clampSelection();
    } catch (e) {
      if (seq !== requestSeq) return;
      console.error("拉取剪贴板历史失败:", e);
      error.value = String(e);
    } finally {
      if (seq === requestSeq) loading.value = false;
    }
  }

  /** 从头重拉首页;展开项一律收起(列表内容将整体替换,旧的展开位置不再成立) */
  function refresh(): Promise<void> {
    expandedId.value = null;
    return load(undefined);
  }

  /**
   * 追加下一页。游标不存状态,从当前末条现取:本地删掉末条后自动退到新末条,被删行已不在库里不会重复。
   * items 为空退化为 refresh。
   */
  function loadMore(): Promise<void> {
    if (!hasMore.value || loading.value) return Promise.resolve();
    const last = items.value[items.value.length - 1];
    if (last === undefined) return refresh();
    return load({ copiedAt: last.copiedAt, id: last.id });
  }

  /** 粘贴:不动列表——面板随即隐藏,上浮由监听器回捕的事件触发重拉 */
  async function paste(id: number): Promise<void> {
    try {
      await pasteClipboardItem(id);
      error.value = "";
    } catch (e) {
      console.error("粘贴剪贴板条目失败:", e);
      error.value = String(e);
    }
  }

  /** 从本地 items 移除一条并修正选中 / 展开;删除与「取消收藏且只看收藏」共用 */
  function removeLocal(id: number): void {
    const index = items.value.findIndex((item) => item.id === id);
    if (index === -1) return;
    items.value.splice(index, 1);
    // 删的在选中项之前:选中项整体前移一格才仍指向同一条;删的正是选中项:下标不变即落到相邻的下一条,末条则钳回
    if (index < selectedIndex.value) selectedIndex.value--;
    clampSelection();
    if (expandedId.value === id) expandedId.value = null;
  }

  /** 删除:成功后本地 splice,不重拉 */
  async function remove(id: number): Promise<void> {
    try {
      await deleteClipboardItem(id);
      error.value = "";
      removeLocal(id);
    } catch (e) {
      console.error("删除剪贴板条目失败:", e);
      error.value = String(e);
    }
  }

  /** 切换收藏:成功后本地翻转;只看收藏时取消收藏的条目不再满足筛选,本地移除 */
  async function toggleFavorite(id: number): Promise<void> {
    const item = items.value.find((i) => i.id === id);
    if (item === undefined) return;
    const next = !item.favorite;
    try {
      await setClipboardItemFavorite(id, next);
      error.value = "";
      item.favorite = next;
      if (favoriteOnly.value && !next) removeLocal(id);
    } catch (e) {
      console.error("设置收藏失败:", e);
      error.value = String(e);
    }
  }

  /** 切换「只看收藏」;筛选变了本地数据不再成立,回到首项并重拉 */
  function toggleFavoriteOnly(): Promise<void> {
    favoriteOnly.value = !favoriteOnly.value;
    selectedIndex.value = 0;
    return refresh();
  }

  /** 切换分类 Tab;相同分类不重复拉 */
  function setKind(next: ClipboardKind | null): Promise<void> {
    if (next === kind.value) return Promise.resolve();
    kind.value = next;
    selectedIndex.value = 0;
    return refresh();
  }

  /** Ctrl+←/→:在 全部 → 文本 → 图片 → 文件 间循环 */
  function cycleKind(delta: 1 | -1): Promise<void> {
    const current = KIND_CYCLE.indexOf(kind.value);
    const next = KIND_CYCLE[(current + delta + KIND_CYCLE.length) % KIND_CYCLE.length];
    return setKind(next ?? null);
  }

  /** 展开 / 收起:相同 id 收起,不同 id 替换(同时只有一项展开) */
  function toggleExpanded(id: number): void {
    expandedId.value = expandedId.value === id ? null : id;
  }

  /**
   * ↑/↓ 移动选中:两端回绕(与主页网格一致);但在末条按 ↓ 且还有下一页时改为追加下一页并停在原位,
   * 让键盘用户不会卡在第 100 条,也不会在还有数据时跳回顶部。
   */
  function moveSelection(delta: 1 | -1): void {
    const length = items.value.length;
    if (length === 0) return;
    const next = selectedIndex.value + delta;
    if (next >= length) {
      if (hasMore.value) {
        void loadMore();
        return;
      }
      selectedIndex.value = 0;
      return;
    }
    selectedIndex.value = next < 0 ? length - 1 : next;
  }

  /** 选中某个下标(鼠标悬停) */
  function select(index: number): void {
    selectedIndex.value = index;
  }

  // ---- 触发重拉的两个外部来源:监听器事件、搜索词 ----

  /** 两个防抖定时器;卸载时清掉,避免组件销毁后还去 refresh */
  let changedTimer: ReturnType<typeof setTimeout> | undefined;
  let queryTimer: ReturnType<typeof setTimeout> | undefined;

  useTauriEvent(EVENTS.CLIPBOARD_CHANGED, () => {
    clearTimeout(changedTimer);
    changedTimer = setTimeout(() => void refresh(), CHANGED_DEBOUNCE_MS);
  });

  watch(
    () => toValue(query),
    () => {
      clearTimeout(queryTimer);
      queryTimer = setTimeout(() => {
        selectedIndex.value = 0;
        void refresh();
      }, QUERY_DEBOUNCE_MS);
    },
  );

  onMounted(() => void refresh());
  onUnmounted(() => {
    clearTimeout(changedTimer);
    clearTimeout(queryTimer);
    // 让在途响应到达时被当作过期丢弃
    requestSeq++;
  });

  return {
    items,
    kind,
    favoriteOnly,
    selectedIndex,
    selected,
    expandedId,
    loading,
    hasMore,
    error,
    refresh,
    loadMore,
    paste,
    remove,
    toggleFavorite,
    toggleFavoriteOnly,
    setKind,
    cycleKind,
    toggleExpanded,
    moveSelection,
    select,
  };
}
