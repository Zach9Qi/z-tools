# 前端 IPC 规范(invoke / 事件 / 降级)

> 前端与 Rust 之间的所有通信都经过这一层。契约的 Rust 侧见 `../backend/command-guidelines.md`,两侧变更清单见 `../guides/ipc-contract.md`。

---

## 1. invoke 只能出现在 `src/lib/api.ts`(或拆分后的 `src/lib/api/**/*.ts`)

- 组件、composable、store 都不直接 `import { invoke }`;统一 `import { greet } from "@/lib/api"`。(README「禁止组件直接调用 invoke」)
- 每个命令一个导出函数,**函数名 = Rust 命令名的 camelCase**:`get_settings` → `getSettings()`。
- 命令名以字符串字面量写在封装函数里,不建常量表、不用枚举、不引入 tauri-specta(命令名只在封装函数内出现一次,常量表没有收益)。
- 命令多了按领域拆 `src/lib/api/<domain>.ts`,由 `src/lib/api/index.ts` 汇出,调用方 import 路径不变。

```ts
// src/lib/api.ts —— 现有样板
export function greet(name: string): Promise<string> {
  if (!isTauriRuntime()) {
    return Promise.resolve(`（浏览器预览）你好，${name}！`);
  }
  return invoke<string>("greet", { name });
}
```

## 2. 参数与返回值

- `invoke<T>()` **必须**写返回类型泛型;无返回值写 `invoke<void>`。
- 参数对象 key 用 **camelCase**,Tauri 会自动映射到 Rust 的 snake_case 形参;Rust 侧结构体带 `#[serde(rename_all = "camelCase")]`,因此 TS 侧永远只看到 camelCase。(官方文档「Passing Arguments」)
- 多个可选参数打包成一个 `options` 对象传递,而不是一长串位置参数。
- 封装函数**不做业务校验**(判空、长度、格式都在 Rust 命令层,前端不复制一份文案);只在拦截“调用方编程错误”时才抛 `TypeError`,例如必须是 `file://` URL 却传了相对路径。不确定属于哪类时,不在前端校验。
- 每个导出函数写 `/** */` 中文 JSDoc:做什么、非 Tauri 环境如何降级、失败时 reject 的值是什么。

## 3. 错误契约

- Rust 侧 `AppError` 序列化为**中文字符串**(`src-tauri/src/error.rs` 的 `impl Serialize` → `serialize_str(self.to_string())`)。因此 `invoke` reject 的值就是可展示文案,调用方 `catch (error)` 后用 `String(error)` 直接绑到模板(`HelloWorld.vue` 的 `errorMessage`)。
- 组件层处理错误的固定套路:`loading` 置位 → `try / catch / finally` → `console.error("中文前缀:", error)` + 写入 `errorMessage`;不让 Promise 悬空 reject。
- 前端**不要**再给错误文案拼动作前缀(「保存失败:」),Rust 文案已经是完整句子(两侧都拼前缀会重复)。
- 未来若需要按错误类型分支(如区分「参数错误」与「系统错误」做不同 UI),改为结构化 `{ kind, message }` 契约;本仓库尚未需要,**不要**提前引入。

## 4. 浏览器预览降级

- `src/lib/runtime.ts` 的 `isTauriRuntime()` 检测 `__TAURI_INTERNALS__`;这是**唯一**允许触碰该全局变量的地方(在业务代码里直判 `__TAURI_INTERNALS__` 是 hack,应集中判定)。
- 每个 `api.ts` 封装函数都要有非 Tauri 分支:返回带「(浏览器预览)」标识的假数据、或 no-op、或 `Promise.reject(new Error("浏览器预览不支持 xxx"))`,保证 `bun run dev` 能打开页面。事件 composable 同理:非 Tauri 环境下不调 `listen`,直接返回。
- 降级分支要让人一眼看出是假数据,不要与真实返回无法区分。

## 5. 事件(Rust → 前端)

本仓库尚未使用事件;引入时按以下约定:

- 事件名格式 `domain://action`,kebab-case,如 `settings://updated`。
- 事件名常量与 payload 类型集中在 `src/lib/events.ts`:

```ts
export const EVENTS = {
  SETTINGS_UPDATED: "settings://updated",
} as const;

/** 事件名 → payload 类型映射;与 Rust 侧 emit 的结构体一一对应 */
export interface EventPayloads {
  [EVENTS.SETTINGS_UPDATED]: { key: string };
}
```

- `listen` 封装成 composable(如 `useTauriEvent(name, handler)`),内部 `onUnmounted` 调用 unlisten;处理「组件已卸载但 `listen` 的 Promise 才 resolve」的竞态:resolve 后发现已卸载就立刻 unlisten。(Tauri 官方文档要求组件卸载时必须 unlisten)
- 不在组件里手写 `listen` + 手动保存 `unlistenFn`。
- 大量或有序的数据流(下载进度、日志流)用 `Channel`,不用事件;事件系统官方定位是「少量数据、多生产者多消费者」。
- payload 类型写在 `events.ts`,消费方不 `event.payload as X` 强转(这类强转是技术债)。

## 6. 类型镜像

- Rust 返回的结构体在 TS 侧**手写**镜像 `interface`(不引入 ts-rs / specta),放在 `src/types/<domain>.ts`。
- 文件头注释写明镜像的 Rust 路径与 `rename_all` 规则,并提醒「Rust 改字段这里必须同步,否则前端读到 undefined 且无报错」。
- 字段名与 Rust 一致(经 camelCase 转换),类型名与 Rust 结构体同名,不加 `I` 前缀。

## 7. 禁止

- `.vue` / store 里 `import` `@tauri-apps/api/*`;composable 里 `import` `@tauri-apps/api/core`(`@tauri-apps/api/event` 仅限事件 composable)。
- `invoke("cmd")` 不写泛型。
- 参数 key 用 snake_case(Rust 侧不使用 `rename_all = "snake_case"`)。
- 直接读 `window.__TAURI_INTERNALS__`。
- `listen` 后不清理。
