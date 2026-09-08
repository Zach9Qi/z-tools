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
 * 聚焦输入框,两种光标策略:
 * - 默认光标到末尾:从「匹配结果」进入工具页 / 返回主页时输入框带着旧词重建,
 *   程序聚焦后光标默认在开头,继续输入会插到前面;移到末尾让用户接着写。
 * - `selectAll`:全局快捷键唤出窗口时全选旧词,直接输入即覆盖,不删不清空(Esc 仍可关闭)。
 */
function focus(options: { selectAll?: boolean } = {}): void {
  const el = inputRef.value;
  if (el === null) return;
  el.focus();
  if (options.selectAll) {
    el.select();
    return;
  }
  const end = el.value.length;
  el.setSelectionRange(end, end);
}

// 搜索框是启动器唯一的输入焦点,挂载即聚焦;视图切换时组件被 v-if 重建,自然再次聚焦
onMounted(focus);

defineExpose({ focus });
</script>

<template>
  <!-- 透明底,由外层搜索栏承担表面。
       不画焦点环(规范的显式例外,见 styling-guidelines §10):这是启动器内唯一且常驻的焦点目标,
       每次唤出都程序聚焦,环会常亮成一圈突兀的边框而不传递任何信息;焦点位置由光标与面板本身表达。
       spellcheck / autocomplete 关闭:搜索词不是自然语言,拼写波浪线与历史下拉都是干扰 -->
  <input
    ref="inputRef"
    v-model="model"
    type="text"
    spellcheck="false"
    autocomplete="off"
    :placeholder="placeholder"
    class="h-10 min-w-0 flex-1 bg-transparent px-1 text-lg tracking-tight outline-hidden placeholder:text-muted-foreground"
  />
</template>
