<script setup lang="ts">
import { ref } from "vue";
import KeyboardKey from "@/components/common/KeyboardKey.vue";
import SearchInput from "@/components/launcher/SearchInput.vue";
import IconSearch from "~icons/lucide/search";

/** 主页搜索词,由面板 v-model 绑定 */
const model = defineModel<string>({ default: "" });

/** 内层输入框实例,用于把 focus 转发给面板 */
const inputRef = ref<InstanceType<typeof SearchInput> | null>(null);

function focus(): void {
  inputRef.value?.focus();
}

defineExpose({ focus });
</script>

<template>
  <!-- 搜索栏是窗口 chrome:data-tauri-drag-region 让透明无边框窗口可拖动,select-none 防止拖动时选中文字;
       输入框本身不带该属性,所以点击输入框不会触发拖动 -->
  <div data-tauri-drag-region class="flex h-16 shrink-0 items-center gap-4 px-6 select-none">
    <IconSearch class="size-5 shrink-0 text-muted-foreground" aria-hidden="true" />
    <SearchInput ref="inputRef" v-model="model" placeholder="搜索工具…" />
    <!-- 全局唤起快捷键提示;实际绑定由 Rust 侧窗口任务落地,这里只是文案 -->
    <div class="flex shrink-0 items-center gap-1">
      <KeyboardKey>Alt</KeyboardKey>
      <KeyboardKey>Enter</KeyboardKey>
    </div>
  </div>
</template>
