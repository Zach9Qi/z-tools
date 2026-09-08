# 执行计划:启动器前端壳与工具注册契约

> 设计见 `design.md`;每步完成后跑对应验证命令再进入下一步。所有步骤都是前端,Rust 侧零改动。

## 0. 前置

- [ ] 读 `.trellis/spec/frontend/index.md` 的开发前检查清单;涉及的规范文件已列在 `implement.jsonl`。
- [ ] `bun add pinia`;确认 `package.json` dependencies 出现 `pinia`,锁文件只更新 `bun.lock`。

## 1. 纯逻辑层(先测再写,TDD)

- [ ] `src/lib/launcher/keyLabels.ts` + `keyLabels.test.ts`:`KEY_LABELS`、`formatKeyLabel`(方向键 / Enter / Delete / Escape / Backspace 映射;单字母大写;其余原样)。
- [ ] `src/lib/launcher/search.ts` + `search.test.ts`:`buildSections` / `flattenSections`。用例:空 query → 单一「全部工具」;title 命中;keyword 大小写不敏感命中;`accepts` 命中且不与 named 重复;空分区剔除;全空返回 `[]`;全局下标连续。
- [ ] `src/lib/launcher/navigation.ts` + `navigation.test.ts`:`chunkRows`(分区内按 8 分行,跨分区不并行)、`nextIndex`。用例:左右 ±1 与首尾回绕;下移保列;下移到短行贴行尾;顶底回绕;单行时上下不动或回绕到自身;`total===0` 返回 -1。
- 验证:`bun run test`。

## 2. 契约与注册表

- [ ] `src/types/tool.ts`:`ToolItem` / `ViewToolModule` / `LaunchToolModule` / `ToolModule` / `isViewModule`。
- [ ] `src/tools/icons.ts`:`iconOf(name)`;登记本任务演示工具用到的 lucide 名(如 `puzzle`、`sparkles`、`terminal`、`folder`、`link`、`calculator`、`palette`、`clock`、`file-text`、`settings`、`image`),回退 `puzzle`。
- [ ] `src/tools/demo/DemoToolPage.vue`:接收 `query`,`flex min-h-0 flex-1 flex-col` 根,内容居中回显「工具页在此挂载 · 当前输入:xxx」(`text-sm text-muted-foreground`)。
- [ ] `src/tools/demo/index.ts`:`demoViewTool`(id `demo-view`,title「示例工具页」,placeholder「在示例工具里搜索…」)+ `demoLaunchTools`(≥ 8 个,id `demo-launch-1..n`,标题各异、部分 `keywords` 含 `demo`,其中一个 `accepts: (q) => q.startsWith("http")` 用于验证「匹配结果」分区);`run()` 返回 resolved Promise 且不做副作用,由面板显示提示。
- [ ] `src/tools/registry.ts`:`modules` / `catalog` / `moduleOf`。
- 验证:`bun run build`(类型)。

## 3. 状态与 composable

- [ ] `src/stores/keymap.ts`:`useKeymapStore`(`register` / `unregister` / `dispatch` / `hints`),`KeyBinding` / `KeyHint` 类型随文件导出。
- [ ] `src/composables/useKeymap.ts`:`useKeymap(bindings)`(mount 登记 / unmount 注销 / watch 重登记)与 `useKeymapListener()`(window keydown 单点监听,Tab 拦截、isComposing / alt / meta / shift 放过)。
- [ ] `src/composables/useRowNavigation.ts`:`selectedIndex`、`select(i)`、`reset()`,内部 `useKeymap` 登记四个方向键(label「选择」)与 Enter(label「打开」);接收 `getSections`、`columns`、`onActivate`。
- [ ] `src/lib/window.ts` + `src/composables/useAutoHeight.ts`。
- 验证:`bun run lint && bun run build`。

## 4. 组件

- [ ] `src/components/common/KeyboardKey.vue`。
- [ ] `src/components/launcher/SearchInput.vue`(`defineModel<string>()`、`defineExpose({ focus })`、`onMounted` 自动聚焦)。
- [ ] `HomeSearchBar.vue` / `ToolSearchBar.vue`(后者 Backspace 空值 → `close`,徽章按钮 `@mousedown.prevent`)。
- [ ] `ToolTile.vue` → `ToolSection.vue` → `ResultsPanel.vue`(含空态)。
- [ ] `LauncherFooter.vue`(`storeToRefs(useKeymapStore()).hints` + 末尾 Esc;左侧可选错误 / 提示文案)。
- [ ] `LauncherPanel.vue`(设计 §3 数据流)。
- [ ] `App.vue` 改根;`main.ts` 装 Pinia;删除 `src/components/HelloWorld.vue`。
- [ ] `src-tauri/capabilities/default.json` 追加 `core:window:allow-hide`、`core:window:allow-set-size`,更新 description。
- 验证:`bun run format && bun run format:check && bun run lint && bun run test && bun run build`。

## 5. 手工验收

- [ ] `bun run dev` 浏览器:AC1 ~ AC6(深浅色各看一次:系统切换或 DevTools 模拟 `prefers-color-scheme`)。
- [ ] `bun run tauri dev`:AC7(Esc 隐藏;主页搜索过滤时窗口高度变化;进入工具页窗口高 600)。
- [ ] AC9 grep:
  ```bash
  grep -rn "@tauri-apps/api" src --include=*.vue --include=*.ts | grep -v "src/lib/"
  grep -rnE "bg-(zinc|slate|gray|red|blue)-|dark:|\bz-[0-9]|text-\[|bg-\[" src --include=*.vue
  ```
  两条都应无输出。

## 6. 收尾(Phase 3)

- [ ] 3.3 spec 回写:`directory-structure.md` 增加 `tools/`、`stores/`、`lib/launcher/`、`lib/window.ts`、`components/common|launcher/`;`ipc-guidelines.md` 补「`@tauri-apps/api/window` 只允许在 `src/lib/window.ts`」;`styling-guidelines.md` / `component-guidelines.md` 中 `HelloWorld.vue` 样板引用改指向新组件;`frontend/index.md` 关键决策补「快捷键登记表用 Pinia」。
- [ ] 3.4 提交:`feat(launcher): 启动器前端壳与工具注册契约`。

## 风险文件 / 回滚点

- 修改的既有文件只有:`src/App.vue`、`src/main.ts`、`package.json` + `bun.lock`、`src-tauri/capabilities/default.json`;删除 `src/components/HelloWorld.vue`。其余全部新增。回滚:`git checkout -- <上述文件>` + 删除新增目录。
- 步骤 1 结束是第一个安全点(纯函数 + 测试,无 UI 影响);步骤 3 结束是第二个安全点(尚未改 App.vue)。

## 子代理分工建议(Phase 2)

- 步骤 1 + 2 可交给一个 `trellis-implement`(纯 TS,测试驱动);步骤 3 + 4 交给第二个(依赖前者的类型);`trellis-check` 在步骤 4 结束后做全量检查(AC8 / AC9 + 五条门禁)。
