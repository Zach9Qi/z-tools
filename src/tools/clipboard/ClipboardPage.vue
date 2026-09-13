<script setup lang="ts">
// 剪贴板工具页:Tabs + 可滚动列表(滚到接近底部时追加分页)+ 空态 / 加载 / 错误条。
// 状态全部在 useClipboardHistory;这里只负责布局、键位登记与滚动容器的两件事(触发翻页、重置后回顶)。
import { nextTick, ref, watch } from "vue";
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
  error,
  resetTick,
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

/** 列表滚动容器;翻页阈值与回顶都作用在它上 */
const listRef = ref<HTMLElement | null>(null);
/** 距底不足此像素数即请求下一页;一页 100 条远高于一屏,滚动一定会经过这个区间 */
const LOAD_MORE_THRESHOLD_PX = 200;

// 只在用户滚动时触发:请求失败不会自转重试,再滚一次即重试;在途 / 到底由 loadMore 自己守卫
function handleScroll(): void {
  const el = listRef.value;
  if (el === null) return;
  if (el.scrollTop + el.clientHeight >= el.scrollHeight - LOAD_MORE_THRESHOLD_PX) void loadMore();
}

// 整表重置(筛选 / 搜索 / 唤起时发现漏了事件)后回到顶部;翻页、置顶、普通唤起不触发,滚动位置留着
watch(resetTick, () => {
  void nextTick(() => listRef.value?.scrollTo({ top: 0 }));
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
    <div ref="listRef" class="min-h-0 flex-1 overflow-y-auto border-t p-2" @scroll="handleScroll">
      <!-- 空态与列表互斥;首屏加载中不显示空态,避免闪一下「暂无记录」 -->
      <div v-if="items.length === 0 && !loading" class="flex flex-col items-center gap-4 py-12">
        <div class="flex size-12 items-center justify-center rounded-2xl border bg-muted">
          <IconClipboard class="size-6 text-muted-foreground" aria-hidden="true" />
        </div>
        <p v-if="error" class="text-sm text-destructive">加载失败</p>
        <p v-else class="text-sm text-muted-foreground">
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
    </div>
  </section>
</template>
