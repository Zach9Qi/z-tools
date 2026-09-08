<script setup lang="ts">
import ToolTile from "@/components/launcher/ToolTile.vue";
import type { Section, StampedItem } from "@/lib/launcher/search";

defineProps<{
  section: Section;
  /** 当前选中的全局下标;与 stamped.index 相等的磁贴高亮 */
  selectedIndex: number;
}>();

const emit = defineEmits<{
  /** 请求选中某个全局下标 */
  select: [index: number];
  /** 激活某个条目 */
  activate: [item: StampedItem];
}>();
</script>

<template>
  <section class="flex flex-col gap-2">
    <header class="flex h-5 items-center px-2">
      <h2 class="text-xs font-semibold tracking-wide text-muted-foreground">{{ section.title }}</h2>
    </header>
    <!-- 列数必须与 useRowNavigation 的 columns 一致,否则上下方向键会错行 -->
    <div class="grid grid-cols-8 gap-2">
      <ToolTile
        v-for="stamped in section.items"
        :key="stamped.item.id"
        :item="stamped.item"
        :selected="stamped.index === selectedIndex"
        @select="emit('select', stamped.index)"
        @activate="emit('activate', stamped)"
      />
    </div>
  </section>
</template>
