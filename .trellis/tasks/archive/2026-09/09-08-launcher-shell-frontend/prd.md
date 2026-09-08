# 启动器前端布局与工具注册契约

## Goal

参考 `C:/Users/chenziqi/Desktop/codes/rust/zach-tools` 的启动器**布局**,在 z-tools 中实现个人工具箱启动器的前端壳与工具注册契约;**样式不照搬**,全部按本仓库 `.trellis/spec/frontend/styling-guidelines.md` 三层令牌 / 交互焦点范式重做。用户价值:打开应用即得到一个可搜索、可键盘操作、可挂载工具页的启动器骨架,后续工具只需按契约注册即可出现在网格中。

## Background(已确认的事实)

### 参考项目布局(zach-tools,只取结构,不取配色)

- 面板 `LauncherPanel`:页面根即面板,宽随窗口(800 逻辑 px),`max-h-150`(600px)上限;主页态高度随内容,工具页态强制 `h-150` 撑满、内部滚动。
- 搜索栏 `h-16`,`data-tauri-drag-region`;左侧搜索图标,中间 `<input>` `text-lg tracking-tight` 透明底,右侧 `<kbd>` 快捷键提示(Alt + Enter)。
- 结果区 `flex min-h-0 flex-1 flex-col border-y p-4`;分区 = `header h-5`(`h2 text-xs font-semibold tracking-wide` + 右侧 action 插槽)+ `grid grid-cols-8` 磁贴网格。
- 磁贴 `ToolTile`:纵向 icon 盒(`size-11 rounded-xl border`,内 icon)+ 标题(`line-clamp-2 min-h-8 text-center text-xs font-medium`)。
- 空结果:居中,icon 盒 `rounded-2xl`,文案 `text-sm`。
- 工具页态:搜索栏左侧换成「工具徽章按钮」(icon + 标题,点击返回主页),后接 `<input>`(placeholder 由工具提供);下方 `<component :is="module.page" :query="toolQuery" />`。
- 页脚 `footer h-10 border-t px-4`,右对齐,`text-xs` 键位提示列表,每条 `kbd×n + label`,末尾固定 `Esc`(主页「隐藏」/ 工具页「返回」)。
- `KeyboardKey`:`<kbd>` 小圆角描边、`font-medium`。
- 行为:子串匹配(title / keywords `includes`)+ `accepts?(query)` 动态匹配;搜索态分区「匹配结果」→「搜索结果」;一维 `selectedIndex` + 按 8 分行的二维方向键导航(左右首尾回绕、上下就近保列并回绕);Enter 激活;鼠标 hover 选中;`useKeymap` 集中登记快捷键并驱动页脚提示;Esc 主页隐藏窗口 / 工具页返回;工具页输入框为空时 Backspace 返回;`useAutoHeight` 用 ResizeObserver 把面板高度同步给窗口 `setSize`;搜索框在视图切换后自动聚焦。
- 工具契约:`ToolItem { id, title, icon, keywords, accepts?, action: "view" | "launch" }`;`ViewToolModule { item, page, placeholder? }`;`LaunchToolModule { item, run(ctx) }`;`registry.ts` 手工数组 + `moduleOf(item)`;`icons.ts` 手工映射 lucide 图标名 → 组件,未命中回退 `puzzle`。
- 剪贴板工具、sqlx / arboard / 粘贴 / 图片存储等全部不属于启动器,不搬。

### 本仓库现状(z-tools)

- Tauri 2 + Vue 3.5 + Vite 8 + Tailwind 4 + `unplugin-icons`(lucide)+ Vitest + oxlint + Prettier(含 tailwind 排序插件),包管理 bun;**未安装 Pinia**。
- `src/` 仅 `App.vue`(渲染 `HelloWorld`)、`components/HelloWorld.vue`、`lib/api.ts`(greet 封装)、`lib/runtime.ts`(`isTauriRuntime`)、`index.css`。
- `index.css` 已有完整三层令牌:`background / foreground / card / popover / primary / secondary / muted / accent / destructive / border / input / ring`(均 `light-dark()`)、`--radius: 0.5rem` 派生 sm~2xl、`--z-*` 六档、`font-sans / font-mono`;基础层已处理 `color-scheme`、细滚动条、`::selection`、reduced-motion、`button cursor`。
- `tauri.conf.json5` 主窗口:`800×600, center`,默认有边框 / 不透明;`capabilities/default.json` 仅 `core:default`。
- 规范约束(实现必须遵守):
  - 组件只用语义令牌工具类;焦点 `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`;hover / 选中叠加底用 `accent`;静态表面 `bg-card border`;字号只用默认档位(参考项目的 `text-2xs` 不引入);间距走 4px 网格(参考项目的 `px-5.5 / gap-3.5 / size-5.5 / h-4.5 / gap-4.5 / scale-96` 一律取整到默认档位);`z-(--z-*)`;拖拽区 `data-tauri-drag-region` + `select-none`,不在 body 全局 `user-select: none`。
  - `invoke` 只允许出现在 `src/lib/api.ts` / `src/lib/api/**`;`.vue` / store 不 import `@tauri-apps/api/*`。
  - 跨不相邻组件共享的状态用 Pinia setup store,禁止模块级 `ref` / `reactive` 单例,禁止 `provide/inject` 传业务状态(参考项目的 `useKeymap` / `useToolView` 模块级单例写法**不能照搬**)。
  - 组件放 `src/components/`(同领域 ≥5~6 个建子目录),composable 放 `src/composables/`,共享类型放 `src/types/`,纯函数放 `src/lib/` 并配同目录 `*.test.ts`(Vitest 无 DOM 环境,只测纯函数)。
  - 所有注释 / 文案中文,标识符英文;`console` 只允许 `error` / `warn`。

## Decisions(已由用户拍板)

- **D1 范围**:本任务只做前端启动器布局 + 工具注册契约;Rust 侧窗口管理留后续任务。
- **D2 演示工具**:加入 **1 个 `view` 型占位工具**(占位页面,回显 `query`)+ **多个 `launch` 型占位工具**(`run()` 只在页面内给出提示,不调后端),总数 ≥ 9 以便主页网格出现第二行换行;全部标注为示例,放在 `src/tools/demo/`,后续真实工具进来时直接删除。
- **D3 窗口 API**:本任务**直接接 Tauri 窗口 API**——主页 Esc → `getCurrentWindow().hide()`;`useAutoHeight` → `setSize(LogicalSize)`;`capabilities/default.json` 追加 `core:window:allow-hide`、`core:window:allow-set-size`。浏览器预览下二者降级为 no-op。

## Requirements

### R1 工具注册契约(`src/types/tool.ts` + `src/tools/`)

- R1.1 `ToolItem`:`id`(唯一)、`title`、`icon`(lucide 图标名)、`keywords: string[]`、`accepts?(query): boolean`、`action: "view" | "launch"`。
- R1.2 `ViewToolModule`:`item.action === "view"`,`page: Component`(接收 prop `query: string`),`placeholder?: string`。
- R1.3 `LaunchToolModule`:`item.action === "launch"`,`run(ctx: { query: string }): Promise<void>`。
- R1.4 `ToolModule = ViewToolModule | LaunchToolModule`,提供 `isViewModule()` 类型谓词。
- R1.5 `src/tools/registry.ts`:手工维护 `modules: ToolModule[]`,导出 `catalog: ToolItem[]` 与 `moduleOf(item)`(未注册 id 抛错)。
- R1.6 `src/tools/icons.ts`:`iconOf(name)` 返回图标组件,未登记的名字回退 `puzzle`。
- R1.7 演示工具见 D2;每个工具目录自带 `index.ts` 导出其 `ToolModule`。

### R2 主页态布局与搜索

- R2.1 面板为页面根,宽 100%,`max-h-150`;静态表面样式(`bg-card text-card-foreground border rounded-2xl`)。
- R2.2 搜索栏 `h-16`,带 `data-tauri-drag-region select-none`:搜索图标 + 透明底输入框(`text-lg tracking-tight`,`outline-hidden`,自动聚焦)+ 右侧 `<kbd>Alt</kbd><kbd>Enter</kbd>` 提示。
- R2.3 结果区 `border-y p-4`,可滚动;分区标题行 `h-5`(`text-xs font-semibold tracking-wide text-muted-foreground`)+ `grid grid-cols-8` 磁贴网格。
- R2.4 主页无搜索词时显示单一分区「全部工具」= `catalog` 全部条目(无使用历史,不做「最近使用 / 已固定」)。
- R2.5 有搜索词时:分区「匹配结果」= `accepts?.(query)` 为真且未命中名称的条目;分区「搜索结果」= `title` 或任一 `keywords` 小写子串包含 `query`;空分区不渲染;两者皆空显示空态(居中图标盒 + `text-sm text-muted-foreground` 文案)。
- R2.6 磁贴:`<button>`,纵向 icon 盒(`size-11 rounded-xl border`)+ 两行定高标题;选中态用 `bg-accent text-accent-foreground`,hover 同;焦点按规范 ring;`@mousedown.prevent` 防止抢搜索框焦点;`mouseenter` 选中。
- R2.7 页脚 `h-10 border-t px-4` 右对齐,展示当前已登记快捷键提示(`kbd×n + label`)+ 固定末尾 `Esc 隐藏`(主页)/ `Esc 返回`(工具页)。

### R3 键盘交互

- R3.1 方向键在展平后的条目列表上导航:`←/→` ±1 且首尾回绕(跨行);`↑/↓` 按每行 8 个跳行,列号就近保留(短行贴行尾),顶底回绕。
- R3.2 `Enter` 激活选中项;搜索词变化时选中回到 0。
- R3.3 `Tab / Shift+Tab` 被拦截(焦点常驻搜索框);输入法 `isComposing` 期间不处理任何快捷键;带 `alt / meta / shift` 的组合键放过;`ctrl` 必须与绑定声明一致。
- R3.4 `useKeymap(bindings)`:组件挂载时登记、卸载时注销自己那份;页脚提示由所有已登记绑定派生(`ArrowUp→↑`、`Enter→↵`、`Delete→Del`,单字母大写,`Ctrl` 前置)。
- R3.5 `Esc`:主页 → 调用窗口隐藏(D3);工具页 → 返回主页。工具页输入框为空时 `Backspace` → 返回主页。

### R4 工具页态

- R4.1 激活 `view` 型:进入工具页,搜索栏变为「工具徽章按钮(icon + title,点击返回)+ 输入框(placeholder 来自模块,`v-model` 为 `toolQuery`)+ 快捷键提示」;若从「匹配结果」分区激活,把主页搜索词带入 `toolQuery`,否则为空;主页搜索词清空。
- R4.2 工具页态面板强制 `h-150`,页面区域 `flex min-h-0 flex-1 flex-col` 内部滚动;`<component :is="module.page" :query="toolQuery" />`。
- R4.3 激活 `launch` 型:`await module.run({ query })`,错误 `console.error("启动工具失败:", error)` 并在页脚 / 面板内展示 `text-destructive` 文案(不抛出)。
- R4.4 返回主页时清空 `toolQuery`、`activeModule`,搜索框重新聚焦。

### R5 窗口交互(D3)

- R5.1 `src/lib/window.ts`:`hideLauncher()`、`resizeLauncherToContent(height)`,内部使用 `@tauri-apps/api/window`;非 Tauri 运行时 no-op;这是**唯一**允许 import `@tauri-apps/api/window` 的文件。
- R5.2 `useAutoHeight(elRef)`:ResizeObserver 观察面板根,`offsetHeight` 变化时调 `resizeLauncherToContent(Math.ceil(h))`,宽度取 `window.innerWidth`;卸载时 disconnect。
- R5.3 `capabilities/default.json` 追加 `core:window:allow-hide`、`core:window:allow-set-size`。

### R6 工程

- R6.1 `App.vue` 根改为 `<main class="h-screen w-screen overflow-hidden">` + `LauncherPanel`;删除 `HelloWorld.vue`(保留 `lib/api.ts` 的 `greet` 与 Rust `greet` 命令不动,避免扩大范围)。
- R6.2 引入 `pinia`(runtime dependency),`main.ts` 装配;共享状态只有「快捷键登记表」进 store。
- R6.3 纯逻辑(搜索分区、导航下标计算、键位标签格式化)放 `src/lib/launcher/*.ts` 并配 `*.test.ts`。
- R6.4 `bun run format && bun run format:check && bun run lint && bun run test && bun run build` 全绿;`cargo` 侧无改动。

## Acceptance Criteria

- [ ] AC1 `bun run dev` 浏览器预览打开即见启动器面板:搜索栏 / 「全部工具」分区(≥9 个磁贴,一行 8 个,第二行有换行)/ 页脚提示;深浅色跟随系统均可区分面板 / 页面底 / 选中态。
- [ ] AC2 输入 `demo` 等关键词,网格实时过滤为「搜索结果」;输入无匹配文本出现空态;清空恢复「全部工具」。
- [ ] AC3 `←/→/↑/↓` 按 R3.1 规则移动选中高亮(含回绕与跨行);鼠标悬停同步选中;`Enter` 激活。
- [ ] AC4 激活 view 型占位工具进入工具页:搜索栏出现徽章 + 新 placeholder,占位页回显输入;`Esc` / 空输入 `Backspace` / 点击徽章均返回主页并重新聚焦搜索框。
- [ ] AC5 激活 launch 型占位工具:页面内出现一次性提示文案,不进入工具页。
- [ ] AC6 页脚提示随登记变化:主页显示「↑↓←→ 选择 · ↵ 打开 · Esc 隐藏」;工具页末尾为「Esc 返回」。
- [ ] AC7 `bun run tauri dev` 下:主页 `Esc` 隐藏窗口;主页内容变化时窗口高度随面板变化,工具页态窗口高 600。
- [ ] AC8 `src/lib/launcher/*.test.ts` 覆盖:名称匹配 / accepts 匹配 / 空分区剔除;左右回绕、上下保列与短行贴行尾、顶底回绕;键位标签映射。
- [ ] AC9 R6.4 五条门禁全绿;`grep` 确认 `.vue` / store / composable 中无 `@tauri-apps/api` import,组件中无原始色 / `dark:` / 裸 `z-`。

## Out of Scope

- Rust 侧窗口管理:透明无边框 / 置顶 / 跳过任务栏 / 启动隐藏 / 失焦隐藏 / 托盘菜单 / 全局快捷键 / `SC_KEYMENU` 拦截 / `hide_launcher` 命令 / `tauri.conf.json5` 窗口属性调整。
- 任何真实工具(剪贴板等)的实现。
- 使用历史 / 固定工具及其持久化。
- 组件级单测(需另开任务引入 `@vue/test-utils` + `happy-dom`)。

## Risks / Deferred

- 在有边框窗口下 `Esc` 隐藏后没有托盘 / 全局快捷键可唤回,`tauri dev` 需重启进程;窗口任务落地后消失。
- 可拉伸窗口被 `useAutoHeight` 覆盖尺寸,拖大后会回缩;窗口任务将设 `resizable: false`。
- `data-tauri-drag-region` 在有边框窗口下无害;透明无边框后才体现作用。
