# 状态管理

> Pinia 已安装(`dependencies`,`main.ts` 中 `use(createPinia())`)。现有 store 只有 `src/stores/keymap.ts`(快捷键登记表);启动器的视图状态仍在组件内(`LauncherPanel.vue`)。本文件规定「状态放哪」与「store 怎么写」。

---

## 1. 状态放哪

| 状态种类 | 放哪 | 例子 |
|---|---|---|
| 只有一个组件用的 UI 状态 | 组件内 `ref` / `computed` | `LauncherPanel.vue` 的 `homeQuery` / `toolQuery` / `activeModule` |
| 父子之间传递 | props 向下、emits 向上;两层以内不要引 store | `LauncherPanel` → `ResultsPanel` 的 `sections` / `activate` |
| 多个不相邻组件共享、或需要跨路由保留 | Pinia store(`src/stores/`) | 快捷键登记表(`stores/keymap.ts`)、用户设置 |
| 跨不相邻组件的「登记 / 消费」 | store 持有登记表;登记方通过 composable 在 `onMounted` / `onUnmounted` 增删**自己那一份**(按句柄),消费方用 `storeToRefs` 读派生值 | 工具页 / 导航 composable 用 `useKeymap` 登记,页脚读 `hints` |
| 来自 Rust 的「真相」(设置、列表) | Rust 是唯一数据源;前端 store 只做镜像 + 缓存,变更走 `api.ts` 命令,再由事件或重新拉取刷新 | Rust emit `settings://updated` → 前端 store 同步 |
| 派生值 | 永远 `computed`,不存副本 | `LauncherPanel.vue` 的 `view`(由 `activeModule` 派生)、`stores/keymap.ts` 的 `hints`(由 `bindings` 派生) |

## 2. 引入 Pinia 时的写法

一律用 **setup store**,导出名统一 `useXxxStore`。下面是「镜像 Rust 数据」类 store 的示意;真实样板见 `src/stores/keymap.ts`(纯前端状态,不走 IPC):

```ts
// src/stores/settings.ts
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { getSettings, updateSettings } from "@/lib/api";
import type { Settings } from "@/types/settings";

export const useSettingsStore = defineStore("settings", () => {
  /** 来自 Rust 的设置镜像;null 表示尚未加载 */
  const settings = ref<Settings | null>(null);
  const isLoaded = computed(() => settings.value !== null);

  /** 从 Rust 拉取最新设置 */
  async function load() {
    settings.value = await getSettings();
  }

  return { settings, isLoaded, load };
});
```

- 文件名 `src/stores/<domain>.ts`,store id 与文件名一致。
- 业务动作写成 store 的函数(action),组件只调 action、读 state,不在组件里拼多个 store 的读写。
- 组件里解构 store 用 `storeToRefs()`,否则丢响应性。
- 一个 store 内需要别的 store 时,在 setup 函数体内调用另一个 `useXxxStore()`,不要在模块顶层调用。
- 「登记表」类 store(如 `keymap`)的模式:`register()` 返回句柄、`unregister(handle)` 整份删除,派生值(`hints`)用 `computed`;不让登记方直接改 `Map`。生命周期绑定不写在 store 里(store 没有组件上下文),而在 composable(`useKeymap`)里用 `onMounted` / `onUnmounted` 完成。
- IPC 调用仍然走 `@/lib/api`,store 不 `import { invoke }`。

## 3. 持久化

业界无统一做法(pinia-plugin-persistedstate / VueUse `useLocalStorage` / IndexedDB 均有人用),本项目暂不规定。原则只有一条:需要跨启动保留、且与 Rust 相关的数据,持久化放 Rust 侧(文件 / tauri-plugin-store),前端不要另存一份成为第二真相。纯 UI 偏好(侧栏折叠)才允许前端本地持久化。

## 4. 禁止

- 用 `reactive` 做全局单例模块代替 store(无 devtools、无生命周期)。
- 用 `provide/inject` 传递业务状态代替 store(Vue 官方定位 `provide/inject` 为组件库内部依赖注入;跨多层共享业务状态用 Pinia)。
- 把 Rust 已经持有的数据在前端改了不回写、或在前端与 Rust 各改一份。
- store 里直接 `invoke`、直接读 `localStorage`(要持久化就用统一的方案并写进本文件)。
