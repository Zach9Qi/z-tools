# Composable 规范

> Vue 官方术语是 composable(组合式函数),目录固定 `src/composables/`,不用 `hooks/`。现有四个:
>
> | 文件 | 职责 |
> |---|---|
> | `useKeymap.ts` | `useKeymap(bindings)`:把一份快捷键绑定登记进 `stores/keymap`,挂载登记 / 卸载注销 / 变化重新登记;`useKeymapListener()`:挂**唯一**的 `window keydown` 监听并转给 `store.dispatch`,只由 `LauncherPanel` 调一次 |
> | `useRowNavigation.ts` | 磁贴网格的 `selectedIndex` + 方向键 / Enter 登记;下标计算全部交给 `lib/launcher/navigation.ts` 纯函数 |
> | `useAutoHeight.ts` | `ResizeObserver` 观察面板根,高度变化时调 `lib/window.resizeLauncherToContent`;卸载 `disconnect` |
> | `useTauriEvent.ts` | `useTauriEvent(EVENTS.X, handler)`:订阅一个 Rust 事件,payload 类型由 `lib/events.ts` 推导;非 Tauri 不订阅;卸载 unlisten 并处理 `listen` 晚于卸载 resolve 的竞态。**唯一**允许 import `@tauri-apps/api/event` 的文件;`LauncherPanel` 用它在 `launcher://open` 时聚焦搜索框 |

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

## 3. 生命周期

- 在 composable 内注册的一切副作用都要在同一个函数内用 `onUnmounted` / `onScopeDispose` 清理。
- 只能在 `setup` 同步阶段调用 composable(Vue 规则);不要在 `onMounted` 回调或异步函数里调用。
- 需要在非组件上下文复用时,用 `effectScope` 包裹,不要绕过 Vue 的作用域机制。

## 4. 与其他层的关系

- composable 可以调用 `src/lib/api.ts` 和 store;不反向被 `lib/` 依赖。
- 一个 composable 只做一件事:「监听事件」和「拉取列表」分开写,不做大而全的 `useApp()`。
- 与 UI 库无关:不 import 组件、不操作 DOM(需要 DOM 的用 `ref<HTMLElement>` 由调用方传入,如 `useAutoHeight(rootRef)`)。
- **唯一的全局键盘监听放在 `useKeymapListener`**,其他 composable(`useKeymap` / `useRowNavigation` / 工具页自己的)只登记绑定、不挂 `addEventListener`。登记与监听拆开,是为了避免每个登记方各挂一个监听导致同一次 keydown 被多处处理、`isComposing` / Tab 拦截等公共规则散落多处。

## 5. 测试

- composable 的纯逻辑部分抽成 `lib/` 里的纯函数并写 `*.test.ts`;涉及生命周期的部分靠组件层手工验证,不强求单测(当前 `vitest.config.ts` 无 DOM 环境)。

## 6. 禁止

- 目录叫 `hooks/`(与 React 术语混淆;本仓库固定 `composables/`)。
- 返回数组、返回 `reactive` 对象。
- composable 里直接 `invoke`。
- 无清理的 `listen` / `setInterval` / `addEventListener`。
