// 使用真实 Vue 响应式与挂载/卸载生命周期,只替换 IPC 边界(命令与事件订阅);不读取系统剪贴板。
import { createRenderer, nextTick, ref, type Ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTauriEvent } from "@/composables/useTauriEvent";
import {
  deleteClipboardItem,
  listClipboardItems,
  setClipboardItemFavorite,
} from "@/lib/api/clipboard";
import { EVENTS } from "@/lib/events";
import type { ClipboardItem } from "@/types/clipboard";
import { PAGE_SIZE, useClipboardHistory } from "./useClipboardHistory";

vi.mock("@/lib/api/clipboard", () => ({
  listClipboardItems: vi.fn(),
  deleteClipboardItem: vi.fn(),
  pasteClipboardItem: vi.fn(),
  setClipboardItemFavorite: vi.fn(),
}));
vi.mock("@/composables/useTauriEvent", () => ({ useTauriEvent: vi.fn() }));

// 最小组件只渲染注释节点,让 composable 的真实生命周期运行,不需要 DOM 环境。
const { createApp } = createRenderer<object, object>({
  createElement: () => ({}),
  createText: () => ({}),
  createComment: () => ({}),
  insert: () => {},
  remove: () => {},
  setText: () => {},
  setElementText: () => {},
  parentNode: () => null,
  nextSibling: () => null,
  patchProp: () => {},
});

type History = ReturnType<typeof useClipboardHistory>;
interface PendingRequest {
  resolve: (items: ClipboardItem[]) => void;
  reject: (error: Error) => void;
}

/** 按发出顺序排队的 list 请求;测试用 settle(index, …) 决定哪一个先回来 */
const requests: PendingRequest[] = [];
const cleanups: Array<() => void> = [];
/** 按事件名收集的订阅回调;测试用它模拟后端 emit */
const handlers = new Map<string, Array<(payload: unknown) => void>>();

function mountHistory(query: Ref<string> = ref("")) {
  const captured: History[] = [];
  const app = createApp({
    setup() {
      captured.push(useClipboardHistory(query));
      return () => null;
    },
  });
  app.mount({});
  cleanups.push(() => app.unmount());
  const history = captured[0];
  if (history === undefined) throw new Error("测试组件未初始化");
  return history;
}

function emit(name: string, payload: unknown = null): void {
  (handlers.get(name) ?? []).forEach((handler) => handler(payload));
}

function textItem(id: number, overrides: Partial<ClipboardItem> = {}): ClipboardItem {
  return {
    id,
    copiedAt: id,
    favorite: false,
    kind: "text",
    preview: "测试",
    size: 6,
    charCount: 2,
    truncated: false,
    ...overrides,
  } as ClipboardItem;
}

/** 从 start 往下递减 id 的一页;copiedAt = id,天然满足倒序 */
function page(start = 1000, length = PAGE_SIZE): ClipboardItem[] {
  return Array.from({ length }, (_, index) => textItem(start - index));
}

function ids(history: History): number[] {
  return history.items.value.map((item) => item.id);
}

async function settle(index: number, result: ClipboardItem[] | Error): Promise<void> {
  const request = requests[index];
  if (request === undefined) throw new Error(`第 ${index + 1} 次请求尚未发出`);
  if (result instanceof Error) request.reject(result);
  else request.resolve(result);
  await nextTick();
  await nextTick();
}

/** 挂载并让首屏落地,大多数用例的起点 */
async function mountLoaded(first = page(), query: Ref<string> = ref("")) {
  const history = mountHistory(query);
  await settle(0, first);
  return history;
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  vi.spyOn(console, "error").mockImplementation(() => {});
  requests.length = 0;
  handlers.clear();
  vi.mocked(listClipboardItems).mockImplementation(
    () => new Promise((resolve, reject) => requests.push({ resolve, reject })),
  );
  vi.mocked(useTauriEvent).mockImplementation((name, handler) => {
    const list = handlers.get(name) ?? [];
    list.push(handler as (payload: unknown) => void);
    handlers.set(name, list);
  });
});

afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useClipboardHistory 首屏与分页", () => {
  it("挂载拉首屏并选中首项;满页未到底,短页到底", async () => {
    const history = mountHistory();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({
        before: undefined,
        query: "",
        favoriteOnly: false,
        limit: PAGE_SIZE,
      }),
    );
    expect(history.loading.value).toBe(true);
    expect(history.selected.value).toBeUndefined();

    await settle(0, page());
    expect(history.loading.value).toBe(false);
    expect(history.exhausted.value).toBe(false);
    expect(history.selectedIndex.value).toBe(0);
    expect(history.selected.value?.id).toBe(1000);
    expect(history.resetTick.value).toBe(1);

    void history.refresh();
    await settle(1, page(2000, 3));
    expect(history.exhausted.value).toBe(true);
    expect(ids(history)).toEqual([2000, 1999, 1998]);
    expect(history.resetTick.value).toBe(2);
  });

  it("挂载首页失败保留错误文案,空列表不翻页也不自动重试", async () => {
    const history = mountHistory();
    await settle(0, new Error("首屏失败"));
    expect(history.items.value).toEqual([]);
    expect(history.selected.value).toBeUndefined();
    expect(history.expandedId.value).toBe(null);
    expect(history.exhausted.value).toBe(true);
    expect(history.loading.value).toBe(false);
    expect(history.error.value).toContain("首屏失败");

    await history.loadMore();
    await vi.advanceTimersByTimeAsync(1000);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
  });

  it("翻页用末条作游标、按 id 去重;在途 / 到底 / 空列表不发请求", async () => {
    const history = mountHistory();
    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    await settle(0, page());

    const loading = history.loadMore();
    expect(history.loading.value).toBe(true);
    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 901, id: 901 } }),
    );
    // 响应里夹带一条本地已有的(在途期间被置顶过的)条目,不能重复追加
    await settle(1, [textItem(1000), ...page(900, 2)]);
    await loading;
    expect(ids(history)).toEqual([...page().map((i) => i.id), 900, 899]);
    expect(history.exhausted.value).toBe(true);
    expect(history.resetTick.value).toBe(1);

    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
  });

  it("本地删掉末条后翻页用新末条作游标,exhausted 不受本地增删影响", async () => {
    const history = await mountLoaded();
    await history.remove(901);
    expect(history.items.value).toHaveLength(PAGE_SIZE - 1);
    expect(history.exhausted.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 902, id: 902 } }),
    );
  });

  it("翻页失败保留列表并写错误文案,不自动重试,再次调用可重试", async () => {
    const history = await mountLoaded();
    void history.loadMore();
    await settle(1, new Error("分页失败"));
    expect(history.items.value).toHaveLength(PAGE_SIZE);
    expect(history.error.value).toContain("分页失败");
    expect(history.loading.value).toBe(false);
    await vi.advanceTimersByTimeAsync(1000);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);

    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
    await settle(2, page(900, 1));
    expect(history.error.value).toBe("");
    expect(history.items.value).toHaveLength(PAGE_SIZE + 1);
  });
});

describe("useClipboardHistory 过期丢弃", () => {
  it("翻页在途时切分类:翻页响应整体忽略,loading 由重拉收尾", async () => {
    const history = await mountLoaded();
    void history.loadMore();
    void history.setKind("text");
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ kind: "text", before: undefined }),
    );

    await settle(1, page(900));
    expect(history.loading.value).toBe(true);
    expect(history.items.value).toEqual(page());

    const refreshed = page(2000, 5);
    await settle(2, refreshed);
    expect(history.loading.value).toBe(false);
    expect(history.items.value).toEqual(refreshed);
    expect(history.exhausted.value).toBe(true);
  });

  it.each(["成功", "失败"])("过期%s响应不能覆盖最新重拉的结果或错误", async (outcome) => {
    const history = await mountLoaded();
    void history.refresh();
    void history.toggleFavoriteOnly();
    await settle(2, new Error("最新失败"));
    await settle(1, outcome === "成功" ? page(3000) : new Error("过期失败"));
    expect(history.items.value).toEqual([]);
    expect(history.error.value).toContain("最新失败");
    expect(history.loading.value).toBe(false);
    expect(history.resetTick.value).toBe(2);
  });

  it("过期首页失败不能清掉最新成功结果、选中或展开", async () => {
    const history = await mountLoaded();
    void history.refresh();
    void history.setKind("text");
    await settle(2, page(2000));
    history.moveSelection(1);
    history.toggleExpanded(1998);

    await settle(1, new Error("过期失败"));
    expect(history.items.value).toEqual(page(2000));
    expect(history.selected.value?.id).toBe(1999);
    expect(history.expandedId.value).toBe(1998);
    expect(history.error.value).toBe("");
    expect(history.loading.value).toBe(false);
    expect(history.exhausted.value).toBe(false);
    expect(history.resetTick.value).toBe(2);
  });

  it("卸载后:在途响应不落地,待发防抖不再请求", async () => {
    const query = ref("");
    const history = mountHistory(query);
    query.value = "卸载前搜索";
    await nextTick();
    cleanups.splice(0).forEach((cleanup) => cleanup());
    await settle(0, page());
    await vi.runAllTimersAsync();
    expect(history.items.value).toEqual([]);
    expect(history.resetTick.value).toBe(0);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
  });
});

describe("useClipboardHistory 筛选", () => {
  it.each(["分类", "收藏", "搜索", "显式刷新"])(
    "%s首页失败清空列表、选中与展开,不再翻页或自动重试;后续切筛选成功恢复",
    async (source) => {
      const query = ref("");
      const history = await mountLoaded(page(), query);
      history.moveSelection(1);
      history.toggleExpanded(998);

      if (source === "分类") void history.setKind("image");
      else if (source === "收藏") void history.toggleFavoriteOnly();
      else if (source === "显式刷新") void history.refresh();
      else {
        query.value = "关键词";
        await nextTick();
        await vi.advanceTimersByTimeAsync(150);
      }
      expect(listClipboardItems).toHaveBeenCalledTimes(2);
      expect(listClipboardItems).toHaveBeenLastCalledWith(
        expect.objectContaining({ before: undefined }),
      );
      await settle(1, new Error("首页失败"));
      expect(history.items.value).toEqual([]);
      expect(history.selected.value).toBeUndefined();
      expect(history.selectedIndex.value).toBe(-1);
      expect(history.expandedId.value).toBe(null);
      expect(history.exhausted.value).toBe(true);
      expect(history.resetTick.value).toBe(2);
      expect(history.error.value).toContain("首页失败");
      expect(history.loading.value).toBe(false);

      await history.loadMore();
      await vi.advanceTimersByTimeAsync(1000);
      expect(listClipboardItems).toHaveBeenCalledTimes(2);
      expect(history.error.value).toContain("首页失败");

      void history.setKind("text");
      expect(listClipboardItems).toHaveBeenCalledTimes(3);
      const recovered = page(2000).map((item) => ({ ...item, favorite: true }));
      await settle(2, recovered);
      expect(history.items.value).toEqual(recovered);
      expect(history.selected.value?.id).toBe(2000);
      expect(history.expandedId.value).toBe(null);
      expect(history.exhausted.value).toBe(false);
      expect(history.resetTick.value).toBe(3);
      expect(history.error.value).toBe("");
    },
  );

  it("搜索词连续变化只在防抖后发一次;切分类立刻重拉并取消待发防抖;相同分类不重复拉", async () => {
    const query = ref("");
    const history = await mountLoaded(page(), query);
    query.value = "a";
    await nextTick();
    query.value = "ab";
    await nextTick();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(150);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(expect.objectContaining({ query: "ab" }));

    query.value = "abc";
    await nextTick();
    void history.setKind("image");
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
    await vi.advanceTimersByTimeAsync(150);
    expect(listClipboardItems).toHaveBeenCalledTimes(3);

    void history.setKind("image");
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
  });

  it("搜索词首尾空白被规范化(trim),且纯空格变化不触发无谓刷新", async () => {
    const query = ref("  a  ");
    mountHistory(query);
    expect(listClipboardItems).toHaveBeenCalledWith(expect.objectContaining({ query: "a" }));
    await settle(0, page());

    query.value = "  a    ";
    await nextTick();
    await vi.advanceTimersByTimeAsync(150);
    // trimmed 仍为 "a",不触发额外请求
    expect(listClipboardItems).toHaveBeenCalledTimes(1);

    query.value = "   ";
    await nextTick();
    await vi.advanceTimersByTimeAsync(150);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(expect.objectContaining({ query: "" }));
  });

  it("Ctrl+←/→ 在 全部 → 文本 → 图片 → 文件 间循环", async () => {
    const history = await mountLoaded();
    void history.cycleKind(-1);
    expect(history.kind.value).toBe("files");
    void history.cycleKind(1);
    expect(history.kind.value).toBe(null);
    void history.cycleKind(1);
    expect(history.kind.value).toBe("text");
  });
});

describe("useClipboardHistory 本地变更", () => {
  it("删除选中项就近改选下一条;删末条选上一条;删展开项则收起", async () => {
    const history = await mountLoaded(page(1000, 3));
    history.toggleExpanded(999);
    history.moveSelection(1);
    expect(history.selected.value?.id).toBe(999);

    await history.remove(999);
    expect(deleteClipboardItem).toHaveBeenCalledWith(999);
    expect(ids(history)).toEqual([1000, 998]);
    expect(history.selected.value?.id).toBe(998);
    expect(history.expandedId.value).toBe(null);

    await history.remove(998);
    expect(history.selected.value?.id).toBe(1000);
    await history.remove(1000);
    expect(history.selected.value).toBeUndefined();
    expect(history.selectedIndex.value).toBe(-1);
  });

  it("删除非选中项不改变选中;命令失败不动列表并写错误文案", async () => {
    const history = await mountLoaded(page(1000, 3));
    history.moveSelection(1);
    await history.remove(1000);
    expect(history.selected.value?.id).toBe(999);
    expect(history.selectedIndex.value).toBe(0);

    vi.mocked(deleteClipboardItem).mockRejectedValueOnce(new Error("删除失败"));
    await history.remove(999);
    expect(ids(history)).toEqual([999, 998]);
    expect(history.error.value).toContain("删除失败");
  });

  it("普通视图翻转星标不移除;只看收藏时取消收藏本地移除", async () => {
    const history = await mountLoaded(page(1000, 2));
    await history.toggleFavorite(1000);
    expect(setClipboardItemFavorite).toHaveBeenCalledWith(1000, true);
    expect(history.items.value[0]?.favorite).toBe(true);
    expect(ids(history)).toEqual([1000, 999]);

    void history.toggleFavoriteOnly();
    await settle(1, [textItem(1000, { favorite: true })]);
    await history.toggleFavorite(1000);
    expect(setClipboardItemFavorite).toHaveBeenLastCalledWith(1000, false);
    expect(history.items.value).toEqual([]);
    expect(history.selected.value).toBeUndefined();
  });

  it("↑/↓ 顶底停住不回绕;末条按 ↓ 且未到底时翻页并停在原位", async () => {
    const history = await mountLoaded(page(1000, 3));
    history.moveSelection(-1);
    expect(history.selectedIndex.value).toBe(0);
    history.moveSelection(1);
    history.moveSelection(1);
    history.moveSelection(1);
    expect(history.selectedIndex.value).toBe(2);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);

    void history.refresh();
    await settle(1, page());
    history.moveSelection(-1);
    expect(history.selectedIndex.value).toBe(0);
    for (let i = 0; i < PAGE_SIZE; i++) history.moveSelection(1);
    expect(history.selectedIndex.value).toBe(PAGE_SIZE - 1);
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 901, id: 901 } }),
    );
  });
});

describe("useClipboardHistory 后端事件", () => {
  it("新条目置顶,选中 / 展开 / exhausted / resetTick 不变;同 id 重发只上浮不重复", async () => {
    const history = await mountLoaded(page(1000, 3));
    history.moveSelection(1);
    history.toggleExpanded(998);

    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2000));
    expect(ids(history)).toEqual([2000, 1000, 999, 998]);
    expect(history.selected.value?.id).toBe(999);
    expect(history.expandedId.value).toBe(998);
    expect(history.exhausted.value).toBe(true);
    expect(history.resetTick.value).toBe(1);

    emit(EVENTS.CLIPBOARD_CHANGED, textItem(998, { copiedAt: 3000 }));
    expect(ids(history)).toEqual([998, 2000, 1000, 999]);
    expect(history.items.value[0]?.copiedAt).toBe(3000);
    expect(history.expandedId.value).toBe(998);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
  });

  it("不属于当前 kind / 只看收藏视图的条目忽略", async () => {
    const history = await mountLoaded(page(1000, 2));
    void history.setKind("image");
    await settle(1, []);
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2000));
    expect(history.items.value).toEqual([]);

    void history.setKind(null);
    await settle(2, page(1000, 2));
    void history.toggleFavoriteOnly();
    await settle(3, []);
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2000));
    expect(history.items.value).toEqual([]);
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2001, { favorite: true }));
    expect(ids(history)).toEqual([2001]);
  });

  it("有搜索词时事件只当失效信号,与搜索词防抖合并为一次重拉", async () => {
    const query = ref("关键词");
    const history = await mountLoaded(page(1000, 2), query);
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2000));
    expect(ids(history)).toEqual([1000, 999]);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    query.value = "关键词2";
    await nextTick();
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2001));
    await vi.advanceTimersByTimeAsync(150);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ query: "关键词2", before: undefined }),
    );
  });

  it("空列表收到新条目事件时自动选中该首项", async () => {
    const history = await mountLoaded([]);
    expect(history.selected.value).toBeUndefined();
    emit(EVENTS.CLIPBOARD_CHANGED, textItem(2000));
    expect(ids(history)).toEqual([2000]);
    expect(history.selected.value?.id).toBe(2000);
    expect(history.selectedIndex.value).toBe(0);
  });

  it("唤起 sync 失败保留列表、选中与展开,不自动重试", async () => {
    const history = await mountLoaded();
    history.moveSelection(1);
    history.toggleExpanded(998);
    emit(EVENTS.LAUNCHER_OPENED);
    await settle(1, new Error("同步失败"));

    expect(history.items.value).toEqual(page());
    expect(history.selected.value?.id).toBe(999);
    expect(history.expandedId.value).toBe(998);
    expect(history.resetTick.value).toBe(1);
    expect(history.exhausted.value).toBe(false);
    expect(history.loading.value).toBe(false);
    expect(history.error.value).toContain("同步失败");
    await vi.advanceTimersByTimeAsync(1000);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
  });

  it("唤起 sync:首条相同不重置,首条不同才整表重置", async () => {
    const history = await mountLoaded();
    history.moveSelection(1);
    emit(EVENTS.LAUNCHER_OPENED);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    await settle(1, page());
    expect(history.selected.value?.id).toBe(999);
    expect(history.resetTick.value).toBe(1);

    emit(EVENTS.LAUNCHER_OPENED);
    await settle(2, page(2000));
    expect(history.items.value).toEqual(page(2000));
    expect(history.selected.value?.id).toBe(2000);
    expect(history.resetTick.value).toBe(2);
  });
});
