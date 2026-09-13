// 剪贴板工具页的状态拥有者:列表 / 筛选 / 选中 / 展开 / 分页,以及对后端命令与事件的编排。
// 三条套路(design §1–§2):
// - 列表请求只有一个入口 requestList():loading / 错误 / 过期丢弃都在那里;整表重拉时 generation +1,在途的旧请求回来对不上号即整体忽略;
// - 选中以 id 持有(selectedId),下标派生:置顶 / 删除 / 重排后选中跟着条目走,不需要修正下标;
// - 自己发起的删除 / 收藏改本地;监听器事件 `clipboard://changed` 带着条目来,属于当前视图就本地置顶,
//   有搜索词时不在前端复刻后端 LIKE 语义,只当失效信号防抖重拉;筛选变化整表重拉;唤起时只比首条 id 决定要不要重置。
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

/** 每页条数;exhausted 的唯一判据是「响应是否不满一页」 */
export const PAGE_SIZE = 100;
/** 整表重拉的防抖:连续输入搜索词、粘贴写回 + 回捕连发事件,都只拉最后一次 */
const REFRESH_DEBOUNCE_MS = 150;
/** Ctrl+←/→ 循环切换的顺序;null 即「全部」 */
const KIND_CYCLE: Array<ClipboardKind | null> = [null, "text", "image", "files"];

export function useClipboardHistory(query: MaybeRefOrGetter<string>) {
  /** 当前搜索关键字(首尾空白不参与匹配,与后端 LIKE 语义一致) */
  const trimmedQuery = computed(() => toValue(query).trim());

  /** 已加载的条目,与后端 (copiedAt DESC, id DESC) 全序一致;整表重置替换、翻页追加、本地变更就地改 */
  const items = ref<ClipboardItem[]>([]);
  /** 分类 Tab;null = 全部 */
  const kind = ref<ClipboardKind | null>(null);
  /** 「只看收藏」筛选,与 kind / query 叠加 */
  const favoriteOnly = ref(false);
  /** 选中条目的 id;列表置顶 / 删除后选中跟随条目本身,不随下标漂移;列表为空时 null */
  const selectedId = ref<number | null>(null);
  /** 当前展开的条目 id;与 selectedId 独立,同时只有一项展开;整表重置时清空 */
  const expandedId = ref<number | null>(null);
  /** 是否有一次 list 请求在途(首屏 / 重拉 / 翻页);翻页靠它串行 */
  const loading = ref(false);
  /** 上一页是否不满一页(已到底);只在响应到达时写,本地增删不推它 */
  const exhausted = ref(false);
  /** 最近一次请求 / 命令失败的文案(Rust 已是完整中文句子);下一次成功清空 */
  const error = ref("");
  /** 整表重置计数;页面 watch 它把列表滚回顶部(普通翻页 / 置顶 / 唤起 sync 命中不推) */
  const resetTick = ref(0);

  /** 选中项下标,由 id 派生;选中项不在列表里(理论上只在过渡态)为 -1 */
  const selectedIndex = computed(() => items.value.findIndex((i) => i.id === selectedId.value));
  /** 当前选中项;items 为空时 undefined */
  const selected = computed<ClipboardItem | undefined>(() => items.value[selectedIndex.value]);

  /** 列表版本号:整表重拉时 +1,翻页不加;请求出发时记下,回来对不上就丢,防止旧搜索 / 旧翻页盖住新结果 */
  let generation = 0;
  /** 防抖句柄;搜索词变化与「有搜索词时收到事件」共用,卸载时清掉 */
  let refreshTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * 列表请求的统一入口:loading、错误、过期丢弃都在这里处理;调用方只声明差异。
   * 过期(发出后又整表重拉过)的响应整体忽略,连 loading 也不动,由最新请求自己收尾。
   */
  async function requestList(options: {
    /** true = 整表重拉,版本号 +1,在途的旧请求回来后作废 */
    invalidate?: boolean;
    /** 翻页游标;缺省拉首页 */
    before?: ListCursor;
    /** 把本次结果合入列表 */
    apply: (page: ClipboardItem[]) => void;
  }): Promise<void> {
    const gen = options.invalidate ? ++generation : generation;
    loading.value = true;
    try {
      const page = await listClipboardItems({
        kind: kind.value ?? undefined,
        favoriteOnly: favoriteOnly.value,
        query: trimmedQuery.value,
        before: options.before,
        limit: PAGE_SIZE,
      });
      if (gen !== generation) return;
      error.value = "";
      options.apply(page);
    } catch (e) {
      if (gen !== generation) return;
      console.error("拉取剪贴板历史失败:", e);
      error.value = String(e);
    } finally {
      if (gen === generation) loading.value = false;
    }
  }

  /** 用首页结果整体重置:回到顶部、选中首项、收起展开(旧的展开位置不再成立) */
  function applyReset(page: ClipboardItem[]): void {
    items.value = page;
    exhausted.value = page.length < PAGE_SIZE;
    selectedId.value = page[0]?.id ?? null;
    expandedId.value = null;
    resetTick.value++;
  }

  /**
   * 拉首页。reset = 无条件重置(挂载 / 筛选变化 / 搜索词变化);
   * sync = 只比首条 id:对不上说明隐藏期间漏了事件才重置,对得上滚动、选中、已加载分页全留着。
   */
  function refreshList(mode: "reset" | "sync"): Promise<void> {
    clearTimeout(refreshTimer);
    return requestList({
      invalidate: true,
      apply: (page) => {
        if (mode === "reset" || page[0]?.id !== items.value[0]?.id) applyReset(page);
      },
    });
  }

  /** 无条件从头重拉 */
  function refresh(): Promise<void> {
    return refreshList("reset");
  }

  /** 防抖后整表重拉;期间再次调用只顺延 */
  function scheduleRefresh(): void {
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => void refresh(), REFRESH_DEBOUNCE_MS);
  }

  /**
   * 追加下一页。游标不存状态,从当前末条现取:本地删掉末条后自动退到新末条,置顶也不影响尾部锚点。
   * 在途 / 已到底 / 列表为空时不发;失败后不自动重试,用户再滚动一次即重试。
   */
  function loadMore(): Promise<void> {
    const last = items.value[items.value.length - 1];
    if (loading.value || exhausted.value || last === undefined) return Promise.resolve();
    return requestList({
      before: { copiedAt: last.copiedAt, id: last.id },
      apply: (page) => {
        exhausted.value = page.length < PAGE_SIZE;
        // 在途期间事件可能已把本页某条置顶(重复复制),响应是移动前的快照,按 id 去重
        const known = new Set(items.value.map((item) => item.id));
        items.value.push(...page.filter((item) => !known.has(item.id)));
      },
    });
  }

  /** 粘贴:不动列表——面板随即隐藏,写回被监听器回捕后以同 id 的事件回来置顶 */
  async function paste(id: number): Promise<void> {
    try {
      await pasteClipboardItem(id);
      error.value = "";
    } catch (e) {
      console.error("粘贴剪贴板条目失败:", e);
      error.value = String(e);
    }
  }

  /** 从本地列表移除一条;移除的是选中项时就近改选同位置的下一条(末条则上一条),是展开项则收起 */
  function dropItem(id: number): void {
    const index = items.value.findIndex((item) => item.id === id);
    if (index === -1) return;
    items.value.splice(index, 1);
    if (selectedId.value === id) {
      selectedId.value = items.value[Math.min(index, items.value.length - 1)]?.id ?? null;
    }
    if (expandedId.value === id) expandedId.value = null;
  }

  /** 删除:成功后本地移除,不重拉 */
  async function remove(id: number): Promise<void> {
    try {
      await deleteClipboardItem(id);
      error.value = "";
      dropItem(id);
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
      if (favoriteOnly.value && !next) dropItem(id);
    } catch (e) {
      console.error("设置收藏失败:", e);
      error.value = String(e);
    }
  }

  /** 切换「只看收藏」;筛选变了本地数据不再成立,整表重拉 */
  function toggleFavoriteOnly(): Promise<void> {
    favoriteOnly.value = !favoriteOnly.value;
    return refresh();
  }

  /** 切换分类 Tab;相同分类不重复拉 */
  function setKind(next: ClipboardKind | null): Promise<void> {
    if (next === kind.value) return Promise.resolve();
    kind.value = next;
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
   * ↑/↓ 移动选中:顶底停住,不回绕。末条按 ↓ 且还有下一页时追加下一页并停在原位,
   * 让键盘用户不会卡在第 100 条。
   */
  function moveSelection(delta: 1 | -1): void {
    const length = items.value.length;
    if (length === 0) return;
    // 选中项不在列表里(过渡态)时任何方向都先落回首项
    if (selectedIndex.value === -1) {
      selectedId.value = items.value[0]?.id ?? null;
      return;
    }
    const next = selectedIndex.value + delta;
    if (next >= length) {
      void loadMore();
      return;
    }
    if (next < 0) return;
    selectedId.value = items.value[next]?.id ?? null;
  }

  // ---- 监听器事件:条目属于当前视图就本地合并,不整表重拉 ----

  /** 条目是否属于当前 kind / favoriteOnly 视图;关键字语义留给后端 */
  function matchesFilters(item: ClipboardItem): boolean {
    if (kind.value !== null && item.kind !== kind.value) return false;
    if (favoriteOnly.value && !item.favorite) return false;
    return true;
  }

  /** 把条目放到列表顶部(已在列表中则先移除旧位置),镜像后端的 copiedAt 排序;选中 / 展开按 id 自动跟随;无选中时选中该项 */
  function placeOnTop(item: ClipboardItem): void {
    const index = items.value.findIndex((i) => i.id === item.id);
    if (index !== -1) items.value.splice(index, 1);
    items.value.unshift(item);
    if (selectedId.value === null || selectedIndex.value === -1) {
      selectedId.value = item.id;
    }
  }

  /**
   * 监听器录入 / 上浮:不属于当前视图的忽略(切筛选时会整表重拉);
   * 有搜索词时不在前端复刻后端的 LIKE 匹配,只当失效信号防抖重拉;否则直接置顶。
   */
  function handleRecorded(item: ClipboardItem): void {
    if (!matchesFilters(item)) return;
    if (trimmedQuery.value !== "") {
      scheduleRefresh();
      return;
    }
    placeOnTop(item);
  }

  useTauriEvent(EVENTS.CLIPBOARD_CHANGED, handleRecorded);
  // 隐藏期间列表由事件维护;唤起只比首条 id,对得上不重置(兜底 emit 失败 / 事件与重拉的竞态)
  useTauriEvent(EVENTS.LAUNCHER_OPENED, () => void refreshList("sync"));
  watch(trimmedQuery, scheduleRefresh);

  onMounted(() => void refresh());
  onUnmounted(() => {
    clearTimeout(refreshTimer);
    // 让卸载后到达的响应全部作废(含 finally),不再写任何状态
    generation++;
  });

  return {
    items,
    kind,
    favoriteOnly,
    selectedIndex,
    selected,
    expandedId,
    loading,
    exhausted,
    error,
    resetTick,
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
  };
}
