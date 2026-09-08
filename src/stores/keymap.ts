// 快捷键登记表:各组件通过 useKeymap 登记自己的绑定,页脚从 hints 读提示,面板的单点监听调 dispatch 分发。
// 登记方(工具页 / 导航 composable)与消费方(页脚)不相邻,所以进 Pinia store 而不是模块级单例。
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { formatKeyLabel } from "@/lib/launcher/keyLabels";

/** 一条快捷键绑定 */
export interface KeyBinding {
  /** KeyboardEvent.key 列表,任一命中即触发 */
  keys: string[];
  /** 是否要求 Ctrl;不写视为 false,即事件带 ctrlKey 时不命中 */
  ctrl?: boolean;
  /** 页脚提示文案 */
  label: string;
  /** 是否出现在页脚;默认 true。Esc 由页脚固定渲染,登记时置 false 避免重复 */
  hint?: boolean;
  onPress: (e: KeyboardEvent) => void;
}

/** 页脚展示用的一条提示;keys 已经过 formatKeyLabel,Ctrl 已前置 */
export interface KeyHint {
  keys: string[];
  label: string;
}

export const useKeymapStore = defineStore("keymap", () => {
  /** 已登记的绑定;键是 register 返回的句柄,一次 useKeymap 调用对应一份,便于整份注销 */
  const bindings = ref(new Map<symbol, KeyBinding[]>());

  /** 页脚提示:按登记顺序展开、过滤 hint === false;Map 保持插入顺序,所以先登记的先显示 */
  const hints = computed<KeyHint[]>(() => {
    const result: KeyHint[] = [];
    for (const list of bindings.value.values()) {
      for (const binding of list) {
        if (binding.hint === false) continue;
        const keys = binding.keys.map(formatKeyLabel);
        result.push({ keys: binding.ctrl ? ["Ctrl", ...keys] : keys, label: binding.label });
      }
    }
    return result;
  });

  /** 登记一份绑定,返回注销用的句柄 */
  function register(list: KeyBinding[]): symbol {
    const handle = Symbol("keymap");
    bindings.value.set(handle, list);
    return handle;
  }

  /** 注销 register 返回的那份绑定;句柄不存在时静默忽略(重复注销是安全的) */
  function unregister(handle: symbol): void {
    bindings.value.delete(handle);
  }

  /**
   * 把 keydown 事件分发给第一条命中的绑定。
   * 命中条件:key 在列表内且 Ctrl 状态与声明一致(不带 ctrl 的绑定不响应 Ctrl 组合键)。
   * 命中后 preventDefault 并调用 onPress,返回 true;无命中返回 false 让事件走默认行为(如输入字符)。
   */
  function dispatch(e: KeyboardEvent): boolean {
    for (const list of bindings.value.values()) {
      for (const binding of list) {
        if (!binding.keys.includes(e.key)) continue;
        if (Boolean(binding.ctrl) !== e.ctrlKey) continue;
        e.preventDefault();
        binding.onPress(e);
        return true;
      }
    }
    return false;
  }

  return { bindings, hints, register, unregister, dispatch };
});
