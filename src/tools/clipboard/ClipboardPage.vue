<script setup lang="ts">
// 剪贴板工具页:Tabs + 可滚动列表 + 底部哨兵(触发追加分页)+ 空态 / 加载 / 错误条。
// 状态全部在 useClipboardHistory;这里只负责布局、键位登记与哨兵观察。
import { nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useKeymap } from "@/composables/useKeymap";
import IconClipboard from "~icons/lucide/clipboard";
import ClipboardItemRow from "./components/ClipboardItemRow.vue";
import ClipboardTabs from "./components/ClipboardTabs.vue";
import { useClipboardHistory } from "./composables/useClipboardHistory";

const props = defineProps<{
  /** 工具页搜索栏的当前输入,由面板透传 */
  query: string;
}>();

const {
  items,
  kind,
  favoriteOnly,
  selectedIndex,
  selected,
  expandedId,
  loading,
  canLoadMore,
  error,
  loadMore,
  paste,
  remove,
  toggleFavorite,
  toggleFavoriteOnly,
  setKind,
  cycleKind,
  toggleExpanded,
  moveSelection,
} = useClipboardHistory(() => props.query);

/** 列表滚动容器;IntersectionObserver 的 root */
const listRef = ref<HTMLElement | null>(null);
/** 列表末尾的哨兵;进入视口即请求下一页 */
const sentinelRef = ref<HTMLElement | null>(null);
/** 哨兵观察器;卸载时 disconnect */
let observer: IntersectionObserver | undefined;

/** 哨兵当前是否在列表视口内;用几何而非 IO 回调缓存,避免拿到过期状态 */
function sentinelInView(): boolean {
  const list = listRef.value;
  const sentinel = sentinelRef.value;
  if (list === null || sentinel === null) return false;
  return sentinel.getBoundingClientRect().top <= list.getBoundingClientRect().bottom;
}

onMounted(() => {
  const list = listRef.value;
  const sentinel = sentinelRef.value;
  if (list === null || sentinel === null) return;
  observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) void loadMore();
    },
    { root: list },
  );
  observer.observe(sentinel);
});
onUnmounted(() => observer?.disconnect());

// IO 只在「进入 / 离开」时回调:成功且还有下一页时等 DOM 更新补查哨兵。
// nextTick 内复查分页条件,避免排队期间已有新请求在途、失败或到底。
watch(canLoadMore, (canLoad) => {
  if (!canLoad) return;
  void nextTick(() => {
    if (canLoadMore.value && sentinelInView()) void loadMore();
  });
});

useKeymap([
  {
    keys: ["ArrowUp", "ArrowDown"],
    label: "选择",
    onPress: (e) => moveSelection(e.key === "ArrowUp" ? -1 : 1),
  },
  {
    keys: ["Enter"],
    label: "粘贴",
    onPress: () => {
      if (selected.value !== undefined) void paste(selected.value.id);
    },
  },
  {
    keys: ["Delete"],
    label: "删除",
    onPress: () => {
      if (selected.value !== undefined) void remove(selected.value.id);
    },
  },
  {
    keys: ["p"],
    ctrl: true,
    label: "收藏",
    onPress: () => {
      if (selected.value !== undefined) void toggleFavorite(selected.value.id);
    },
  },
  { keys: ["f"], ctrl: true, label: "只看收藏", onPress: () => void toggleFavoriteOnly() },
  {
    keys: ["ArrowLeft", "ArrowRight"],
    ctrl: true,
    label: "切换分类",
    onPress: (e) => void cycleKind(e.key === "ArrowLeft" ? -1 : 1),
  },
]);
</script>

<template>
  <!-- 单根 section:面板会把 flex min-h-0 flex-1 flex-col fallthrough 到这里 -->
  <section>
    <ClipboardTabs
      :kind="kind"
      :favorite-only="favoriteOnly"
      @select-kind="setKind"
      @toggle-favorite-only="toggleFavoriteOnly"
    />
    <!-- 错误条:命令失败的中文文案(Rust 产出,完整句子,不拼前缀);详情区内的拉取错误不走这里 -->
    <p v-if="error" class="shrink-0 border-t px-4 py-1 text-xs text-destructive">{{ error }}</p>
    <!-- min-h-0 flex-1 让列表在面板固定高度内自己滚动 -->
    <div ref="listRef" class="min-h-0 flex-1 overflow-y-auto border-t p-2">
      <!-- 空态与列表互斥;首屏加载中不显示空态,避免闪一下「暂无记录」 -->
      <div v-if="items.length === 0 && !loading" class="flex flex-col items-center gap-4 py-12">
        <div class="flex size-12 items-center justify-center rounded-2xl border bg-muted">
          <IconClipboard class="size-6 text-muted-foreground" aria-hidden="true" />
        </div>
        <p class="text-sm text-muted-foreground">
          {{ query || favoriteOnly || kind !== null ? "没有匹配的记录" : "暂无剪贴板记录" }}
        </p>
      </div>
      <ul v-else class="flex flex-col gap-0.5">
        <ClipboardItemRow
          v-for="(item, index) in items"
          :key="item.id"
          :item="item"
          :selected="index === selectedIndex"
          :expanded="item.id === expandedId"
          @paste="paste(item.id)"
          @toggle-favorite="toggleFavorite(item.id)"
          @toggle-expanded="toggleExpanded(item.id)"
        />
      </ul>
      <!-- 哨兵始终渲染(不随 hasMore v-if),观察器只挂一次 -->
      <div ref="sentinelRef" class="h-px" aria-hidden="true" />
    </div>
  </section>
</template>
