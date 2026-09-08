<script setup lang="ts">
// 示例工具,真实工具进来后整目录删除。
// view 型工具页的最小样板:只接收 query 并回显,用于验证「搜索栏输入 → 工具页 prop」的链路。
import { computed } from "vue";

const { query } = defineProps<{
  /** 工具页态搜索栏的当前输入,由面板透传 */
  query: string;
}>();

/** 回显文案;空输入显示「(空)」,避免页面看起来像没渲染 */
const display = computed(() => (query === "" ? "(空)" : query));
</script>

<template>
  <!-- 根用 flex min-h-0 flex-1 flex-col 撑满面板内容区,内部溢出时自己滚动,不撑破面板 h-150 -->
  <section class="flex min-h-0 flex-1 flex-col items-center justify-center overflow-y-auto p-4">
    <p class="text-sm text-muted-foreground">工具页在此挂载 · 当前输入:{{ display }}</p>
  </section>
</template>
