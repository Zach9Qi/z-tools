<script setup lang="ts">
// 列表中的一条:收起态一行(类型图标 + 摘要 + 相对时间 + 星标 + chevron),展开时在下方挂 ClipboardItemDetail。
// 行内只有两个动作按钮:常显星标(切换收藏)与 chevron(展开);删除没有鼠标入口,只走 Delete 键。
import { computed, ref, watch } from "vue";
import { toAssetUrl } from "@/lib/api/clipboard";
import type { ClipboardItem } from "@/types/clipboard";
import IconChevronDown from "~icons/lucide/chevron-down";
import IconFileText from "~icons/lucide/file-text";
import IconFiles from "~icons/lucide/files";
import IconImage from "~icons/lucide/image";
import IconStar from "~icons/lucide/star";
import { formatRelativeTime, isExpandable, summarizeFiles } from "../lib/format";
import ClipboardItemDetail from "./ClipboardItemDetail.vue";

const { item, selected, expanded } = defineProps<{
  item: ClipboardItem;
  /** 是否为当前键盘选中项;高亮覆盖整行(含详情区) */
  selected: boolean;
  /** 是否展开详情;与 selected 独立 */
  expanded: boolean;
}>();

const emit = defineEmits<{
  /** 点击行主体 = 粘贴 */
  paste: [];
  toggleFavorite: [];
  toggleExpanded: [];
}>();

/** 类型图标,按 kind 选 */
const TYPE_ICONS = { text: IconFileText, image: IconImage, files: IconFiles } as const;

/** 行根元素;选中时滚到可见 */
const rootRef = ref<HTMLElement | null>(null);
/** 是否渲染 chevron;不可展开的行用等宽占位保持对齐 */
const expandable = computed(() => isExpandable(item));
/** 文件条目里任一文件已不存在 → 整行标灰(条目保留,粘贴时目标应用自行报错) */
const hasMissingFile = computed(
  () => item.kind === "files" && item.files.some((file) => !file.exists),
);
/** 单文件时把完整路径放进 title,补足收起态只显示文件名的信息 */
const filesTitle = computed(() =>
  item.kind === "files" && item.files.length === 1 ? item.files[0]?.path : undefined,
);
/** 相对时间随每次渲染重算即可,不为它单独起定时器 */
const time = computed(() => formatRelativeTime(item.copiedAt));

// 列表可滚动,方向键选到视口外的行时滚到可见;nearest 避免每次都跳到顶部
watch(
  () => selected,
  (isSelected) => {
    if (isSelected) rootRef.value?.scrollIntoView({ block: "nearest" });
  },
);
</script>

<template>
  <!-- 整行高亮只由 selected 驱动,鼠标悬停 / 移动不改变选择。
       行主体是一个 <button>(点击 = 粘贴),星标 / chevron 是它的兄弟按钮而不是子元素:button 不能嵌套 -->
  <li
    ref="rootRef"
    class="rounded-lg transition-colors"
    :class="selected && 'bg-accent text-accent-foreground'"
  >
    <div
      class="flex items-start gap-2 px-2 py-2"
      :class="hasMissingFile && 'text-muted-foreground'"
    >
      <button
        type="button"
        :aria-current="selected ? 'true' : undefined"
        class="flex min-w-0 flex-1 items-start gap-3 rounded-md text-left outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50"
        :title="filesTitle"
        @mousedown.prevent
        @click="emit('paste')"
      >
        <component
          :is="TYPE_ICONS[item.kind]"
          class="mt-0.5 size-4 shrink-0 text-muted-foreground"
          aria-hidden="true"
        />
        <span class="min-w-0 flex-1">
          <!-- 文本:保留换行只显两行(pre-line 让预览里的换行生效,line-clamp 截断);break-all 防长串撑宽 -->
          <span
            v-if="item.kind === 'text'"
            class="line-clamp-2 text-sm break-all whitespace-pre-line"
            >{{ item.preview }}</span
          >
          <!-- 图片:缩略图定高 56px,宽随比例;lazy 让 100 条缩略图不一次性加载 -->
          <span v-else-if="item.kind === 'image'" class="flex items-center gap-3">
            <img
              :src="toAssetUrl(item.thumbPath)"
              loading="lazy"
              alt=""
              class="h-14 w-auto max-w-48 rounded-md border object-contain"
            />
            <span class="text-xs text-muted-foreground">{{ item.width }}×{{ item.height }}</span>
          </span>
          <span v-else class="block truncate text-sm">{{ summarizeFiles(item.files) }}</span>
        </span>
        <span class="mt-0.5 shrink-0 text-xs text-muted-foreground">{{ time }}</span>
      </button>
      <!-- 星标常显:收藏时实心 + primary;click.stop 不触发粘贴,mousedown.prevent 不夺搜索框焦点。
           它是开关,用 aria-pressed;仅图标所以要 aria-label。
           lucide 的 <path> 自带 fill="none" 属性,svg 上的 fill-current 覆盖不到它,要用 *: 变体直接作用在 path 上 -->
      <button
        type="button"
        :aria-pressed="item.favorite"
        :aria-label="item.favorite ? '取消收藏' : '收藏'"
        class="shrink-0 rounded-md p-1 outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        @mousedown.prevent
        @click.stop="emit('toggleFavorite')"
      >
        <IconStar
          class="size-4"
          :class="item.favorite ? 'text-primary *:fill-current' : 'text-muted-foreground'"
          aria-hidden="true"
        />
      </button>
      <!-- chevron 仅可展开时渲染;否则等宽占位(size-6 = p-1 + size-4)保持时间列对齐 -->
      <button
        v-if="expandable"
        type="button"
        :aria-expanded="expanded"
        :aria-label="expanded ? '收起' : '展开'"
        class="shrink-0 rounded-md p-1 text-muted-foreground outline-hidden transition-colors hover:bg-accent hover:text-accent-foreground focus-visible:ring-3 focus-visible:ring-ring/50"
        @mousedown.prevent
        @click.stop="emit('toggleExpanded')"
      >
        <IconChevronDown
          class="size-4 transition-transform"
          :class="expanded && 'rotate-180'"
          aria-hidden="true"
        />
      </button>
      <span v-else class="size-6 shrink-0" aria-hidden="true" />
    </div>
    <!-- v-if 而非 v-show:重建才触发 Detail 的 onMounted(拉全文 + 滚到可见),收起即释放全文 -->
    <ClipboardItemDetail v-if="expanded" :item="item" />
  </li>
</template>
