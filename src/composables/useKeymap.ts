// 快捷键 composable:useKeymap 只负责「把绑定登记进 store 并随组件生命周期注销」;
// useKeymapListener 负责「挂唯一的 window keydown 监听」,由面板调用一次。
// 拆开是为了避免每个登记方各挂一个监听。
import { onMounted, onUnmounted, toValue, watch, type MaybeRefOrGetter } from "vue";
import { useKeymapStore, type KeyBinding } from "@/stores/keymap";

/**
 * 登记一份快捷键绑定,组件挂载时登记、卸载时注销。
 * 传 getter / ref 时绑定变化(如 label 随视图切换)会先注销旧句柄再重新登记,页脚提示随之更新。
 */
export function useKeymap(bindings: MaybeRefOrGetter<KeyBinding[]>): void {
  const store = useKeymapStore();
  /** 当前那份绑定在 store 里的句柄;null 表示尚未登记或已注销 */
  let handle: symbol | null = null;

  function apply(): void {
    if (handle !== null) store.unregister(handle);
    handle = store.register(toValue(bindings));
  }

  onMounted(apply);
  // 只在挂载后才响应变化;挂载前 store 里没有句柄,重复登记没有意义
  watch(
    () => toValue(bindings),
    () => {
      if (handle !== null) apply();
    },
  );
  onUnmounted(() => {
    if (handle !== null) store.unregister(handle);
    handle = null;
  });
}

/**
 * 在 window 上挂唯一的 keydown 监听,把事件交给 store.dispatch。
 * - 输入法合成期间(isComposing)一律放过,否则会吞掉候选词确认的 Enter。
 * - Tab / Shift+Tab 无条件拦截:焦点常驻搜索框,不允许跳到磁贴上。
 * - 带 alt / meta / shift 的组合键放过,交给系统 / 浏览器(Alt+Enter 等由 Rust 侧全局快捷键处理)。
 * 组件卸载时移除监听。
 */
export function useKeymapListener(): void {
  const store = useKeymapStore();

  function onKeydown(e: KeyboardEvent): void {
    if (e.isComposing) return;
    if (e.key === "Tab") {
      e.preventDefault();
      return;
    }
    if (e.altKey || e.metaKey || e.shiftKey) return;
    store.dispatch(e);
  }

  window.addEventListener("keydown", onKeydown);
  onUnmounted(() => window.removeEventListener("keydown", onKeydown));
}
