# 组件规范(Vue 3 SFC)

> 参考实现:`src/components/launcher/LauncherPanel.vue`(状态拥有者 + 异步错误套路)、`src/components/launcher/ToolTile.vue`(props / emits / 选中态)、`src/App.vue`。

---

## 1. SFC 基本形态

- 一律 `<script setup lang="ts">`,不写 Options API,不写无 `lang="ts"` 的 script。(本仓库全部如此)
- 块顺序固定:`<script setup>` → `<template>` → `<style>`(如有)。业界对 template 先后无统一做法,本仓库以现有文件为准,不混用。
- 组件文件 PascalCase,模板里也用 PascalCase 标签(`<LauncherPanel />`),不用 kebab-case。
- 一个文件一个组件;不用 `defineOptions({ name })`(Volar 按文件名推断即可)。

## 2. Props / Emits

```ts
// 类型用 TS 泛型声明,不用运行时对象写法
const { size = "md", disabled = false } = defineProps<{
  size?: "sm" | "md" | "lg";
  disabled?: boolean;
}>();

// emits 用元组语法(Vue 3.3+ 官方推荐的简写)
const emit = defineEmits<{
  change: [value: string];
  close: [];
}>();
```

- 一律用 `defineProps<T>()` 泛型写法。
- 默认值用 **解构默认值**(Vue 3.5 响应式 props 解构,本仓库 Vue 3.5.x),不用 `withDefaults`。
- 不在子组件里修改 props(Vue 单向数据流);需要双向绑定用 `defineModel()`(Vue 3.4+ 官方 API;`SearchInput.vue` / `HomeSearchBar.vue` 即此写法)。
- 需要让父组件调用子组件方法(如 `focus()`)时用 `defineExpose`,并只暴露那一个方法;中间层需要转发时也写一个同名函数再 `defineExpose`(`HomeSearchBar` → `SearchInput`)。

## 3. 组件内状态

来自 `LauncherPanel.vue` 的写法,是本项目的样板:

- 每个 `ref` / `computed` 上方一行 `/** 中文说明 */`,说明它代表什么、何时变化、与谁互斥(`message` 与 `error` 互斥、`toolQuery` 返回主页时清空)。
- 派生状态用 `computed`,不另存副本(`view` 由 `activeModule` 派生,不单独存一个「是否在工具页」的 ref)。
- 异步操作:`try / catch` → `console.error("中文前缀:", error)` + 错误写入 `error` ref 供模板展示,不让异常冒泡(`activate()` 对 `module.run()` 的处理);有可感知的等待时再加 `loading` 置位并在 `finally` 复位。
- 一个领域只有一个「状态拥有者」组件(`LauncherPanel`),其余子组件只接 props / 发 emits;子组件自己持有的状态仅限与自身生命周期绑定的部分(`ResultsPanel` 持有 `selectedIndex`,因为它卸载时方向键登记也应一起消失)。
- 只在组件内使用的状态留在组件;需要跨组件共享再进 Pinia(见 `state-management.md`)。

## 4. 模板

- 用语义化标签(`<main>`、`<section>`、`<header>`、`<form>`),不用一堆 `<div>`。
- 表单提交用 `<form @submit.prevent="submit">` + `type="submit"` 按钮,让回车天然可用。
- 装饰性图标一律 `aria-hidden="true"`,按钮语义由文字承担。
- 互斥的两个提示用 `v-if` / `v-else-if`,不写两个独立 `v-if`。
- 模板里的注释用 `<!-- 中文 -->`,解释「为什么这样写」(例如 `App.vue` 里解释根容器为何只管布局、不写配色类)。
- 模板里不写复杂表达式,逻辑提到 `computed`(Vue 官方风格指南「简单的模板表达式」;`LauncherFooter.vue` 的 `escapeLabel`、`ToolTile.vue` 的 `icon` 即此做法)。
- 互斥重建的分支用 `v-if` / `v-else-if` 而非 `v-show`,当你**需要**重建触发 `onMounted`(如 `SearchInput` 自动聚焦)或随卸载注销登记时;否则用 `v-show`。在模板注释里写明选择理由(`LauncherPanel.vue` 即此做法)。
- 焦点常驻单一输入框的界面,其他可点击元素加 `@mousedown.prevent` 防止抢焦点(`ToolTile.vue`、`ToolSearchBar.vue` 的徽章)。

## 5. 图标

- 通过 `unplugin-icons` 按需导入:`import IconSend from "~icons/lucide/send";`(`vite.config.ts` 中 `compiler: "vue3"`, `scale: 1`)。
- 尺寸用 Tailwind `size-*`,颜色跟随 `currentColor`,不写内联 `width/height`。
- 图标集只装 `@iconify-json/lucide`;需要别的图标集时显式安装到 devDependencies,不开 `autoInstall`。

## 6. 样式

- 优先 Tailwind 工具类 + 语义令牌(`bg-background`、`text-muted-foreground`),详见 `styling-guidelines.md`。
- 类名顺序由 `prettier-plugin-tailwindcss` 自动排序,不手动整理。
- 只有工具类无法表达时才写 `<style scoped>`;禁止全局 `<style>`(全局样式只在 `src/index.css`)。
- 有 `variant` / `size` props 的组件(按钮、徽章等),类名映射按 `styling-guidelines.md` 「组件变体写法」用 `as const` 对象,不引入 `clsx` / `cva`。

## 7. 禁止

- 组件内 `import { invoke }` / `import { listen }` —— 走 `src/lib/api.ts` 与 composable(见 `ipc-guidelines.md`)。
- `any`、非空断言 `!`、`as unknown as X`(见 `type-safety.md`)。
- `withDefaults`、运行时对象式 `defineProps({...})`、Options API。
- 在 `setup` 顶层直接 `await` IPC(会把组件变成异步组件,需要 `<Suspense>`);初始化拉取写成不带 `await` 的函数调用或放 `onMounted`,并走 `try / catch` → `console.error` + `error` ref 套路(§3)。
