# 前端 IPC 规范(invoke / 事件 / 降级)

> 前端与 Rust 之间的所有通信都经过这一层。契约的 Rust 侧见 `../backend/command-guidelines.md`,两侧变更清单见 `../guides/ipc-contract.md`。

---

## 1. invoke 只能出现在 `src/lib/api/**/*.ts`

- 封装层已按领域拆文件(真实布局):

  ```text
  src/lib/api/
  ├── index.ts       # 汇出口:export * from "@/lib/api/launcher" / "@/lib/api/clipboard";自身不写任何封装
  ├── launcher.ts    # hideLauncher / getToggleShortcut ↔ commands/launcher.rs
  └── clipboard.ts   # 6 个剪贴板命令 + toAssetUrl() ↔ commands/clipboard.rs;浏览器假数据表也在这里
  ```

  调用方仍统一 `import { … } from "@/lib/api"`(工具内部也可按领域 `from "@/lib/api/clipboard"`);新领域 = 新建 `api/<domain>.ts` + 在 `index.ts` 加一行 `export *`。文件头注释写对应的 Rust 命令文件。
- `@tauri-apps/api/core` 的**所有**导出(`invoke`、`convertFileSrc`)都只能在这些文件里出现。`convertFileSrc` 的唯一调用点是 `src/lib/api/clipboard.ts` 的 `toAssetUrl(path)`:组件拿到的已是可直接放进 `<img src>` 的 URL,非 Tauri 返回一张写着「浏览器预览」的占位 SVG data URL。后端返回的是绝对路径字符串,转 URL 是前端 IPC 层的事;`tauri.conf.json5` 的 `assetProtocol.scope` 必须覆盖该目录(见 `../backend/config-and-permissions.md` §1)。
- 同理,`@tauri-apps/api/window` **只允许在 `src/lib/window.ts`** import,且该文件只做一件事:`resizeLauncherToContent()` 把面板高度 `setSize` 给窗口。**隐藏不走窗口 API**,而是 `api/launcher.ts` 的 `hideLauncher()` 命令封装:后端 `hide` 还要顺带 `set_ignore_cursor_events(true)`(透明窗口隐藏后会残留挡点区)并 emit `launcher://close`,前端直调 `getCurrentWindow().hide()` 会绕过这两步;capabilities 也因此不给 `core:window:allow-hide`。`window.ts` 与 `api/**` 遵守同一套规则:非 Tauri 运行时降级为 no-op(浏览器改不了标签页尺寸),失败时 `console.error("中文前缀:", error)` 且**不抛**——窗口尺寸没同步只是视觉问题,不应让 UI 进入错误态。
- 组件、composable、store 都不直接 `import { invoke }`;统一 `import { hideLauncher, getToggleShortcut } from "@/lib/api"`。(README「禁止组件直接调用 invoke」)
- 每个命令一个导出函数,**函数名 = Rust 命令名的 camelCase**:`get_settings` → `getSettings()`。
- 命令名以字符串字面量写在封装函数里,不建常量表、不用枚举、不引入 tauri-specta(命令名只在封装函数内出现一次,常量表没有收益)。
- 可失败命令的封装(`api/clipboard.ts`)与不可失败的写法一致,只是 JSDoc 要写清 reject 的文案来源(如「参数错误: 记录不存在」、非 Windows 的「当前平台暂不支持: …」),调用方据此决定要不要进错误态。

```ts
// src/lib/api/launcher.ts —— 现有样板(两个命令都不可失败,reject 只可能是 IPC 层异常)

/** 隐藏启动器窗口(主页 Esc)。非 Tauri 运行时没有窗口可隐藏,直接 resolve(no-op) */
export function hideLauncher(): Promise<void> {
  if (!isTauriRuntime()) return Promise.resolve();
  return invoke<void>("hide_launcher");
}

/** 读当前生效的全局唤出快捷键(plugin 语法)。浏览器预览回退默认值——这是前端唯一允许出现该字面量的地方 */
export function getToggleShortcut(): Promise<string> {
  if (!isTauriRuntime()) return Promise.resolve(DEFAULT_SHORTCUT_FALLBACK); // 真实代码是字面量,与后端默认值一致;本文件不复述键位
  return invoke<string>("get_toggle_shortcut");
}
```

唤出键的真相在后端(`launcher::DEFAULT_TOGGLE_SHORTCUT`,将来是用户设置):`LauncherPanel` 挂载时调 `getToggleShortcut()`,经 `lib/launcher/keyLabels.ts` 的 `parseShortcut()` 拆成键帽序列传给搜索栏。组件 / 模板 / 注释里**不写死键位**,否则后端改了前端演错提示。

## 2. 参数与返回值

- `invoke<T>()` **必须**写返回类型泛型;无返回值写 `invoke<void>`。
- 参数对象 key 用 **camelCase**,Tauri 会自动映射到 Rust 的 snake_case 形参;Rust 侧结构体带 `#[serde(rename_all = "camelCase")]`,因此 TS 侧永远只看到 camelCase。(官方文档「Passing Arguments」)
- 多个可选参数打包成一个 `options` 对象传递,而不是一长串位置参数。
- 封装函数**不做业务校验**(判空、长度、格式都在 Rust 命令层,前端不复制一份文案);只在拦截“调用方编程错误”时才抛 `TypeError`,例如必须是 `file://` URL 却传了相对路径。不确定属于哪类时,不在前端校验。
- 每个导出函数写 `/** */` 中文 JSDoc:做什么、非 Tauri 环境如何降级、失败时 reject 的值是什么。

## 3. 错误契约

- Rust 侧 `AppError` 序列化为**中文字符串**(`src-tauri/src/error.rs` 的 `impl Serialize` → `serialize_str(self.to_string())`)。因此 `invoke` reject 的值就是可展示文案,调用方 `catch (error)` 后用 `String(error)` 直接绑到模板。
- 组件层处理错误的固定套路:`try / catch` → `console.error("中文前缀:", error)` + 写入 `error` ref 供模板展示;有加载态时再加 `loading` 置位 / `finally` 复位;不让 Promise 悬空 reject。现存样板见 `src/components/launcher/LauncherPanel.vue` 的 `activate()`。
- 前端**不要**再给错误文案拼动作前缀(「保存失败:」),Rust 文案已经是完整句子(两侧都拼前缀会重复)。
- 未来若需要按错误类型分支(如区分「参数错误」与「系统错误」做不同 UI),改为结构化 `{ kind, message }` 契约;本仓库尚未需要,**不要**提前引入。

## 4. 浏览器预览降级

- `src/lib/runtime.ts` 的 `isTauriRuntime()` 检测 `__TAURI_INTERNALS__`;这是**唯一**允许触碰该全局变量的地方(在业务代码里直判 `__TAURI_INTERNALS__` 是 hack,应集中判定)。
- 每个 `api/**` 封装函数都要有非 Tauri 分支:返回带「(浏览器预览)」标识的假数据、或 no-op、或 `Promise.reject(new Error("浏览器预览不支持 xxx"))`,保证 `bun run dev` 能打开页面。事件 composable 同理:非 Tauri 环境下不调 `listen`,直接返回。
- 降级分支要让人一眼看出是假数据,不要与真实返回无法区分。
- 假数据要支持与后端同样的交互路径,才能在浏览器里走完页面逻辑:`api/clipboard.ts` 的 `fakeItems` 是一张**可变**内存表,`listFakeItems` 实现 kind / favoriteOnly / 关键字 / 游标分页,`mutateFake` 实现删除 / 收藏并对不存在的 id 用与后端同句文案 reject。

> **Warning(本任务实测)**:浏览器预览的假数据必须返回**全新对象**(`.map((item) => structuredClone(item))`),不能把可变内存表里的对象直接交给调用方。
>
> 真实 IPC 经 JSON 反序列化,前端拿到的永远是新对象;假数据若直接返回表里的裸对象,它会同时被 `ref<ClipboardItem[]>` 包成 reactive 代理。之后 `setClipboardItemFavorite` 的假分支先改了裸对象 `item.favorite = favorite`,composable 再对代理赋同一个值 `item.favorite = next`——Vue 比较新旧值相等,判定「未变」不触发重渲染,星标不亮。预览环境和真实环境的对象所有权语义必须一致,否则预览里调好的交互到 Tauri 里表现不同(或反之)。

## 5. 事件(Rust → 前端)

已在用:`EVENTS.LAUNCHER_OPENED`(`launcher://open`,后端 show 后发,前端据此聚焦搜索框并全选旧词)、`EVENTS.LAUNCHER_CLOSED`(`launcher://close`,当前无消费者,保留给隐藏时复位状态的需求)、`EVENTS.CLIPBOARD_CHANGED`(`clipboard://changed`,监听器录入 / 上浮后发;`useClipboardHistory` 防抖 150ms 后 `refresh()`);全部无 payload,`EventPayloads` 对应类型为 `null`(Rust 侧 emit `()`)。约定:

- 事件名格式 `domain://action`,kebab-case,如 `launcher://open`。
- 事件名常量与 payload 类型集中在 `src/lib/events.ts`,文件头注明对应的 Rust 常量位置:

```ts
// src/lib/events.ts —— 现有
export const EVENTS = {
  LAUNCHER_OPENED: "launcher://open",
  LAUNCHER_CLOSED: "launcher://close",
  CLIPBOARD_CHANGED: "clipboard://changed",
} as const;

/** 事件名 → payload 类型;与 src-tauri/src/launcher.rs / clipboard.rs 的常量一一对应;无 payload 写 null,不造空对象类型 */
export interface EventPayloads {
  [EVENTS.LAUNCHER_OPENED]: null;
  [EVENTS.LAUNCHER_CLOSED]: null;
  [EVENTS.CLIPBOARD_CHANGED]: null;
}
```

- 监听统一用 `src/composables/useTauriEvent.ts`(`useTauriEvent(name, handler)`):非 Tauri 不订阅;内部 `onUnmounted` 调用 unlisten;处理「组件已卸载但 `listen` 的 Promise 才 resolve」的竞态(resolve 后发现已卸载就立刻 unlisten);订阅失败 `.catch` 只记日志。(Tauri 官方文档要求组件卸载时必须 unlisten)
- **`useTauriEvent.ts` 是整个 `src/` 唯一允许 import `@tauri-apps/api/event` 的文件**,与 `lib/api/**`(core)、`lib/window.ts`(window)并列为三个 Tauri API 入口。
- 事件到达后怎么做由消费方决定,但原则固定:**事件只告知「变了」,数据用命令重拉**;自己发起的变更不依赖事件回流(后端也不会发),命令成功后直接改本地状态(现例 `useClipboardHistory` 的 `remove` / `toggleFavorite`,见 `composable-guidelines.md` §2.1)。
- 不在组件里手写 `listen` + 手动保存 `unlistenFn`。
- 大量或有序的数据流(下载进度、日志流)用 `Channel`,不用事件;事件系统官方定位是「少量数据、多生产者多消费者」。
- payload 类型写在 `events.ts`,消费方不 `event.payload as X` 强转(这类强转是技术债)。

## 6. 类型镜像

- Rust 返回的结构体在 TS 侧**手写**镜像 `interface`(不引入 ts-rs / specta),放在 `src/types/<domain>.ts`。
- 文件头注释写明镜像的 Rust 路径与 `rename_all` 规则,并提醒「Rust 改字段这里必须同步,否则前端读到 undefined 且无报错」。
- 字段名与 Rust 一致(经 camelCase 转换),类型名与 Rust 结构体同名,不加 `I` 前缀。
- Rust tagged 枚举(`#[serde(tag = "kind")]`)在 TS 侧写成判别联合:公共字段抽非导出的 `XxxItemBase`,每个变体一个 `interface` 带字面量 `kind`,再 `type XxxItem = A | B | C`(现例 `src/types/clipboard.ts` 的 `ClipboardItem`)。模板里用 `v-if="item.kind === 'text'"` 收窄,不写 `as`。Rust 侧忘了 `rename_all_fields` 时前端只会静默拿到 `undefined`,见 `../guides/ipc-contract.md` §1 的 Warning。
- Rust 入参 `Option<T>` 在 TS 镜像里写可选字段 `field?: T`(`ListQuery.kind` / `before`),调用方省略即可,不传 `null`。

## 7. 禁止

- `.vue` / store 里 `import` `@tauri-apps/api/*`;composable 里 `import` `@tauri-apps/api/core` / `@tauri-apps/api/window`;`src/composables/useTauriEvent.ts` 之外 `import` `@tauri-apps/api/event`;`src/lib/window.ts` 之外 `import` `@tauri-apps/api/window`;`src/lib/api/**` 之外调 `convertFileSrc`。
- 浏览器假数据直接返回内存表里的对象(不 `structuredClone`)。
- 前端直调窗口 `hide()` / `show()`(走 `hideLauncher()` 命令;显示由 Rust 侧触发,前端没有入口)。
- 在 `api/launcher.ts` 的 `getToggleShortcut()` 回退值之外写死唤出键字面量。
- `invoke("cmd")` 不写泛型。
- 参数 key 用 snake_case(Rust 侧不使用 `rename_all = "snake_case"`)。
- 直接读 `window.__TAURI_INTERNALS__`。
- `listen` 后不清理。
