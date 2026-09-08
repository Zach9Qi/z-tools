// 测试 isTauriRuntime():根据 globalThis 上是否注入 __TAURI_INTERNALS__ 判断当前是否运行在 Tauri WebView 中
import { afterEach, describe, expect, it, vi } from "vitest";
import { isTauriRuntime } from "./runtime";

describe("isTauriRuntime", () => {
  // 每个用例结束都还原全局对象,避免注入的 __TAURI_INTERNALS__ 泄漏到其他用例
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("globalThis 上没有 __TAURI_INTERNALS__ 时返回 false(浏览器预览 / node 环境)", () => {
    expect("__TAURI_INTERNALS__" in globalThis).toBe(false);
    expect(isTauriRuntime()).toBe(false);
  });

  it("注入 __TAURI_INTERNALS__ 后返回 true(模拟 Tauri WebView)", () => {
    vi.stubGlobal("__TAURI_INTERNALS__", {});
    expect(isTauriRuntime()).toBe(true);
  });
});
