<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { iconOf } from "@/tools/icons";
import type { ToolItem } from "@/types/tool";

const { item, selected } = defineProps<{
  item: ToolItem;
  /** 是否为当前键盘 / 鼠标选中项,由父级按全局下标算出 */
  selected: boolean;
}>();

const emit = defineEmits<{
  /** 鼠标移入时请求选中,让 hover 与键盘选中共用同一高亮 */
  select: [];
  /** 点击激活 */
  activate: [];
}>();

/** 磁贴图标组件;item 不变时不重算 */
const icon = computed(() => iconOf(item.icon));

/** 磁贴根元素;键盘选中时需要把它滚进结果区视野 */
const rootRef = ref<HTMLButtonElement | null>(null);

// 结果区可滚动,方向键选到折叠区外的磁贴时自动滚到可见;nearest 避免每次都跳到顶部
// (鼠标 hover 选中时磁贴本就在视野内,scrollIntoView 不会产生位移)
watch(
  () => selected,
  (isSelected) => {
    if (isSelected) rootRef.value?.scrollIntoView({ block: "nearest" });
  },
);
</script>

<template>
  <!-- 选中与 hover 共用 accent 叠加底;mousedown.prevent 让点击不抢搜索框焦点(焦点常驻输入框);
       磁贴不是开关按钮,用 aria-current 而非 aria-pressed 表达「当前选中项」 -->
  <button
    ref="rootRef"
    type="button"
    :aria-current="selected ? 'true' : undefined"
    class="flex flex-col items-center gap-2 rounded-xl p-2 outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50 active:scale-95"
    :class="selected && 'bg-accent text-accent-foreground'"
    @mousedown.prevent
    @mouseenter="emit('select')"
    @click="emit('activate')"
  >
    <!-- 图标盒用 background 而非 card:深色下比面板暗一档,与 accent 叠加后仍能分辨 -->
    <span class="flex size-11 shrink-0 items-center justify-center rounded-xl border bg-background">
      <component :is="icon" class="size-5" aria-hidden="true" />
    </span>
    <!-- 两行定高:min-h-8 = 2 × text-xs/4 行高,单行标题也占两行,磁贴高度一致 -->
    <span class="line-clamp-2 min-h-8 w-full text-center text-xs/4 font-medium">
      {{ item.title }}
    </span>
  </button>
</template>
