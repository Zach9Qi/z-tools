# 工具模块规范(如何新增一个工具)

> 启动器与工具之间**唯一**的耦合点是 `src/types/tool.ts` 的契约 + `src/tools/registry.ts` 的登记。启动器不认识任何具体工具;工具不 import 启动器内部组件。本文件基于 `src/types/tool.ts`、`src/tools/registry.ts`、`src/tools/icons.ts`、`src/tools/clipboard/`(当前唯一的工具,也是新工具的样板)的真实实现。

---

## 1. 契约(`src/types/tool.ts`)

### 1.1 `ToolItem` —— 工具面向搜索 / 展示的元数据

| 字段 | 含义 | 注意 |
|---|---|---|
| `id` | 唯一标识,`registry.moduleOf()` 按它查模块 | kebab-case;重复 id 会让后登记者覆盖前者且无报错 |
| `title` | 磁贴标题,同时参与搜索匹配 | 磁贴标题区两行定高(`line-clamp-2`),过长会被截断 |
| `icon` | lucide 图标名 | 必须是 `tools/icons.ts` 里登记过的键,否则回退 `puzzle`(见 §4) |
| `keywords` | 搜索别名 | 与 `title` 一起做小写子串匹配;放英文缩写 / 拼音 / 同义词 |
| `accepts?(query)` | 动态匹配 | 返回 `true` 时进入「匹配结果」分区;收到的 `query` **已经 `trim()` + 小写**,不必再处理。只在「输入本身就是工具要处理的内容」(URL、路径、算式)时提供,名字能搜到的不要写 |
| `action` | `"view"` \| `"launch"` | 判别字段,`isViewModule()` 以它为准而非「是否有 page」,与 `item` 上的声明保持单一真相 |

### 1.2 `ViewToolModule` —— 激活后进入工具页

- `item.action === "view"`。
- `page: Component`:工具页组件,**必须**接收 prop `query: string`(见 §5)。
- `placeholder?: string`:工具页态搜索栏占位文案;不给则面板用「搜索…」。
- 类型上 `run?: never`,写了 `run` 就推断不成 view 型——两种模块互斥是故意的,一个工具只能是一种行为。

### 1.3 `LaunchToolModule` —— 激活后执行一次,不进入工具页

- `item.action === "launch"`。
- `run(ctx: { query: string }): Promise<void>`:`ctx.query` 是激活时的主页搜索词(原始输入,未 trim)。失败**直接 reject**,不要自己 `try/catch` 吞掉——面板统一 `console.error("启动工具失败:", e)` 并在页脚以 `text-destructive` 展示 `String(e)`;成功后面板显示一次性提示「已触发:<title>」,工具不必自己做提示。
- 类型上 `page?: never`、`placeholder?: never`。

## 2. 目录

```text
src/tools/<id>/                       # 现例 src/tools/clipboard/
├── index.ts            # 必须:导出该工具的 ToolModule(可以是多个);clipboard 导出一个 ViewToolModule `clipboardTool`
├── XxxPage.vue         # view 型的工具页组件;`ClipboardPage.vue`:单根 <section>,只管布局 / 键位登记 / 哨兵观察,状态全在 composable
├── components/         # 可选:工具私有组件;`ClipboardTabs.vue` / `ClipboardItemRow.vue` / `ClipboardItemDetail.vue`
├── composables/        # 可选:工具私有 composable;`useClipboardHistory.ts`(页面状态拥有者,见 composable-guidelines.md §2.1)
└── lib/                # 可选:工具私有纯函数 + 同目录 *.test.ts;`format.ts`(formatRelativeTime / formatBytes / summarizeFiles / isExpandable)+ `format.test.ts`
```

- 工具目录是一个**领域包**:组件 + 逻辑 + 类型都放在里面,不散到顶层 `components/` / `lib/`。只有 ≥2 个工具共用的东西才提升到顶层(与 `directory-structure.md` 「仅当 ≥2 个模块使用才提升」同一原则)。例外是 IPC 镜像类型(`src/types/clipboard.ts`)与命令封装(`src/lib/api/clipboard.ts`):它们属于 IPC 层而不属于工具,按 `ipc-guidelines.md` 放顶层。
- 工具内部 import 顶层通用设施是允许的:`@/components/common/*`、`@/composables/*`(`useKeymap` / `useTauriEvent`)、`@/types/*`、`@/lib/api`(或 `@/lib/api/<domain>`)、`@/lib/events`、`@/lib/runtime`。
- 工具内部相对 import(`./components/...`、`../lib/format`)与 `@/` 绝对 import 共存:包内用相对路径,跨包用 `@/`。

## 3. 登记(`src/tools/registry.ts`)

```ts
// 现状
import { clipboardTool } from "@/tools/clipboard";
export const modules: ToolModule[] = [clipboardTool];
// 新增工具时
import { myTool } from "@/tools/my-tool";
export const modules: ToolModule[] = [clipboardTool, myTool];
```

- 手工维护 `modules` 数组;`catalog`(供搜索 / 网格)与 `moduleOf(item)` 由它派生,不用另改。
- **数组顺序即主页「全部工具」分区的展示顺序**,把常用的放前面。
- 不做自动扫描(`import.meta.glob`):工具数量是个位数到几十,显式数组让顺序可控、让 `vue-tsc` 能检查每个模块的形状。
- `moduleOf()` 对未注册 id 抛错而不是返回 `undefined`:`catalog` 里的条目都来自 `modules`,找不到只可能是有人绕过 registry 手工构造了 `ToolItem`,那是编程错误。

## 4. 图标(`src/tools/icons.ts`)

- `ToolItem.icon` 写 lucide 图标名(与 lucide 官网一致,kebab-case,如 `"file-text"`);`iconOf(name)` 返回组件,未登记回退 `puzzle`。
- **为什么是手工映射表**:`unplugin-icons` 在编译期把 `~icons/lucide/xxx` 内联成 SVG 组件,只能静态 import,无法按运行时字符串加载。所以新图标要在 `icons.ts` 加一行 import + 一行表项。
- 回退而不抛错:图标缺失只是视觉问题,不应让整个网格渲染失败;看到 puzzle 图标就去 `icons.ts` 补登记。
- **`icons.ts` 只登记经 `ToolItem.icon` 使用的图标**(现为 `clipboard` + 回退 `puzzle` 两项);工具页内部自己用的图标(星标、chevron、类型图标等,不经 `ToolItem.icon`)直接在工具组件里 `import IconX from "~icons/lucide/x"`(`ClipboardItemRow.vue` 的 `star` / `chevron-down` / `file-text` / `image` / `files`),不走这张表。删工具时同步删它在 `icons.ts` 的登记,不留无人使用的表项。

## 5. view 型工具页契约

- 接收 prop `query: string`:工具页态搜索栏的当前输入,由面板透传(`<component :is="module.page" :query="toolQuery" />`)。从「匹配结果」分区激活时,`query` 初值是用户在主页输入的内容;从其他分区(「全部工具」/「搜索结果」)激活时为空串——名称命中时搜索词只是工具名,不是要处理的内容。
- **根元素不必自己写撑满 / 滚动类**:面板在 `<component>` 上统一加了 `flex min-h-0 flex-1 flex-col`,工具页根元素会被 fallthrough 合并这些类;内部溢出时自己 `overflow-y-auto`(`ClipboardPage.vue`:根 `<section>` 不写任何 class,列表容器 `min-h-0 flex-1 overflow-y-auto`)。要求工具页组件是**单根**,否则 class 无法 fallthrough。
- 面板在工具页态强制 `h-150`(600px),工具页可用高度 = 600 − 搜索栏 64 − 页脚 40。
- 页面内快捷键用 `useKeymap([...])` 登记,会**自动出现在页脚提示**并随工具页卸载自动注销;不要自己 `addEventListener("keydown")`(见 `composable-guidelines.md` §4)。方向键 / Enter 在工具页态没有被网格占用(`ResultsPanel` 已卸载),工具页可自由登记。现例 `ClipboardPage.vue` 登记 6 组:`ArrowUp`/`ArrowDown` 选择、`Enter` 粘贴、`Delete` 删除、`Ctrl+P` 收藏、`Ctrl+F` 只看收藏、`Ctrl+ArrowLeft`/`ArrowRight` 切换分类(`ctrl: true` 修饰)。焦点常驻搜索框,单字母 / 空格 / 单独方向键以外的键会与输入冲突,新快捷键优先用 Ctrl 组合。
- 已被面板占用、工具页不要再登记的键:`Escape`(返回主页)、空输入时的 `Backspace`(返回主页)、`Tab`(被无条件拦截)。
- 焦点常驻搜索栏:工具页里的可点击元素加 `@mousedown.prevent`,除非它本身就是需要输入的控件(`ClipboardItemRow.vue` 的行主体 / 星标 / chevron 三个按钮都如此)。行内子按钮用 `@click.stop` 防止冒泡到行的主动作;`<button>` 不能嵌套,行主体与子按钮是兄弟而不是父子。
- 工具页需要调后端时走 `@/lib/api`(或 `@/lib/api/<domain>`),遵守 `ipc-guidelines.md`;不在工具页里 `invoke`。页面状态与命令编排收进工具私有 composable(`useClipboardHistory`),页面组件只做绑定。
- 列表类工具页的分页用底部哨兵 + `IntersectionObserver`(`root` 为列表滚动容器),哨兵常渲染、观察器只挂一次、`onUnmounted` `disconnect`;IO 只在进出时回调,一页加载完后要 `watch(loading)` + `nextTick` 补查一次哨兵是否仍在视口内(`ClipboardPage.vue` 的 `sentinelInView`)。

## 6. 新增一个工具的步骤

1. `src/tools/<id>/index.ts` 导出 `ToolModule`(view 型再写 `XxxPage.vue`)。
2. `src/tools/icons.ts` 登记用到的 lucide 图标(如尚未登记)。
3. `src/tools/registry.ts` 的 `modules` 数组追加一项。
4. 需要后端命令 → 按 `../guides/ipc-contract.md` 走两侧变更清单,前端封装放 `src/lib/api/<domain>.ts`。
5. `bun run format && bun run format:check && bun run lint && bun run test && bun run build`。

除第 4 步外,不应碰 `src/components/launcher/*`、`src/composables/*`、`src/stores/*`;要是发现非改不可,说明契约缺了东西,先改 `types/tool.ts` 并回写本文件。剪贴板工具落地时确实只碰了这几处 + 第 4 步的 IPC 层(`lib/api/clipboard.ts`、`lib/events.ts`、`types/clipboard.ts`),可作为对照。

删除一个工具(早期的 demo 占位工具就是这样移除的):删目录 + `registry.ts` 去掉 import 与数组项 + `icons.ts` 去掉只有它用的图标;`grep -rn "<id>" src` 确认无残留(测试 fixture 里自造的同名字符串除外)。

## 7. 禁止

- 工具 import 启动器内部组件(`@/components/launcher/*`)或直接读写 `LauncherPanel` 的状态;工具与启动器只经 `ToolModule` 契约通信。
- 工具内直接 `invoke` / import `@tauri-apps/api/*`(走 `@/lib/api`、`@/lib/window`)。
- `run()` 内部吞掉错误后 resolve(面板需要 reject 才能展示错误)。
- 在 `ToolItem.icon` 写未登记的图标名后依赖 puzzle 回退当作「没问题」。
- 绕过 `registry.ts` 在别处手工构造 `ToolItem` 塞进网格。
- 重复登记 `Escape` / `Backspace` / `Tab`。
- 用 `import.meta.glob` 自动扫描 `tools/*` 代替手工数组。
