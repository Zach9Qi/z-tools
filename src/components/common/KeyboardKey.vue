<script setup lang="ts">
// 通用键帽:页脚快捷键提示与搜索栏右侧的 Alt / Enter 都用它,保证同一套尺寸与描边。
</script>

<template>
  <!-- 键帽是静态次级底,用 muted;min-w 保证单字符键(↑ / K)也是方形,不随字宽塌陷;
       font-sans 是因为 kbd 元素浏览器默认等宽字体,与页脚正文不一致 -->
  <kbd
    class="inline-flex h-5 min-w-5 items-center justify-center rounded-sm border bg-muted px-1 font-sans text-xs font-medium text-muted-foreground"
  >
    <slot />
  </kbd>
</template>
