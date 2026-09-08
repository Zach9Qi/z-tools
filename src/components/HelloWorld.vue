<script setup lang="ts">
import { computed, ref } from "vue";
import { greet } from "@/lib/api";
// 图标按需导入(unplugin-icons + lucide),编译期内联为 SVG 组件,尺寸用 size-* 控制、颜色跟随 currentColor
import IconLoader from "~icons/lucide/loader-circle";
import IconSend from "~icons/lucide/send";

/** 输入框里的名字 */
const name = ref("");
/** 后端返回的问候文案;为空表示尚未问候 */
const greeting = ref("");
/** 调用失败时的错误文案;与 greeting 互斥 */
const errorMessage = ref("");
/** IPC 调用进行中;由 submit 置位并在 finally 里复位,用于禁用按钮防重复提交 */
const loading = ref(false);
/** 是否可提交:名字非空且没有进行中的调用;由 name / loading 派生,不另存副本 */
const canSubmit = computed(() => name.value.trim() !== "" && !loading.value);

/** 调用后端 greet;失败时记录日志并把文案交给模板展示,不让异常冒泡炸掉 UI */
async function submit() {
  // 按钮 disabled 已挡住点击与回车隐式提交,这里再兜一道让语义不依赖 DOM 状态
  if (!canSubmit.value) return;
  errorMessage.value = "";
  loading.value = true;
  try {
    greeting.value = await greet(name.value);
  } catch (error) {
    console.error("问候失败:", error);
    greeting.value = "";
    errorMessage.value = String(error);
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <!-- 卡片是静态表面,用 card 令牌:浅色与页面底同白靠 border 分层,深色比页面底亮一档 -->
  <section
    class="flex w-full max-w-md flex-col gap-6 rounded-2xl border bg-card p-8 text-card-foreground"
  >
    <header class="flex flex-col gap-2">
      <h1 class="text-2xl font-semibold">Hello World</h1>
      <p class="text-sm text-muted-foreground">
        Tauri 2 + Vue 3 + Tailwind CSS 4 起步模板。输入名字后点击按钮,会通过 IPC 调用 Rust 侧的
        <code class="rounded-sm bg-muted px-1 font-mono">greet</code> 命令。
      </p>
    </header>

    <!-- 用 form 承载,回车即提交;prevent 阻止浏览器默认刷新 -->
    <form class="flex gap-2" @submit.prevent="submit">
      <!-- 表单控件描边用 input(比 border 强);焦点态用 outline-hidden(非 outline-none,高对比模式保留焦点)+ ring/50 半透明环 + 描边变 ring -->
      <input
        v-model="name"
        type="text"
        placeholder="输入你的名字"
        class="flex-1 rounded-md border border-input bg-background px-3 py-2 text-sm outline-hidden focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50"
      />
      <!-- 主 CTA 用 primary(accent 只做 hover / 选中叠加底);hover 走实底变色而非整体降透明,避免文字图标一起变淡;
           disabled 用 pointer-events-none 顺带挡掉 hover 变色 -->
      <button
        type="submit"
        :disabled="!canSubmit"
        class="inline-flex shrink-0 items-center gap-1.5 rounded-md bg-primary px-4 py-2 text-sm font-medium whitespace-nowrap text-primary-foreground outline-hidden transition-colors hover:bg-primary/90 focus-visible:ring-3 focus-visible:ring-ring/50 disabled:pointer-events-none disabled:opacity-50"
      >
        <!-- 装饰性图标一律 aria-hidden,按钮语义由文字承担;loading 时换成旋转的 loader -->
        <IconLoader v-if="loading" class="size-4 animate-spin" aria-hidden="true" />
        <IconSend v-else class="size-4" aria-hidden="true" />
        问候
      </button>
    </form>

    <p v-if="greeting" class="text-sm">{{ greeting }}</p>
    <!-- 错误文案用 destructive,与正常文本可区分 -->
    <p v-else-if="errorMessage" class="text-sm text-destructive">{{ errorMessage }}</p>
  </section>
</template>
