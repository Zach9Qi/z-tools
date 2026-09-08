<script setup lang="ts">
import { storeToRefs } from "pinia";
import { computed } from "vue";
import KeyboardKey from "@/components/common/KeyboardKey.vue";
import { useKeymapStore } from "@/stores/keymap";

const {
  view,
  message = "",
  error = "",
} = defineProps<{
  /** 当前视图,决定末尾 Esc 的文案 */
  view: "home" | "tool";
  /** launch 型工具执行后的一次性提示 */
  message?: string;
  /** launch 型工具执行失败的文案;优先于 message 显示 */
  error?: string;
}>();

/** 所有已登记绑定派生的提示(不含 Esc,Esc 在下面固定追加) */
const { hints } = storeToRefs(useKeymapStore());
/** Esc 文案:工具页返回主页,主页隐藏窗口 */
const escapeLabel = computed(() => (view === "tool" ? "返回" : "隐藏"));
</script>

<template>
  <footer
    class="flex h-10 shrink-0 items-center justify-between border-t px-4 text-xs text-muted-foreground"
  >
    <!-- 左侧互斥:错误优先于提示;都没有时留空 span 占位,让右侧列表保持右对齐 -->
    <p v-if="error" class="truncate text-destructive">{{ error }}</p>
    <p v-else-if="message" class="truncate">{{ message }}</p>
    <span v-else />
    <ul class="flex shrink-0 items-center gap-4">
      <li v-for="(hint, i) in hints" :key="i" class="flex items-center gap-1">
        <KeyboardKey v-for="key in hint.keys" :key="key">{{ key }}</KeyboardKey>
        {{ hint.label }}
      </li>
      <!-- Esc 由页脚固定渲染而非登记方声明,因为两种视图都有它,只是文案不同 -->
      <li class="flex items-center gap-1">
        <KeyboardKey>Esc</KeyboardKey>
        {{ escapeLabel }}
      </li>
    </ul>
  </footer>
</template>
