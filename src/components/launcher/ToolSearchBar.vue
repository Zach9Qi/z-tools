<script setup lang="ts">
import { computed, ref } from "vue";
import KeyboardKey from "@/components/common/KeyboardKey.vue";
import SearchInput from "@/components/launcher/SearchInput.vue";
import { iconOf } from "@/tools/icons";
import type { ViewToolModule } from "@/types/tool";

const { module } = defineProps<{
  /** 当前打开的 view 型工具,徽章与 placeholder 都取自它 */
  module: ViewToolModule;
  /** 全局唤出快捷键的键帽序列，由面板从后端读取；为空时不渲染提示 */
  shortcutKeys: string[];
}>();

const emit = defineEmits<{
  /** 请求返回主页:点击徽章或空输入时按 Backspace */
  close: [];
}>();

/** 工具页搜索词,由面板 v-model 绑定并透传给工具页 */
const model = defineModel<string>({ default: "" });

/** 内层输入框实例,用于把 focus 转发给面板 */
const inputRef = ref<InstanceType<typeof SearchInput> | null>(null);
/** 徽章图标组件;模块不变时不重算 */
const icon = computed(() => iconOf(module.item.icon));
/** 输入框占位文案;模块未提供时用通用文案 */
const placeholder = computed(() => module.placeholder ?? "搜索…");

/**
 * 输入框为空时再按 Backspace 视为「退出工具页」,与终端 / 命令面板的习惯一致。
 * 输入法合成期间 v-model 不更新,model 为空但用户正在删拼音,必须放过。
 */
function onBackspace(e: KeyboardEvent): void {
  if (e.isComposing) return;
  if (model.value === "") emit("close");
}

/** 透传给内层输入框;参数语义见 SearchInput.focus */
function focus(options?: { selectAll?: boolean }): void {
  inputRef.value?.focus(options);
}

defineExpose({ focus });
</script>

<template>
  <!-- 与主页搜索栏同一外框,保证切换视图时高度与内边距不跳动 -->
  <div data-tauri-drag-region class="flex h-16 shrink-0 items-center gap-4 px-6 select-none">
    <!-- 徽章是次级实底按钮,用 secondary;mousedown.prevent 阻止点击时抢走输入框焦点 -->
    <button
      type="button"
      class="inline-flex shrink-0 items-center gap-2 rounded-lg border bg-secondary px-2 py-1 text-sm font-medium text-secondary-foreground outline-hidden transition-colors hover:bg-secondary/80 focus-visible:ring-3 focus-visible:ring-ring/50"
      @mousedown.prevent
      @click="emit('close')"
    >
      <component :is="icon" class="size-4" aria-hidden="true" />
      {{ module.item.title }}
    </button>
    <SearchInput
      ref="inputRef"
      v-model="model"
      :placeholder="placeholder"
      @keydown.backspace="onBackspace"
    />
    <!-- 全局唤出快捷键提示;键位来自后端当前生效值(可配置),不要在此写死 -->
    <div v-if="shortcutKeys.length > 0" class="flex shrink-0 items-center gap-1">
      <KeyboardKey v-for="key in shortcutKeys" :key="key">{{ key }}</KeyboardKey>
    </div>
  </div>
</template>
