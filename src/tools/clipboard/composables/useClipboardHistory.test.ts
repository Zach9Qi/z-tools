// 使用真实 Vue 响应式与挂载/卸载生命周期,只替换 IPC 边界;不读取系统剪贴板。
import { createRenderer, nextTick, ref, type Ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useTauriEvent } from "@/composables/useTauriEvent";
import {
  deleteClipboardItem,
  listClipboardItems,
  setClipboardItemFavorite,
} from "@/lib/api/clipboard";
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

const requests: PendingRequest[] = [];
const cleanups: Array<() => void> = [];
const changedHandlers: Array<() => void> = [];

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

function page(start = 1000, length = PAGE_SIZE): ClipboardItem[] {
  return Array.from({ length }, (_, index) => ({
    id: start - index,
    copiedAt: start - index,
    favorite: false,
    kind: "text",
    preview: "测试",
    size: 6,
    charCount: 2,
    truncated: false,
  }));
}

async function settle(index: number, result: ClipboardItem[] | Error): Promise<void> {
  const request = requests[index];
  if (request === undefined) throw new Error(`第 ${index + 1} 次请求尚未发出`);
  if (result instanceof Error) request.reject(result);
  else request.resolve(result);
  await nextTick();
  await nextTick();
}

beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  vi.spyOn(console, "error").mockImplementation(() => {});
  requests.length = 0;
  changedHandlers.length = 0;
  vi.mocked(listClipboardItems).mockImplementation(
    () => new Promise((resolve, reject) => requests.push({ resolve, reject })),
  );
  vi.mocked(useTauriEvent).mockImplementation((_name, handler) => {
    changedHandlers.push(() => handler(null));
  });
});

afterEach(() => {
  cleanups.splice(0).forEach((cleanup) => cleanup());
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useClipboardHistory 分页", () => {
  it("挂载请求首屏,在途不重复分页,短页成功后停止", async () => {
    const history = mountHistory();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    expect(history).not.toHaveProperty("loadState");
    expect(history.items.value).toEqual([]);
    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);

    const first = page();
    await settle(0, first);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    expect(history.hasMore.value).toBe(true);
    const loadingMore = history.loadMore();
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 901, id: 901 }, limit: PAGE_SIZE }),
    );
    const last = page(900, 2);
    await settle(1, last);
    await loadingMore;
    expect(history.items.value).toEqual([...first, ...last]);
    expect(history.hasMore.value).toBe(false);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    await history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
  });

  it("分页失败保留列表和 hasMore,哨兵与键盘入口都不能重试", async () => {
    const history = mountHistory();
    const first = page();
    await settle(0, first);
    const loadingMore = history.loadMore();
    await settle(1, new Error("分页失败"));
    await loadingMore;
    expect(history.items.value).toEqual(first);
    expect(history.hasMore.value).toBe(true);
    expect(history.loading.value).toBe(false);
    expect(history.error.value).toContain("分页失败");
    expect(history.canLoadMore.value).toBe(false);

    // 不等待误发请求,用请求次数直接检查拒绝分页,避免错误实现让测试挂住。
    void history.loadMore();
    history.select(PAGE_SIZE - 1);
    history.moveSelection(1);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
  });

  it("收藏成功清空共享错误文案也不会解除分页失败状态", async () => {
    const history = mountHistory();
    await settle(0, page());
    void history.loadMore();
    await settle(1, new Error("分页失败"));
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    await history.toggleFavorite(1000);
    expect(setClipboardItemFavorite).toHaveBeenCalledWith(1000, true);
    expect(history.items.value[0]?.favorite).toBe(true);
    expect(history.error.value).toBe("");
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(history.hasMore.value).toBe(true);
  });

  it("列表成功后收藏命令失败不会阻止分页", async () => {
    const history = mountHistory();
    await settle(0, page());
    vi.mocked(setClipboardItemFavorite).mockRejectedValueOnce(new Error("收藏失败"));
    await history.toggleFavorite(1000);
    expect(history.error.value).toContain("收藏失败");
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    expect(history.items.value[0]?.favorite).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    await settle(1, []);
    expect(history.hasMore.value).toBe(false);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
  });

  it("刷新失败保留旧数据,只有后续刷新成功才恢复分页", async () => {
    const history = mountHistory();
    const first = page();
    await settle(0, first);
    expect(history.canLoadMore.value).toBe(true);
    void history.refresh();
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    await settle(1, new Error("刷新失败"));
    expect(history.items.value).toEqual(first);
    expect(history.hasMore.value).toBe(true);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);

    void history.refresh();
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
    const refreshed = page(2000);
    await settle(2, refreshed);
    expect(history.items.value).toEqual(refreshed);
    expect(history.error.value).toBe("");
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(4);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 1901, id: 1901 } }),
    );
  });

  it.each(["搜索", "分类", "后端事件"])("首屏失败后%s仍可刷新并恢复分页", async (source) => {
    const query = ref("");
    const history = mountHistory(query);
    await settle(0, new Error("首屏失败"));
    expect(history.loading.value).toBe(false);
    expect(history.hasMore.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);

    if (source === "搜索") {
      query.value = "新关键词";
      await nextTick();
      await vi.advanceTimersByTimeAsync(200);
    } else if (source === "分类") {
      void history.setKind("text");
    } else {
      changedHandlers.forEach((handler) => handler());
      await vi.advanceTimersByTimeAsync(150);
    }
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({
        before: undefined,
        query: query.value,
        kind: history.kind.value ?? undefined,
      }),
    );
    await settle(1, page());
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
  });

  it("成功时保留键盘分页入口,本地删除末条不改变 hasMore 且使用新末条游标", async () => {
    const history = mountHistory();
    await settle(0, page());
    await history.remove(901);
    expect(deleteClipboardItem).toHaveBeenCalledWith(901);
    expect(history.items.value).toHaveLength(PAGE_SIZE - 1);
    expect(history.hasMore.value).toBe(true);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    history.select(PAGE_SIZE - 2);
    history.moveSelection(1);
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    expect(listClipboardItems).toHaveBeenLastCalledWith(
      expect.objectContaining({ before: { copiedAt: 902, id: 902 } }),
    );
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    await settle(1, []);
    expect(history.hasMore.value).toBe(false);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
  });

  it.each(["成功", "失败"])("过期%s响应不能结束最新在途请求或覆盖列表", async (outcome) => {
    const history = mountHistory();
    void history.refresh();
    await settle(0, outcome === "成功" ? page() : new Error("过期失败"));
    expect(history.loading.value).toBe(true);
    expect(history.canLoadMore.value).toBe(false);
    expect(history.items.value).toEqual([]);
    expect(history.hasMore.value).toBe(false);
    expect(history.error.value).toBe("");
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
    await settle(1, page(2000, 1));
    expect(history.loading.value).toBe(false);
    expect(history.items.value).toEqual(page(2000, 1));
    expect(history.hasMore.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(2);
  });

  it("旧分页成功不能覆盖新刷新失败或重新开放分页", async () => {
    const history = mountHistory();
    const first = page();
    await settle(0, first);
    void history.loadMore();
    void history.refresh();
    await settle(2, new Error("最新刷新失败"));
    await settle(1, page(900));
    expect(history.items.value).toEqual(first);
    expect(history.error.value).toContain("最新刷新失败");
    expect(history.canLoadMore.value).toBe(false);
    expect(history.hasMore.value).toBe(true);
    expect(history.loading.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(3);
  });

  it("旧分页失败不能污染新刷新成功或阻止后续分页", async () => {
    const history = mountHistory();
    await settle(0, page());
    void history.loadMore();
    void history.refresh();
    const refreshed = page(2000);
    await settle(2, refreshed);
    await settle(1, new Error("过期分页失败"));
    expect(history.items.value).toEqual(refreshed);
    expect(history.error.value).toBe("");
    expect(history.hasMore.value).toBe(true);
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(true);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(4);
  });

  it("成功后卸载,已排队的分页入口也不能再发请求", async () => {
    const history = mountHistory();
    await settle(0, page());
    expect(history.canLoadMore.value).toBe(true);
    cleanups.splice(0).forEach((cleanup) => cleanup());
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    void history.loadMore();
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
    expect(history.hasMore.value).toBe(true);
  });

  it("卸载后的响应不提交列表,搜索与后端事件防抖也不再刷新", async () => {
    const query = ref("");
    const history = mountHistory(query);
    query.value = "卸载前搜索";
    await nextTick();
    changedHandlers.forEach((handler) => handler());
    cleanups.splice(0).forEach((cleanup) => cleanup());
    expect(history.loading.value).toBe(false);
    expect(history.canLoadMore.value).toBe(false);
    await settle(0, page());
    await vi.runAllTimersAsync();
    expect(history.canLoadMore.value).toBe(false);
    expect(history.loading.value).toBe(false);
    expect(history.items.value).toEqual([]);
    expect(history.hasMore.value).toBe(false);
    expect(listClipboardItems).toHaveBeenCalledTimes(1);
  });
});
