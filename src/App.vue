<script setup lang="ts">
import HelloWorld from "@/components/HelloWorld.vue";
</script>

<template>
  <!-- 根容器只管布局;color-scheme / 底色 / 文字色 / 字体族已在 index.css 基础层挂到 html、body 上,这里不要再加 scheme-* 或 bg-*/text-* -->
  <main class="flex min-h-screen items-center justify-center p-6">
    <HelloWorld />
  </main>
</template>
