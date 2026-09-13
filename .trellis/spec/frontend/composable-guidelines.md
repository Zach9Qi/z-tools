# Composable 规范

> Vue 官方术语是 composable(组合式函数),目录固定 `src/composables/`(全局通用)与 `src/tools/<id>/composables/`(工具私有),不用 `hooks/`。现有四个通用 + 一个工具私有:
>
> | 文件 | 职责 |
> |---|---|
> | `useKeymap.ts` | `useKeymap(bindings)`:把一份快捷键绑定登记进 `stores/keymap`,挂载登记 / 卸载注销 / 变化重新登记;`useKeymapListener()`:挂**唯一**的 `window keydown` 监听并转给 `store.dispatch`,只由 `LauncherPanel` 调一次 |
> | `useRowNavigation.ts` | 磁贴网格的 `selectedIndex` + 方向键 / Enter 登记;下标计算全部交给 `lib/launcher/navigation.ts` 纯函数 |
> | `useAutoHeight.ts` | `ResizeObserver` 观察面板根,高度变化时调 `lib/window.resizeLauncherToContent`;卸载 `disconnect` |
> | `useTauriEvent.ts` | `useTauriEvent(EVENTS.X, handler)`:订阅一个 Rust 事件,payload 类型由 `lib/events.ts` 推导;非 Tauri 不订阅;卸载 unlisten 并处理 `listen` 晚于卸载 resolve 的竞态。**唯一**允许 import `@tauri-apps/api/event` 的文件;`LauncherPanel` 用它在 `launcher://open` 时聚焦搜索框,`useClipboardHistory` 用它订阅 `clipboard://changed`(带条目 payload)与 `launcher://open`(唤起 sync) |
> | `tools/clipboard/composables/useClipboardHistory.ts` | 剪贴板工具页的**状态拥有者**:列表 / 筛选 / 选中 / 展开 / 分页 + 后端命令与事件的编排;工具私有 composable 的样板(§2.1) |

---

## 1. 何时抽 composable

- 同一段响应式逻辑(状态 + 副作用 + 清理)在 ≥2 个组件出现。
- 组件里出现了「订阅 / 定时器 / 事件监听」这类需要 `onUnmounted` 清理的逻辑,哪怕只用一次也抽出去,让清理与订阅写在同一个函数里。
- 纯函数(无 `ref` / 无生命周期)**不是** composable,放 `src/lib/`。

## 2. 命名与形态

- 文件 `src/composables/useXxx.ts`,导出同名函数 `useXxx`;`use` 前缀是 Vue 官方约定。
- 返回**对象**而不是数组,方便调用方按名解构、按需取用(例如返回 `{ addListener, cleanup }`)。
- 返回的响应式值用 `ref` / `computed` / `readonly()`,不返回 `reactive` 对象(解构会丢响应性)。
- 接受参数时,允许传 `MaybeRefOrGetter<T>` 并用 `toValue()` 取值,让调用方既能传静态值也能传 ref。

```ts
// src/composables/useTauriEvent.ts —— 已落地,与仓库代码一致(真实文件每个局部变量带 /** */ 注释)
import { onUnmounted } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { EventPayloads } from "@/lib/events";
import { isTauriRuntime } from "@/lib/runtime";

/** 监听一个 Rust 事件,组件卸载时自动取消;处理 listen 晚于卸载 resolve 的竞态;浏览器预览下不订阅 */
export function useTauriEvent<K extends keyof EventPayloads>(
  name: K,
  handler: (payload: EventPayloads[K]) => void,
): void {
  // 非 Tauri 环境没有事件总线,listen 会报错;降级为不订阅
  if (!isTauriRuntime()) return;
  let unlisten: UnlistenFn | undefined;
  let disposed = false;
  listen<EventPayloads[K]>(name, (e) => handler(e.payload))
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    // 订阅失败只记日志不抛:少收一个事件不应让 UI 进入错误态,也不让 Promise 悬空 reject
    .catch((e) => console.error("监听事件失败:", e));
  onUnmounted(() => {
    disposed = true;
    unlisten?.();
  });
}
```

这个 composable 返回 `void` 而不是对象,是「返回对象」规则的合理省略:它没有任何可供调用方使用的状态,清理完全由生命周期接管。

> 例外:`@tauri-apps/api/event` 只允许在这类事件 composable 里 import(见 `ipc-guidelines.md`)。

### 2.1 页面状态拥有者型 composable(现例 `useClipboardHistory`)

工具页的状态与命令编排收进一个工具私有 composable,页面组件只做布局 / 绑定 / 键位登记。`useClipboardHistory(query: MaybeRefOrGetter<string>)` 是样板,它确立的套路:

| 套路 | 写法 | 为什么 |
|---|---|---|
| 入参用 `MaybeRefOrGetter` + `toValue()` | 页面传 `() => props.query` | 调用方既能传静态值也能传 getter,composable 内 `watch(() => toValue(query))` |
| 每个 `ref` 一行 `/** */` 写「代表什么 / 何时变 / 与谁互斥」 | `items` / `kind` / `favoriteOnly` / `selectedId` / `expandedId` / `loading` / `exhausted` / `error` / `resetTick` | `expandedId` 与 `selectedId` 独立这类约定只能写在声明处 |
| **列表请求只有一个入口** | `requestList({ invalidate?, clearOnError?, before?, apply })`:`loading` / `try-catch` / 过期判断 / `error` 写入 / `finally` 全在这里;调用方声明「要不要作废在途请求、失败是否清空、拉哪页、结果如何合入」 | 首屏 / 重拉 / 翻页三条路径共享同一份错误与过期处理,不会各写一遍漏一处 |
| **`generation` 只在整表重拉递增** | `const gen = invalidate ? ++generation : generation`;响应到达时 `gen !== generation` 整体忽略(含 `loading` / `error` / `finally`);翻页不递增,靠 `loading` 守卫串行;`onUnmounted` 再 `generation++` | 快速切 Tab / 连续输入只有最后一次的结果是当前筛选的;翻页与翻页之间不互相作废;卸载后的响应不应再写状态 |
| **选中以 id 持有** | `selectedId: Ref<number \| null>`,`selectedIndex` / `selected` 用 `computed` 派生;`dropItem(id)` 删的是选中项时取 `items[min(index, length - 1)]` 就近改选,删的是展开项则 `expandedId = null` | 置顶 / 删除 / 重排后选中跟着条目走,不需要 `selectedIndex--` / clamp 这类下标修正 |
| **自己发起的变更不重拉** | `remove` / `toggleFavorite` 在命令 `await` 成功后直接 `dropItem` / 翻转字段,**不碰 `exhausted`** | 结果确定,重拉只会丢掉已加载的后续页和滚动位置;后端也不为这些变更发事件 |
| **后端事件本地合并** | `clipboard://changed` 带条目:不满足 `kind` / `favoriteOnly` 忽略;有搜索词只当失效信号 `scheduleRefresh()`;否则 `placeOnTop(item)`(按 id 去重后 `unshift`) | kind / favorite 前端能判,`LIKE` 不能判就退化为重拉;粘贴写回被监听器回捕后以同 id 回来,置顶即得到「上浮」效果 |
| **筛选变化整表重拉,唤起只 sync** | `setKind` / `toggleFavoriteOnly` 立即 `refresh()`;`query` 防抖;`launcher://open` → `refreshList("sync")` 只比首条 id,相同不重置 | 筛选变了本地数据不再成立;隐藏期间列表由事件维护,唤起只兜底 emit 失败 / 竞态,不丢滚动与选中 |
| `applyReset` 集中重置 | 替换 `items`、写 `exhausted`、选中首项、收起展开、`resetTick++` | 「回顶 / 选首项」不散落在 `setKind` / `toggleFavoriteOnly` / `query` 三处;页面 `watch(resetTick)` 滚回顶部 |
| `exhausted` 不由本地列表长度推导 | 成功响应写 `page.length < PAGE_SIZE`;reset 失败经 `applyReset([])` 标记到底,本地增删不修改 | 从 `items.length` 推会在本地删一条后误判到底;首页失败后没有可续拉的有效列表 |
| 游标不存状态 | `loadMore()` 从 `items.at(-1)` 现取 `{ copiedAt, id }`;`loading \|\| exhausted \|\| !last` 直接返回;响应按 id 去重后 `push` | 本地删掉末条后自动退到新末条;在途期间被事件置顶的条目不会被追加成重复行 |
| 错误态 | 每个命令 `try / catch` → `console.error("中文前缀:", e)` + `error.value = String(e)`,下次成功清空 | 与 `ipc-guidelines.md` §3 一致;Rust 文案已是完整句子 |
| **首页失败清空,分页 / sync 失败保留** | `refreshList("reset")` 传 `clearOnError: true`;失败先确认 generation 有效,再 `applyReset([])` 清空列表 / 选中 / 展开并保留错误;分页与 `sync` 不启用此选项。空列表且非 loading、有 error 时页面显示「加载失败」及具体原因,不显示正常无结果文案 | 换筛选后不保留不匹配的旧数据,也不拿旧游标续拉;分页失败不丢已经加载的内容。不新增自动重试或失败恢复状态机 |
| 定时器清理 | 一个防抖 `setTimeout` 句柄(搜索词与带搜索词的事件共用)在 `onUnmounted` `clearTimeout` | 组件销毁后不再去 `refresh`;两个来源本来就该合并成一次重拉 |

纯库代码的部分(相对时间、字节数、文件摘要、可展开判定)不在 composable 里,在 `tools/clipboard/lib/format.ts` 并有 `format.test.ts`(§5)。页面级的滚动容器操作(`@scroll` 距底阈值触发 `loadMore()`、`watch(resetTick)` 滚回顶部)留在 `ClipboardPage.vue`:它需要模板 `ref`,composable 不操作 DOM(§4)。

## 3. 生命周期

- 在 composable 内注册的一切副作用都要在同一个函数内用 `onUnmounted` / `onScopeDispose` 清理。
- 只能在 `setup` 同步阶段调用 composable(Vue 规则);不要在 `onMounted` 回调或异步函数里调用。
- 需要在非组件上下文复用时,用 `effectScope` 包裹,不要绕过 Vue 的作用域机制。

## 4. 与其他层的关系

- composable 可以调用 `src/lib/api/**` 和 store;不反向被 `lib/` 依赖。
- 一个 composable 只做一件事:「监听事件」和「拉取列表」分开写,不做大而全的 `useApp()`。
- 与 UI 库无关:不 import 组件、不操作 DOM(需要 DOM 的用 `ref<HTMLElement>` 由调用方传入,如 `useAutoHeight(rootRef)`;或像 `ClipboardPage.vue` 的 `@scroll` 阈值判定 / `scrollTo` 那样留在页面组件里,composable 只暴露 `loadMore()` 与 `resetTick`)。
- **唯一的全局键盘监听放在 `useKeymapListener`**,其他 composable(`useKeymap` / `useRowNavigation` / 工具页自己的)只登记绑定、不挂 `addEventListener`。登记与监听拆开,是为了避免每个登记方各挂一个监听导致同一次 keydown 被多处处理、`isComposing` / Tab 拦截等公共规则散落多处。

## 5. 测试

- composable 的纯逻辑部分抽成 `lib/` 里的纯函数并写 `*.test.ts`(工具私有的放 `tools/<id>/lib/`,现例 `format.ts` / `format.test.ts`)。浏览器预览(`bun run dev`)的假数据要支持同样的交互,才能手工验证这些套路。
- 编排型 composable(请求 / 事件 / 生命周期交织)可以不依赖 DOM 环境直接测:`useClipboardHistory.test.ts` 用 `createRenderer` 造一个只渲染注释节点的最小渲染器挂载真实组件,`vi.mock` 掉 `lib/api/<domain>` 与 `useTauriEvent`,把 list 请求收进一个可控队列(`settle(index, page | Error)` 决定谁先回来),事件订阅按名收集回调后手动 `emit`。这样能锁住过期丢弃、防抖合并、事件置顶这类只靠手工很难复现的时序。首页失败用例需断言列表 / 选中 / 展开清空、错误保留、`loadMore()` 与时间推进不发请求;另测分页 / sync 失败保留数据、过期失败不清掉最新成功结果。

## 6. 禁止

- 目录叫 `hooks/`(与 React 术语混淆;本仓库固定 `composables/`)。
- 返回数组、返回 `reactive` 对象。
- composable 里直接 `invoke`。
- 无清理的 `listen` / `setInterval` / `addEventListener`。
