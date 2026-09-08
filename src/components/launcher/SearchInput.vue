<script setup lang="ts">
import { onMounted, ref } from "vue";

const { placeholder = "" } = defineProps<{
  placeholder?: string;
}>();

/** 输入框内容,由父组件 v-model 绑定 */
const model = defineModel<string>({ default: "" });

/** 原生 input 元素,聚焦时需要;挂载前为 null */
const inputRef = ref<HTMLInputElement | null>(null);

/**
 * 聚焦并把光标移到末尾。
 * 从「匹配结果」进入工具页时输入框带着主页搜索词重建,程序聚焦后光标默认在开头,
 * 继续输入会插到前面;统一移到末尾让任何时候重新聚焦都能接着写。
 */
function focus(): void {
  const el = inputRef.value;
  if (el === null) return;
  el.focus();
  const end = el.value.length;
  el.setSelectionRange(end, end);
}

// 搜索框是启动器唯一的输入焦点,挂载即聚焦;视图切换时组件被 v-if 重建,自然再次聚焦
onMounted(focus);

defineExpose({ focus });
</script>

<template>
  <!-- 透明底,由外层搜索栏承担表面;h-10 让焦点环与 h-16 的搜索栏上下留出空隙;
       spellcheck / autocomplete 关闭:搜索词不是自然语言,拼写波浪线与历史下拉都是干扰 -->
  <input
    ref="inputRef"
    v-model="model"
    type="text"
    spellcheck="false"
    autocomplete="off"
    :placeholder="placeholder"
    class="h-10 min-w-0 flex-1 rounded-md bg-transparent px-1 text-lg tracking-tight outline-hidden placeholder:text-muted-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
  />
</template>
