<script setup lang="ts">
// 结果区自己持有选中下标与方向键 / Enter 登记:它只在主页态挂载,
// 进入工具页卸载时登记自动注销,工具页里的方向键 / Enter 才不会被网格导航吞掉。
import ToolSection from "@/components/launcher/ToolSection.vue";
import { useRowNavigation } from "@/composables/useRowNavigation";
import type { Section, StampedItem } from "@/lib/launcher/search";
import IconSearchX from "~icons/lucide/search-x";

/** 每行磁贴数;必须与 ToolSection 的 grid-cols-8 一致,否则上下移动会错行 */
const COLUMNS = 8;

const props = defineProps<{
  /** 已剔除空分区的分区列表;为空即渲染空态 */
  sections: Section[];
}>();

const emit = defineEmits<{
  activate: [item: StampedItem];
}>();

const { selectedIndex, select } = useRowNavigation({
  getSections: () => props.sections,
  columns: COLUMNS,
  onActivate: (stamped) => emit("activate", stamped),
});
</script>

<template>
  <!-- min-h-0 flex-1 让结果区在面板 max-h 内收缩并自己滚动;border-y 与搜索栏 / 页脚分层 -->
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto border-y p-4">
    <div v-if="sections.length === 0" class="flex flex-col items-center gap-4 py-12">
      <div class="flex size-12 items-center justify-center rounded-2xl border bg-muted">
        <IconSearchX class="size-6 text-muted-foreground" aria-hidden="true" />
      </div>
      <p class="text-sm text-muted-foreground">没有匹配的工具</p>
    </div>
    <!-- shrink-0 防止分区在滚动容器内被压扁 -->
    <div v-else class="flex shrink-0 flex-col gap-4">
      <ToolSection
        v-for="section in sections"
        :key="section.id"
        :section="section"
        :selected-index="selectedIndex"
        @select="select"
        @activate="emit('activate', $event)"
      />
    </div>
  </div>
</template>
