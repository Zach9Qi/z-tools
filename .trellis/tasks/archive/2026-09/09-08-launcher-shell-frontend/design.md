# 技术设计:启动器前端壳与工具注册契约

> 需求见 `prd.md`;本文只写「怎么做」。参考实现 zach-tools 的结构在 prd Background 中,这里只列与之**不同**的决定与本仓库的落点。

## 1. 边界与目录

```text
src/
├── App.vue                          # <main class="h-screen w-screen overflow-hidden"><LauncherPanel /></main>
├── main.ts                          # createApp(App).use(createPinia()).mount("#app")
├── components/
│   ├── common/
│   │   └── KeyboardKey.vue          # <kbd> 通用键帽
│   └── launcher/                    # 启动器领域组件(≥6 个,按 directory-structure 建子目录)
│       ├── LauncherPanel.vue        # 壳:持有视图状态,组合下面所有组件
│       ├── SearchInput.vue          # 透明底输入框 + 自动聚焦 + focus() expose
│       ├── HomeSearchBar.vue        # 主页搜索栏
│       ├── ToolSearchBar.vue        # 工具页搜索栏(徽章 + 输入框)
│       ├── ResultsPanel.vue         # 分区列表 / 空态
│       ├── ToolSection.vue          # 分区标题 + grid
│       ├── ToolTile.vue             # 磁贴按钮
│       └── LauncherFooter.vue       # 页脚键位提示
├── composables/
│   ├── useKeymap.ts                 # 登记快捷键到 store + window keydown 监听(仅面板调用一次监听)
│   ├── useRowNavigation.ts          # selectedIndex + 方向键绑定(调用 lib/launcher/navigation 纯函数)
│   └── useAutoHeight.ts             # ResizeObserver → lib/window.resizeLauncherToContent
├── stores/
│   └── keymap.ts                    # useKeymapStore:已登记绑定表 + hints 派生
├── lib/
│   ├── window.ts                    # 唯一允许 import @tauri-apps/api/window 的文件;浏览器降级 no-op
│   └── launcher/
│       ├── search.ts (+ .test.ts)   # buildSections(catalog, query) / stampSections / flatten
│       ├── navigation.ts (+ .test.ts) # nextIndex(direction, index, total, columns)
│       └── keyLabels.ts (+ .test.ts)  # formatKeyLabel("ArrowUp") → "↑" 等
├── types/
│   └── tool.ts                      # ToolItem / ViewToolModule / LaunchToolModule / ToolModule / isViewModule
└── tools/
    ├── registry.ts                  # modules 数组、catalog、moduleOf
    ├── icons.ts                     # iconOf(name)
    └── demo/
        ├── index.ts                 # 导出 demoViewTool + demoLaunchTools(≥8 个 launch)
        └── DemoToolPage.vue         # view 型占位页:回显 query
```

依赖方向沿用 `directory-structure.md`:`components → composables/stores → lib`;`lib/` 不 import 组件、store。`tools/` 是新增顶层目录(工具 = 组件 + 逻辑的领域模块,组件目录放不下),`tools/*` 可以 import `components/`、`composables/`、`types/`;`components/launcher/*` 只通过 `tools/registry.ts` 与工具相遇。Phase 3.3 需把 `tools/`、`lib/launcher/`、`lib/window.ts` 回写到 `directory-structure.md` / `ipc-guidelines.md`。

## 2. 契约

### 2.1 `src/types/tool.ts`

```ts
import type { Component } from "vue";

export interface ToolItem {
  /** 唯一 id,registry 按此查模块 */
  id: string;
  title: string;
  /** lucide 图标名,经 tools/icons.ts 解析;未登记回退 puzzle */
  icon: string;
  /** 搜索时与 title 一起做小写子串匹配 */
  keywords: string[];
  /** 动态匹配:对搜索词返回 true 时进入「匹配结果」分区(如 URL / 路径) */
  accepts?: (query: string) => boolean;
  action: "view" | "launch";
}

export interface ViewToolModule {
  item: ToolItem & { action: "view" };
  /** 工具页组件,必须接收 prop `query: string` */
  page: Component;
  placeholder?: string;
  run?: never;
}

export interface LaunchToolModule {
  item: ToolItem & { action: "launch" };
  run: (ctx: { query: string }) => Promise<void>;
  page?: never;
  placeholder?: never;
}

export type ToolModule = ViewToolModule | LaunchToolModule;
export function isViewModule(m: ToolModule): m is ViewToolModule;
```

### 2.2 `src/lib/launcher/search.ts`(纯函数,可测)

```ts
export interface Section { id: "all" | "matches" | "named"; title: string; items: StampedItem[] }
export interface StampedItem { item: ToolItem; index: number }  // index = 全局展平下标

export function buildSections(catalog: ToolItem[], query: string): Section[];
// query.trim()==="" → [{ id:"all", title:"全部工具", items: catalog }]
// 否则 named = title/keywords 小写 includes(q);matches = !named && accepts?.(q);
// 顺序 matches → named;空分区剔除;最后统一 stamp 全局下标
export function flattenSections(sections: Section[]): StampedItem[];
```

### 2.3 `src/lib/launcher/navigation.ts`(纯函数,可测)

```ts
export type Direction = "left" | "right" | "up" | "down";
/** 按分区分行(每行最多 columns 个),返回移动后的全局下标;total===0 返回 -1 */
export function nextIndex(dir: Direction, current: number, rows: number[][]): number;
export function chunkRows(sections: Section[], columns: number): number[][]; // 每行装的是全局下标
```

规则(与 prd R3.1 一致):`left/right` 在展平序列 ±1 首尾回绕;`up/down` 找当前所在行 → 目标行(顶底回绕)→ `min(col, targetRow.length-1)`。

### 2.4 `src/lib/launcher/keyLabels.ts`

```ts
export const KEY_LABELS: Record<string, string> = { ArrowUp:"↑", ArrowDown:"↓", ArrowLeft:"←", ArrowRight:"→", Enter:"↵", Delete:"Del", Escape:"Esc", Backspace:"⌫" };
export function formatKeyLabel(key: string): string; // 未命中:单字母大写,否则原样
```

### 2.5 快捷键:`stores/keymap.ts` + `composables/useKeymap.ts`

```ts
// 类型放 types/keymap.ts?——只有 store 与 composable 两个模块用,直接放 stores/keymap.ts 导出
export interface KeyBinding {
  keys: string[];        // KeyboardEvent.key 列表,任一命中
  ctrl?: boolean;        // 必须与事件 ctrlKey 一致
  label: string;         // 页脚文案
  hint?: boolean;        // 默认 true;false 时不出现在页脚(Esc 由页脚固定渲染)
  onPress: (e: KeyboardEvent) => void;
}
export interface KeyHint { keys: string[]; label: string }   // keys 已经过 formatKeyLabel,Ctrl 前置

useKeymapStore(): {
  bindings: Ref<Map<symbol, KeyBinding[]>>;   // 每次 useKeymap 调用一份,symbol 为登记句柄
  hints: ComputedRef<KeyHint[]>;              // 按登记顺序展开,过滤 hint===false
  register(bindings): symbol; unregister(handle): void;
  dispatch(e: KeyboardEvent): boolean;        // 返回是否命中(已 preventDefault + onPress)
}
```

- `useKeymap(bindings: MaybeRefOrGetter<KeyBinding[]>)`:`onMounted` register / `onUnmounted` unregister;`watch` 变化时重新登记(工具页切换 label)。**不**在这里挂 window 监听。
- `LauncherPanel` 单独调用 `useKeymapListener()`(同文件导出的第二个 composable):挂 `window keydown`,规则:`isComposing` → 放过;`Tab` → 无条件 `preventDefault`;`alt/meta/shift` → 放过;否则 `store.dispatch(e)`。卸载时移除。
- 这样「登记表」是 Pinia store(符合 state-management),监听只有一处,页脚 `storeToRefs(store).hints`。

### 2.6 `src/lib/window.ts`

```ts
/** 隐藏当前窗口;浏览器预览 no-op。*/
export async function hideLauncher(): Promise<void>;
/** 把窗口内容区高度设为 height(逻辑像素),宽度取 window.innerWidth;浏览器预览 no-op。*/
export async function resizeLauncherToContent(height: number): Promise<void>;
```

失败时 `console.error("调整启动器窗口失败:", error)`,不向上抛(尺寸同步失败不应打断 UI)。

### 2.7 `useAutoHeight(target: Ref<HTMLElement | null>)`

`onMounted` 建 `ResizeObserver`,回调 `resizeLauncherToContent(Math.ceil(target.value.offsetHeight))`;`onUnmounted` disconnect。非 Tauri 环境仍观察但 `lib/window` no-op,避免分支分散。

## 3. 组件与数据流

```
LauncherPanel (状态拥有者)
  ├─ view: computed(activeModule ? "tool" : "home")
  ├─ homeQuery: ref("")     toolQuery: ref("")     activeModule: ref<ViewToolModule|null>(null)
  ├─ launchError: ref("")   # launch 型 run 失败 / 成功提示,面板底部一行 text-xs
  ├─ sections = computed(buildSections(catalog, homeQuery))
  ├─ nav = useRowNavigation(() => sections.value, 8)   # 内部 useKeymap(方向键 + Enter)
  ├─ useKeymap(() => [{ keys:["Escape"], label: view==="tool" ? "返回" : "隐藏", hint:false, onPress: onEscape }])
  ├─ useKeymapListener()
  ├─ useAutoHeight(rootRef)
  ├─ <HomeSearchBar v-model="homeQuery" ref="searchBar" />            (view==="home")
  ├─ <ResultsPanel :sections :selected-index @select="nav.select" @activate="activate" />
  ├─ <ToolSearchBar :module="activeModule" v-model="toolQuery" @close="closeTool" ref="searchBar" />  (view==="tool")
  ├─ <component :is="activeModule.page" :query="toolQuery" class="flex min-h-0 flex-1 flex-col" />
  └─ <LauncherFooter :view :error="launchError" />   # 读 store.hints 自己追加 Esc
```

- `activate(stamped, source)`:`moduleOf(item)`;view → `activeModule = m; toolQuery = source==="matches" ? homeQuery : ""; homeQuery = ""`;launch → `try { await m.run({ query: homeQuery }) } catch → launchError`。
- `closeTool()`:`activeModule = null; toolQuery = ""`;`nextTick(() => searchBar.value?.focus())`。
- 搜索栏组件用 `defineExpose({ focus })` 转发 `SearchInput` 的 focus;`SearchInput` `onMounted` 自动 `focus()`(视图切换时因 `v-if` 重建自然触发)。
- `ToolSearchBar` 内 `@keydown.backspace` 且 `modelValue===""` → `emit("close")`。
- `ResultsPanel` 只负责渲染 `sections`(空 → 空态)并透传 `select` / `activate`;`ToolSection` 接 `title` + `items` + `selectedIndex`;`ToolTile` 接 `item` + `selected`,emit `select` / `activate`。
- 高度规则:面板根 `class="flex max-h-150 w-full flex-col overflow-hidden rounded-2xl border bg-card text-card-foreground" :class="view === 'tool' && 'h-150'"`。

## 4. 样式映射(参考 → 本仓库规范)

| 元素 | 参考写法 | 本仓库写法 |
|---|---|---|
| 面板 | 透明窗口 + `rounded-2xl border` 自定色 | `bg-card text-card-foreground rounded-2xl border`(静态表面) |
| 搜索栏 | `h-16 gap-3.5 px-5.5` | `h-16 gap-4 px-6`,`data-tauri-drag-region select-none` |
| 搜索图标 | `size-5.5` | `size-5 text-muted-foreground` |
| 输入框 | `h-full text-lg tracking-tight outline-hidden` | `h-10 rounded-md bg-transparent px-1 text-lg tracking-tight placeholder:text-muted-foreground outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`(规范要求 `outline-hidden` 必配 ring;`h-10` 让环不贴搜索栏上下边) |
| 磁贴 | `rounded-xl p-2 active:scale-96`,选中自定色 | `rounded-xl p-2 transition-colors hover:bg-accent hover:text-accent-foreground outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`,选中 `:class="selected && 'bg-accent text-accent-foreground'"`,`active:scale-95` |
| 图标盒 | `size-11 rounded-xl border` | `size-11 rounded-xl border bg-background`(深色下与 card 有一档差) ,icon `size-5` |
| 标题 | `text-xs/4 min-h-8 line-clamp-2` | 同(`text-xs/4` 为默认字号 + 默认行高档位) |
| 分区标题 | `h-5 px-2.5 text-xs font-semibold tracking-wide` | `h-5 px-2 text-xs font-semibold tracking-wide text-muted-foreground` |
| 分区间距 | `gap-4.5` | `gap-4` |
| 网格 | `grid-cols-8 gap-1.5` | `grid-cols-8 gap-2` |
| 空态 | `size-13 rounded-2xl`,`text-2xs` | 图标盒 `size-12 rounded-2xl border bg-muted`,icon `size-6 text-muted-foreground`,文案 `text-sm text-muted-foreground` |
| kbd | `h-4.5 min-w-4.5 rounded-sm border px-1.5 text-2xs` | `h-5 min-w-5 rounded-sm border bg-muted px-1 font-sans text-xs font-medium text-muted-foreground` |
| 页脚 | `h-10 border-t px-4 text-xs` | 同,`text-muted-foreground`;错误文案 `text-destructive` 左侧 |
| 工具徽章按钮 | `rounded-lg border px-2.5 py-1 text-sm font-medium` | `rounded-lg border bg-secondary text-secondary-foreground px-2 py-1 text-sm font-medium hover:bg-secondary/80` + 焦点 ring |

不引入新令牌、新字号、新 z-index;不写 `<style scoped>`。

## 5. 兼容 / 迁移

- 删除 `src/components/HelloWorld.vue`;`src/lib/api.ts` 的 `greet` 与 Rust `greet` 命令保留(不扩大范围),`directory-structure.md` 中对 `HelloWorld.vue` 的样板引用在 3.3 改为指向 `ToolTile.vue` / `LauncherPanel.vue`。
- 新依赖:`pinia`(dependencies)。
- `capabilities/default.json` 追加 `core:window:allow-hide`、`core:window:allow-set-size`,description 同步更新。
- Rust 侧零改动;`tauri.conf.json5` 零改动。

## 6. 取舍记录

| 取舍 | 选择 | 理由 |
|---|---|---|
| 快捷键登记表 | Pinia store | 工具页(深层)登记、页脚(另一支)消费,属跨不相邻组件共享;规范禁模块级单例与 provide/inject |
| 视图状态(activeModule / query) | `LauncherPanel` 本地 ref + props/emits | 消费者都是直接子组件,两层以内不引 store |
| 键盘监听位置 | 面板一处 `useKeymapListener` | 避免多个 composable 各挂一个 window 监听;工具页只登记不监听 |
| 主页分区 | 单一「全部工具」 | 无使用历史,「最近使用 / 已固定」是空壳;保留 `Section.id` 联合便于后续加 |
| 窗口 API 落点 | `lib/window.ts` 直调 `@tauri-apps/api/window` | 与 `lib/api.ts` 同层同规则(唯一入口 + 浏览器降级);不建 Rust 命令(D1) |
| 图标解析 | 手工 `Record<string, Component>` | `unplugin-icons` 只能静态 import,无法按运行时字符串加载 |

## 7. 回滚

- 全部为新增文件 + `App.vue` / `main.ts` / `package.json` / `capabilities/default.json` 四处修改;回滚即 `git checkout` 这四个文件并删除新增目录,恢复 `HelloWorld.vue`。
