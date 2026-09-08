# Tauri2 + Vue3 前端约定调研报告

## ⚠️ 样本说明
- **BiliTools**:仓库已清空(仅 `README.md` 公告,2026-07 停止维护),**无源码**。
- **PakePlus**:仓库无 `src/`(`.gitignore` 未忽略,是前端闭源、仅发布 `dist/`),**无源码**。
- **有效样本 3 个**:HuLa(Vite+NaiveUI)、JiwuChat(Nuxt4+ElementPlus)、AIaW(Quasar)。下文计数以 **N/3** 为准,并标注 N/5。

---

## 1. 目录分层
| 项目 | src 一级目录 |
|---|---|
| HuLa `src/` | `components/ views/ layout/ stores/ hooks/ services/ utils/ typings/ enums/ common/ directives/ plugins/ router/ styles/ strategy/ workers/ mobile/` |
| JiwuChat `app/` | `components/ pages/ layouts/ composables/{api,hooks,store,tauri,utils} types/ constants/ utils/ init/ middleware/ plugins/ directives/ windows/`(Nuxt 约定) |
| AIaW `src/` | `components/ pages/ views/ layouts/ composables/ stores/ utils/ router/ boot/ css/ i18n/` |

- 页面目录:`views/`(HuLa、AIaW)与 `pages/`(JiwuChat、AIaW 两者都有)——**分歧**。
- 组件再分:HuLa 按 `common/` + 业务域(`rightBox/ fileManager/ windows/`);JiwuChat 按业务域(`chat/ user/ setting/ common/`);AIaW **扁平**(60+ 组件同级)。**2/3 有 `components/common/`**。
- 页面与组件边界:HuLa 以 Tauri 窗口为页面单位(`views/loginWindow/`、`views/moreWindow/`);JiwuChat 以路由为单位。
- 枚举/常量独立目录:HuLa `src/enums/index.ts`(单文件超大枚举集),JiwuChat `app/constants/`。2/3。

## 2. IPC 封装
- **有薄封装层但不彻底**(3/3):
  - HuLa:`src/services/tauriCommand.ts`(`getSettings()/switchUserDatabase()` 等函数导出)+ `src/utils/TauriInvokeHandler.ts`(`invokeWithErrorHandler<T>/invokeSilently/invokeWithRetry`,统一抛 `AppException`)。但仍有 **18 个 .vue 直接 import `invoke`**(`components/common/AreaDrawer.vue:32`、`views/chatHistory/index.vue:100`)。
  - JiwuChat:`app/composables/tauri/window.ts`(`exitApp = () => invoke("exit_app")`),仅 `pages/login.vue:41` 一处组件内直调。
  - AIaW:只有 2 处 invoke,均在 `utils/tauri-stream.ts:82`、`utils/update.ts:79`,组件零直调。
- 命令名:HuLa 有 `enum TauriCommand`(`src/enums/index.ts:534`)但 `tauriCommand.ts` 内仍大量字符串字面量;其余 2 个纯字符串字面量。**枚举管理仅 1 个项目且不彻底**。
- **tauri-specta / ts-rs:0/3 使用**(grep 三仓 `package.json`、`Cargo.toml` 无结果)。
- 返回类型:`invoke<T>` 泛型或函数签名 `Promise<Settings>` 手写(HuLa `tauriCommand.ts:53`;JiwuChat `useDeepLink.ts:57` `invoke<OAuthCallbackPayload|null>`)。

## 3. 事件监听
- HuLa:专用 hook `src/hooks/useTauriListener.ts` —— `addListener(listen(...), id?)`,内部按窗口 label 做全局 Map 管理、`safeUnlisten` 防重复调用、`onUnmounted` + `onCloseRequested` 双重清理。事件名用 `enum EventEnum`(`enums/index.ts:40`),窗口内通信另用 mitt(`hooks/useMitt.ts`,自动 `onUnmounted` off)。
- JiwuChat:直接 `listen`,手动保存 `unlistenFn` 并 `onUnmounted(stopListening)`(`composables/hooks/oauth/useDeepLink.ts:65-95`);全局监听集中在 `app/init/init.ts`。事件名字符串字面量。
- AIaW:仅 `utils/tauri-stream.ts:57` 一处,手动 unlisten。
- **结论**:监听放 composable/初始化模块 + `onUnmounted` 清理(3/3);事件名枚举化仅 1/3;专用封装 hook 仅 1/3。

## 4. 状态管理
- **Pinia setup store**:AIaW 7/7 全 setup;HuLa 33 个 store 中 setup 为主,`setting.ts:34` 等少数 options(`.rules` 明文要求 setup);JiwuChat setup(`useSettingStore.ts:40`)。**3/3 倾向 setup**。
- 文件命名:HuLa/AIaW `stores/xxx.ts`(kebab/camel)导出 `useXxxStore`;JiwuChat `composables/store/useXxxStore.ts`。**导出名 `useXxxStore` 3/3 一致,文件名分歧**。
- 持久化:HuLa `pinia-plugin-persistedstate`(`stores/index.ts` `auto: true`)+ `pinia-shared-state`;JiwuChat 用 VueUse `useLocalStorage` 逐字段(`useSettingStore.ts:44`);AIaW 用 Dexie/IndexedDB + 自写 `composables/persistent-reactive.ts`。**分歧,tauri-plugin-store 0/3 用于 Pinia**。
- store 边界:HuLa `.rules`:"业务逻辑放 action,组件只调 action/state;跨 store 在 setup 内调用另一 store 工厂"。

## 5. composables
- 目录名:`hooks/`(HuLa)vs `composables/`(JiwuChat、AIaW)— 2/3 用 composables。
- 文件名:HuLa `useXxx.ts`(44 个全部);JiwuChat `useXxx.ts`;**AIaW 用 kebab-case 文件名**(`set-title.ts` 导出 `useSetTitle`)。函数名 `use` 前缀 3/3。
- 返回结构:对象(HuLa `useTauriListener` 返回 `{addListener,pushListeners,cleanup}`;JiwuChat `useDeepLink` 返回对象)3/3。

## 6. 组件约定
- `<script setup lang="ts">`:HuLa、JiwuChat 统一;AIaW 大部分 ts 但存在 `<script setup>` 无 ts(`components/CopyBtn.vue` 用运行时 `defineProps({...})`)。**2/3 严格**。
- SFC 块顺序:**分歧** —— HuLa `template→script→style`(`components/common/*.vue` 全部);JiwuChat `script→template→style`(`app/components/chat/**`);AIaW `template→script`。
- props:TS 泛型 `defineProps<Props>()` 3/3。默认值:JiwuChat **强制解构默认值、禁 `withDefaults`**(AGENTS.md);HuLa 两者混用(`withDefaults` 8+ 处,解构 8+ 处);AIaW 少见默认值。
- emits:类型化 `defineEmits<{...}>()` 3/3;调用签名式 `(e:'x', v:T): void` 与元组式 `x: [v:T]` 在 HuLa 内混用。
- 文件名:PascalCase 3/3(JiwuChat 目录+`index.vue` 组合,Nuxt 路径即组件名)。
- `defineOptions({name})`:HuLa 仅 5 处,其他 0。**不通用**。

## 7. 类型组织
- 集中:HuLa `src/typings/*.d.ts`(全局 `declare namespace STO/Common`,`stores.d.ts`)+ `src/services/types.ts`(API 类型);JiwuChat `app/types/`(`result.ts` 的 `Result<T>`/`StatusCode`);AIaW `utils/types.ts` 单文件。**"集中目录 + 就近声明"混合 3/3**。
- `type` vs `interface`:HuLa 偏 `type`(`services/types.ts` 全 `export type`);JiwuChat 偏 `interface`。**分歧**。
- 全局 `.d.ts`:3/3 有(HuLa `typings/global.d.ts`,JiwuChat 根 `shims.d.ts/declarations.d.ts`,AIaW `env.d.ts/quasar.d.ts`)。
- 前后端共享类型:**3/3 手写镜像**,无生成。

## 8. 样式
- **UnoCSS 3/3**(`uno.config.ts` 三仓皆有),Tailwind 0/3。
- 设计令牌:HuLa `src/styles/scss/global/variable.scss` + `html[data-theme="dark"]`,用 `bg-[--var]` 消费;JiwuChat 用 uno shortcuts(`card-bg-color`)+ `--at-apply`。2/3 有令牌层。
- `<style scoped lang="scss">`:HuLa/JiwuChat 几乎每个组件都有(HuLa >100 个);AIaW 少。

## 9. 工程化
| | HuLa | JiwuChat | AIaW |
|---|---|---|---|
| Lint/格式 | **Biome** `biome.json`(+ Prettier 仅 .vue `.prettierrc`) | ESLint `@antfu/eslint-config` `eslint.config.mjs` | ESLint legacy `.eslintrc.cjs`(standard) |
| husky/lint-staged | `.husky/`+`.lintstagedrc.mjs`(含 `cargo fmt`) | `.husky/pre-commit`+`package.json#lint-staged` | 无 |
| commitlint | `commitlint.config.cjs`(config-conventional + cz-git 中文提示,英文 type) | `commitlint.config.ts`(config-conventional,header≤100) | 无 |
| 版本 | release-it `.release-it.js` | `npm version` | 无 |
| CI 检查 | `release.yml` 仅 build;`rust-clippy.yml`;`codeql.yml`;**无前端 lint/test job** | 仅 build | 仅 build |
- Conventional Commits 2/3(subject 中文/英文均允许);changeset 0/3。

## 10. 测试
- vitest:HuLa 有 `vitest.config.ts`(`include: src/**/*.{test,spec}.ts`, happy-dom),**但仓内 0 个测试文件**;JiwuChat AGENTS.md 明言"无 JS 测试";AIaW `"test": "echo No test"`。**3/3 实际无前端测试**。

## 11. 运行时降级
- 3/3 有,方式不一:
  - AIaW:`utils/platform-api.ts:11` `export const IsTauri = '__TAURI_INTERNALS__' in window`,统一分派 `fetch/clipboard/exportFile`。最规范。
  - JiwuChat:store 内 `appPlatform`/`isWeb/isDesktop` computed(`useSettingStore.ts:69-79`),tauri 模块 **动态 `import("@tauri-apps/api/core")`**(`useDeepLink.ts:55`)。
  - HuLa:分散 3 处直接判 `window.__TAURI_INTERNALS__`(`hooks/useNetworkStatus.ts:51`、`Bot.vue:203`)。

## 12. 反模式/踩坑(来源:AGENTS/.rules/代码注释)
- HuLa `useTauriListener.ts:10-21`:重复调用同一 `unlisten` 会致底层 `listeners[eventId]` 不存在 → 需 `WeakSet` 去重。
- HuLa `.rules`:禁止 `_` 前缀未用变量、commit 不用 emoji;storeToRefs 解构;跨窗口用 Tauri event、同窗口用 mitt。
- JiwuChat AGENTS.md:el-tooltip 同时用 `:content` 与 `#content` 导致栈溢出;禁 BEM;禁 `px` 用 `rem`;`--at-apply` 替代 `@apply`。
- HuLa `stores/setting.ts:31` TODO:localStorage 持久化未按账号隔离。

---

## 汇总

| 档 | 结论 |
|---|---|
| **公认(3/3)** | `defineProps<T>()` TS 泛型;类型化 `defineEmits`;组件文件 PascalCase;Pinia setup store + `useXxxStore` 导出;composable 函数 `use` 前缀、返回对象;UnoCSS;有 Tauri/Web 运行时检测;前后端类型手写镜像、无 specta;IPC 有薄封装层(services/utils/tauri 目录)但非组件零直调;`listen` 后 `onUnmounted` 清理;CI 只 build 不 lint/test;实际无前端单测 |
| **多数(2/3)** | `composables/` 目录名;`components/common/`;独立 `enums|constants/`;`<script setup lang="ts">` 严格统一;husky+lint-staged+commitlint(Conventional Commits);设计令牌 CSS 变量层;`<style scoped lang="scss">` 普遍 |
| **分歧/不确定** | `views/` vs `pages/`;SFC 块顺序(template 先 vs script 先);`withDefaults` vs 解构默认值;`type` vs `interface`;Biome vs ESLint;持久化方案(persistedstate / useLocalStorage / IndexedDB);命令/事件名枚举化(仅 HuLa);专用 `useTauriListener` 封装(仅 HuLa);`isTauri` 集中在 platform-api(仅 AIaW);`defineOptions({name})`(极少) |