<script setup lang="ts">
// 工具页顶栏:左侧分类 Tab,右侧「收藏」筛选开关。
// 两者都是纯展示 + emit,状态由 ClipboardPage 经 useClipboardHistory 持有。
import type { ClipboardKind } from "@/types/clipboard";
import IconStar from "~icons/lucide/star";

const { kind, favoriteOnly } = defineProps<{
  /** 当前分类;null = 全部 */
  kind: ClipboardKind | null;
  /** 「只看收藏」是否开启 */
  favoriteOnly: boolean;
}>();

const emit = defineEmits<{
  selectKind: [kind: ClipboardKind | null];
  toggleFavoriteOnly: [];
}>();

/** Tab 定义;顺序即显示顺序,与 useClipboardHistory 的循环顺序一致 */
const TABS: Array<{ value: ClipboardKind | null; label: string }> = [
  { value: null, label: "全部" },
  { value: "text", label: "文本" },
  { value: "image", label: "图片" },
  { value: "files", label: "文件" },
];
</script>

<template>
  <!-- 所有按钮 mousedown.prevent:焦点常驻搜索框;select-none 因为这是 chrome 区不是内容 -->
  <header class="flex h-10 shrink-0 items-center justify-between gap-2 px-4 select-none">
    <nav class="flex items-center gap-1" aria-label="剪贴板分类">
      <!-- 选中态用 accent 叠加底(与 hover 同一组类),aria-current 表达当前 Tab -->
      <button
        v-for="tab in TABS"
        :key="tab.label"
        type="button"
        :aria-current="tab.value === kind ? 'true' : undefined"
        class="rounded-md px-2.5 py-1 text-xs font-medium outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        :class="tab.value === kind ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'"
        @mousedown.prevent
        @click="emit('selectKind', tab.value)"
      >
        {{ tab.label }}
      </button>
    </nav>
    <div class="flex items-center gap-1 text-xs">
      <!-- 收藏是独立筛选开关(不是置顶),用 aria-pressed;开启时星标实心并用 primary 强调 -->
      <button
        type="button"
        :aria-pressed="favoriteOnly"
        class="inline-flex items-center gap-1 rounded-md px-2.5 py-1 font-medium outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        :class="favoriteOnly ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'"
        @mousedown.prevent
        @click="emit('toggleFavoriteOnly')"
      >
        <IconStar
          class="size-3.5"
          :class="favoriteOnly && 'fill-current text-primary'"
          aria-hidden="true"
        />
        收藏
      </button>
    </div>
  </header>
</template>
