<script setup lang="ts">
// 列表项的展开态(行内手风琴):文本全文 / 原图 / 全部文件路径。
// 文本全文在这里按需向后端拉:列表 DTO 只带 300 字预览,单条上限 1 MiB 不能随列表下发;
// 组件随收起卸载即丢弃,不缓存(重新展开再拉,本地 SQLite 微秒级)。
import { onMounted, ref } from "vue";
import { getClipboardText, toAssetUrl } from "@/lib/api/clipboard";
import type { ClipboardItem } from "@/types/clipboard";
import { formatBytes } from "../lib/format";

const { item } = defineProps<{
  item: ClipboardItem;
}>();

/** 详情区根元素;挂载后滚到可见 */
const rootRef = ref<HTMLElement | null>(null);
/** 文本全文;仅 text 条目使用,拉取前为空串 */
const text = ref("");
/** 全文拉取中 */
const loading = ref(false);
/** 全文拉取失败的文案;只在详情区内显示,不进页面级 error */
const error = ref("");

async function loadText(id: number): Promise<void> {
  loading.value = true;
  try {
    text.value = await getClipboardText(id);
  } catch (e) {
    console.error("读取剪贴板文本失败:", e);
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  // nearest:展开项在视口下方时把详情顶部露出来,已在视野内时不动
  rootRef.value?.scrollIntoView({ block: "nearest" });
  if (item.kind === "text") void loadText(item.id);
});
</script>

<template>
  <!-- max-h-72 约为列表可用高度的一半,保证展开时上下仍能露出邻行;内部自己滚动。
       文字必须可选中复制,所以这里不加 select-none;它不在粘贴按钮内部,点击不会触发粘贴 -->
  <div ref="rootRef" class="flex max-h-72 flex-col gap-1 overflow-auto border-t px-3 py-2 text-xs">
    <template v-if="item.kind === 'text'">
      <p class="text-muted-foreground">{{ item.charCount }} 字 · {{ formatBytes(item.size) }}</p>
      <p v-if="loading" class="text-muted-foreground">加载中…</p>
      <p v-else-if="error" class="text-destructive">{{ error }}</p>
      <!-- pre-wrap 保留换行与缩进但允许长行折行;break-all 防止无空格的长串撑破宽度 -->
      <pre v-else class="font-mono text-xs break-all whitespace-pre-wrap">{{ text }}</pre>
    </template>
    <template v-else-if="item.kind === 'image'">
      <p class="text-muted-foreground">
        {{ item.width }}×{{ item.height }} · {{ formatBytes(item.size) }}
      </p>
      <!-- max-h-64 留出头部一行,整体不超过容器 max-h-72,不出现只为一行文字而生的滚动条 -->
      <img
        :src="toAssetUrl(item.imagePath)"
        alt=""
        class="max-h-64 w-auto rounded-md object-contain"
      />
    </template>
    <ul v-else class="flex flex-col gap-0.5">
      <!-- 不存在的文件整行标灰,路径部分划线;「不存在」放在划线 span 之外才不会一起被划掉 -->
      <li
        v-for="file in item.files"
        :key="file.path"
        class="font-mono break-all"
        :class="!file.exists && 'text-muted-foreground'"
      >
        <span :class="!file.exists && 'line-through'">{{ file.path }}</span>
        <span v-if="!file.exists"> · 不存在</span>
      </li>
    </ul>
  </div>
</template>
