<script setup lang="ts">
// 启动器壳:唯一持有视图状态(主页 / 工具页、两个搜索词、当前工具)的组件,
// 子组件只通过 props / emits 与它通信;快捷键登记表走 store(页脚在另一支消费)。
import { computed, nextTick, ref, useTemplateRef } from "vue";
import HomeSearchBar from "@/components/launcher/HomeSearchBar.vue";
import LauncherFooter from "@/components/launcher/LauncherFooter.vue";
import ResultsPanel from "@/components/launcher/ResultsPanel.vue";
import ToolSearchBar from "@/components/launcher/ToolSearchBar.vue";
import { useAutoHeight } from "@/composables/useAutoHeight";
import { useKeymap, useKeymapListener } from "@/composables/useKeymap";
import { buildSections, type StampedItem } from "@/lib/launcher/search";
import { hideLauncher } from "@/lib/window";
import { catalog, moduleOf } from "@/tools/registry";
import { isViewModule, type ViewToolModule } from "@/types/tool";

/** 面板根元素,供 useAutoHeight 观察高度 */
const rootRef = ref<HTMLElement | null>(null);
/** 当前视图的搜索栏(主页 / 工具页二选一),返回主页时用它重新聚焦 */
const searchBar = useTemplateRef("searchBar");

/** 主页搜索词;进入工具页时清空 */
const homeQuery = ref("");
/** 工具页搜索词,透传给工具页 query prop;返回主页时清空 */
const toolQuery = ref("");
/** 当前打开的 view 型工具;null 即主页 */
const activeModule = ref<ViewToolModule | null>(null);
/** launch 型工具执行后的一次性提示;与 error 互斥,下一次激活时清空 */
const message = ref("");
/** launch 型工具执行失败的文案;与 message 互斥 */
const error = ref("");

/** 当前视图,由 activeModule 派生 */
const view = computed<"home" | "tool">(() => (activeModule.value === null ? "home" : "tool"));
/** 主页分区列表,随搜索词重算 */
const sections = computed(() => buildSections(catalog, homeQuery.value));

/**
 * 激活一个条目。
 * view 型:进入工具页;若来自「匹配结果」分区(accepts 命中),把主页搜索词带进工具页,
 * 因为用户输入的正是该工具要处理的内容(如 URL);名称命中时搜索词只是工具名,不带入。
 * launch 型:执行 run(),成功给提示、失败记日志并展示错误,不抛出。
 */
async function activate(stamped: StampedItem): Promise<void> {
  const module = moduleOf(stamped.item);
  message.value = "";
  error.value = "";
  if (isViewModule(module)) {
    toolQuery.value = stamped.sectionId === "matches" ? homeQuery.value : "";
    activeModule.value = module;
    homeQuery.value = "";
    return;
  }
  try {
    await module.run({ query: homeQuery.value });
    message.value = `已触发:${module.item.title}`;
  } catch (e) {
    console.error("启动工具失败:", e);
    error.value = String(e);
  }
}

/** 返回主页;搜索栏因 v-if 重建会自动聚焦,这里再显式聚焦一次兜底 */
function closeTool(): void {
  activeModule.value = null;
  toolQuery.value = "";
  void nextTick(() => searchBar.value?.focus());
}

/** Esc:工具页返回主页,主页隐藏窗口 */
function onEscape(): void {
  if (view.value === "tool") {
    closeTool();
  } else {
    void hideLauncher();
  }
}

// Esc 的 label 随视图变化,用 getter 让 useKeymap 重新登记;hint: false 因为页脚固定渲染 Esc
useKeymap(() => [
  {
    keys: ["Escape"],
    label: view.value === "tool" ? "返回" : "隐藏",
    hint: false,
    onPress: onEscape,
  },
]);
useKeymapListener();
useAutoHeight(rootRef);
</script>

<template>
  <!-- 面板是静态表面(card + border);max-h-150 是窗口高度上限。
       主页高度随内容(useAutoHeight 把它同步给窗口),工具页强制 h-150 撑满,
       否则工具页内容少时面板会塌成一条,工具页组件也无法靠 flex-1 拿到可滚动区域 -->
  <section
    ref="rootRef"
    class="flex max-h-150 w-full flex-col overflow-hidden rounded-2xl border bg-card text-card-foreground"
    :class="view === 'tool' && 'h-150'"
  >
    <!-- 两种搜索栏用 v-if 互斥重建而非 v-show:重建触发 SearchInput 的 onMounted 自动聚焦,
         也让 placeholder / 徽章随视图整体切换,不必在同一组件内做分支 -->
    <template v-if="view === 'home'">
      <HomeSearchBar ref="searchBar" v-model="homeQuery" />
      <!-- 方向键 / Enter 的登记在 ResultsPanel 内部,随它一起卸载,工具页不受影响 -->
      <ResultsPanel :sections="sections" @activate="activate" />
    </template>
    <template v-else-if="activeModule">
      <ToolSearchBar
        ref="searchBar"
        v-model="toolQuery"
        :module="activeModule"
        @close="closeTool"
      />
      <!-- 工具页契约:接收 query prop;这里统一给根元素 flex min-h-0 flex-1,工具页不必各自重写才能撑满并内部滚动 -->
      <component :is="activeModule.page" :query="toolQuery" class="flex min-h-0 flex-1 flex-col" />
    </template>
    <LauncherFooter :view="view" :message="message" :error="error" />
  </section>
</template>
