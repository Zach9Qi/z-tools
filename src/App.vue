<script setup lang="ts">
import LauncherPanel from "@/components/launcher/LauncherPanel.vue";
</script>

<template>
  <!-- 根容器只管布局;color-scheme / 底色 / 文字色 / 字体族已在 index.css 基础层挂到 html、body 上,这里不要再加 scheme-* 或 bg-*/text-* -->
  <!-- 面板即页面:宽高铺满窗口、overflow-hidden 交给面板内部滚动,窗口尺寸由 useAutoHeight 反向跟随面板 -->
  <main class="h-screen w-screen overflow-hidden">
    <LauncherPanel />
  </main>
</template>
