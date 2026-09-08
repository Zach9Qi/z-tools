我已收集足够证据,以下是调研报告。

---

# Tauri 2 前后端 IPC 契约 & 工程化 事实标准调研

调研样本(5 个):**官方 plugins-workspace**(fs/store/dialog guest-js,权重最高)、**官方 create-tauri-app 模板**、**hoppscotch-desktop**(Vue3)、**clash-verge-rev**(React)、**EcoPaste**(React)。

## 1. invoke 封装层

| 结论 | 证据 |
|---|---|
| **单独目录集中封装,组件不裸调 invoke**(3/3 应用项目;EcoPaste 明文禁止,clash-verge 有个别漏网) | EcoPaste `src/commands/index.ts` 头注释:"调用方一律 `import { foo } from "@/commands"`,禁止裸调 invoke";clash-verge `src/services/cmds.ts`(但 `components/setting/setting-clash.tsx:3`、`pages/unlock.tsx:22` 仍直接 import invoke);hoppscotch `src/kernel/store.ts`、`src/services/updater.client.ts`(`views/PortableHome.vue:245` 也有裸调) |
| 目录名**分歧**:`src/services/`(clash-verge)、`src/commands/`(EcoPaste)、`src/kernel/`+`src/services/`(hoppscotch) | 同上 |
| **一个大文件**(clash-verge `cmds.ts` ~500 行、EcoPaste `commands/index.ts` ~1400 行)vs 按领域拆(hoppscotch `updater.client.ts` 类封装)——多数为单文件 | 同上 |
| **函数名 = Rust 命令名 camelCase 化**(3/3):`get_profiles`→`getProfiles` | EcoPaste 注释"命名与 Rust 函数同名转 camelCase";clash-verge `cmds.ts:14` |
| **`invoke<T>("cmd", { args })` 显式泛型标返回类型**(官方+3 项目全部如此;无返回值写 `invoke<void>` 或省略) | store guest-js:205 `invoke<number>('plugin:store|load', …)`;clash-verge `cmds.ts:11` `invoke<void>('copy_clash_env')` |
| **参数 key 用 camelCase**,Rust 侧 command 参数自动 camelCase 映射,struct 加 `#[serde(rename_all = "camelCase")]`(官方+3 项目一致) | fs `commands.rs:262/308/360` 全部 `rename_all = "camelCase"`;hoppscotch `config.rs:152-155` 明文注释;clash-verge `cmds.ts:60` `{ activeId, overId }`;例外:clash-verge 透传 mihomo 配置用 kebab-case(`global.d.ts` IConfigData) |
| 官方 guest-js 把 options 打包成 `{ options }` 单对象传给 Rust | dialog `index.ts:363` `invoke('plugin:dialog|open', { options })`;fs `:626` `{ fromPath, toPath, options }` |

## 2. 命令名常量

- **字符串字面量直接写在封装函数内**:官方 fs/store/dialog、clash-verge、hoppscotch(4 处)。
- **常量对象 `TAURI_COMMAND`**:仅 EcoPaste `src/constants/commands.ts`,且注释限定"仅在 commands/index.ts 内使用"。
- **tauri-specta 生成绑定**:未见任何调研项目使用(官方 haptics/geolocation 仅 optional feature 提供 specta 类型)。**结论:字面量为多数,常量表为 1 个项目。**

## 3. 事件契约

| 结论 | 证据 |
|---|---|
| **事件名 `domain://action`**(官方 store + clash-verge + EcoPaste,3 处) | store guest-js:303 `'store://change'`;clash-verge `events.ts` `'verge://refresh-profiles'`;EcoPaste `constants/events.ts` `"clipboard://updated"`;hoppscotch 用 kebab `"updater-event"` |
| **payload 类型映射表 / 常量表集中定义** | clash-verge `events.ts:6` `interface VergeEvents { 'verge://xxx': PayloadType }`(强类型 subscribe);EcoPaste `TAURI_EVENT as const` |
| **官方在 guest-js 内包装 listen 并过滤 payload,返回 `UnlistenFn`** | store `onKeyChange` :299-308;fs watch 用 `Channel` + `Resource.close()` :1290-1305 |
| 清理:React 用 hook 封装(`useMount/useUnmount` + ref);Vue 用 `onUnmounted(() => unlisten())`;clash-verge 处理"unlisten 在卸载后才 resolve"的竞态 | EcoPaste `hooks/useTauriListen.ts`;hoppscotch `PortableHome.vue:291`;clash-verge `events.ts:43-50` |
| 事件 payload 为 tagged union 时用 `#[serde(tag = "type")]` ↔ TS `{ type: "X"; … }` | hoppscotch `updater.rs:27` ↔ `updater.client.ts:17` |

## 4. 共享类型

- **手写镜像 TS interface**(3/3 应用项目,官方 guest-js 亦手写);无 ts-rs/specta/JSON-schema 生成。
- 位置分歧:EcoPaste `src/types/{clipboard,settings}.ts`(镜像注释指向 Rust 路径);clash-verge 全局 `src/types/global.d.ts` 无 import 的 ambient 类型(`I` 前缀:`IProfileItem`)+ 部分类型就地 export 在 `cmds.ts`;hoppscotch 依赖 `@hoppscotch/kernel` 包类型。
- 命名:EcoPaste/hoppscotch 与 Rust 同名(`Settings`↔`Settings`);clash-verge `I` 前缀(旧风格)。
- EcoPaste `types/settings.ts:1-15` 注释是最佳实践模板:写明"镜像 `src-tauri/.../model.rs::Settings`;rename_all 规则;Rust 改字段本文件必须同步"。

## 5. 错误契约

| 项目 | Rust 侧 | 前端处理 |
|---|---|---|
| 官方 fs/store | `impl Serialize for Error` → `serialize_str(self.to_string())` **纯字符串** (store `error.rs:37-43`, fs `commands.rs:65-67`) | reject 值为 string,由调用方 catch |
| EcoPaste | `AppError` 自定义 Serialize → `{ kind, message }` (`core/error.rs:22-32`) | `commands/index.ts:295-330` `call<T>()` 统一 catch → `toAppError` → log + toast → **rethrow**;调用方不再写 toast |
| clash-verge | `CmdResult<T> = Result<T, CommandFailure>`,`{ code?, detail, operation? }` (`cmd/mod.rs:4-12`) | `notice-service.ts:305-318` `isCommandFailure` 类型守卫,按 `code` 映射 i18n key,`.catch(err => showNotice.error(err))` |
| hoppscotch | `Result<T, String>` 为主 | `try/catch` + `console.warn`,fp-ts `Either` 包一层 |

**结论**:官方口径是字符串错误;成熟应用(2/3)走**结构化 `{ kind|code, message|detail }` + 前端封装层统一 toast 后 rethrow**。

## 6. 官方 guest-js 可借鉴写法

1. **命名函数 + 单 options 对象**:`async function mkdir(path, options?: MkdirOptions): Promise<void>`;options interface 每字段 JSDoc(fs `index.ts:637-648`)。
2. **文件末尾集中 `export type {…}` / `export {…}`**,而非逐个 export(fs `:1442-1494`)。
3. **每个公开 API 有 `@example` 代码块 + `@since`**(dialog `:5-33`,fs `:605-613`)。
4. **入参校验在 JS 侧抛 `TypeError`**(fs `:616-623` "Must be a file URL")、`Object.freeze(options)`(dialog `:360`)。
5. **有状态资源用 `class X extends Resource { rid }`**,`invoke` 传 `{ rid: this.rid }`(store `:187`)。

## 7. 工程化

| 仓库 | Lint | Format | Hooks | Commit 规范 | Release | CI |
|---|---|---|---|---|---|---|
| plugins-workspace(官方) | ESLint 10 flat + typescript-eslint `recommendedTypeChecked` + security (`eslint.config.js`) | Prettier `singleQuote, semi:false, trailingComma:none` (`.prettierrc`) | 无 | 无 commitlint | covector `.changes/*.md` | `fmt.yml`(rustfmt+prettier+taplo)、`lint-rust.yml` `cargo clippy … -D warnings`、`lint-javascript.yml`、`check-license-header.yml` |
| clash-verge-rev | ESLint 10 flat + import-x/order + unused-imports (`eslint.config.ts`) | **Biome 只做 format**:lineWidth 80、single quote、semi asNeeded (`biome.json`) | husky → `cargo make pre-commit/pre-push` (`Makefile.toml`):pre-commit 仅 fmt+lint-staged,pre-push clippy+typecheck+knip | 无 commitlint;要求 signed commit (`CONTRIBUTING.md:122`) | 自定义 `scripts/release-version.mjs` | `frontend-check.yml`(format:check/lint/typecheck/test/knip)、`lint-clippy.yml` 三平台 `cargo clippy-all`(alias in `.cargo/config.toml` = `-D warnings`)、`rustfmt.yml`、`cargo-audit.yml` |
| EcoPaste | **Biome check**(lint+format 合一,`noConsole: error`,organizeImports/sortedKeys on) | Biome 默认(2 空格) | simple-git-hooks:pre-commit lint-staged(前端 biome,**Rust fmt+clippy -D warnings**),commit-msg commitlint (`simple-git-hooks.json`, `lint-staged.config.ts`) | `@commitlint/config-conventional`;单行英文 type(`feat/fix/refactor/docs`) | release-it + conventional-changelog,bumper 同步 `Cargo.toml/Cargo.lock` 版本 (`.release-it.ts`) | `pr-check.yml`:jobs `changes`→`frontend`(biome, tsc)/`backend`(fmt --check, clippy -D warnings, test,mac+win 矩阵) |
| hoppscotch | ESLint 9 + `@vue/eslint-config-typescript` + eslint-plugin-prettier;`no-restricted-globals` 禁 localStorage (`eslint.config.mjs`) | Prettier `semi:false, printWidth 80, doubleQuote` (`.prettierrc.js`) | husky,`pre-commit` = `pnpm -r do-lint && do-typecheck` | `@commitlint/config-conventional` (`commitlint.config.js`) | — | 本地克隆无 workflows(monorepo 浅克隆) |
| create-tauri-app | 无 | 无 | 无 | — | — | — |

**Rust 侧**:`cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` 进 CI ——**官方 + clash-verge + EcoPaste 3/3 一致**;clash-verge/EcoPaste 还有 `rust-toolchain.toml`、clash-verge 有 `rustfmt.toml`(max_width 120)与 `.clippy.toml`。

## 8. tsconfig / vite 关键项

- **官方模板基线**(`template-vue-ts/tsconfig.json`):`strict`、`noUnusedLocals`、`noUnusedParameters`、`noFallthroughCasesInSwitch`、`moduleResolution: bundler`、`isolatedModules`、`noEmit`、`references: tsconfig.node.json`。hoppscotch/EcoPaste **完全沿用**;clash-verge 少 noUnused*(交给 eslint)。
- **路径别名 `@/*`**:EcoPaste、clash-verge(+`@root`);hoppscotch 用 `~/*` + 多别名。官方模板无别名。
- **vite 三件套**(`clearScreen:false`、`server.port 1420 + strictPort`、`watch.ignored ["**/src-tauri/**"]`)+ `TAURI_DEV_HOST` hmr 配置:官方模板 `vite.config.ts.lte`、hoppscotch、EcoPaste **3/3 逐字沿用**;clash-verge 走自定义 `scripts/dev.mjs`,vite 只配 port 3000。
- `envPrefix TAURI_`:**所有样本均未配置**(Tauri 2 已不需要)。

## 9. package.json scripts

- 官方基线:`dev: vite`、`build: vue-tsc --noEmit && vite build`、`preview`、`tauri: tauri`。
- 通用附加:`lint`、`format`(clash-verge 另有 `format:check`)、`typecheck`/`tsc`/`lint:ts`(3 种叫法,分歧)、`test: vitest run`。
- EcoPaste 用 `run-s` 组合:`build = tsc → build:icon → build:vite`;hoppscotch monorepo 用 `do-lint/do-typecheck` 前缀供 `pnpm -r` 递归。
- **未见 `check` = lint+typecheck 聚合脚本**(hoppscotch 的 `pre-commit` 脚本近似)。
- `preinstall: npx only-allow pnpm` + `packageManager` 字段:EcoPaste、hoppscotch。

## 10. 反模式/踩坑(来自注释)

- **Rust 改字段前端静默失真**:EcoPaste `types/settings.ts` 注释 "否则前端读到的是空字段,组件渲染会失真而无报错"。
- **listen 竞态**:clash-verge `events.ts:43` "Resolved after teardown: unsubscribe immediately rather than leak";`onSubscribed` 回调解决首屏事件丢失。
- **错误文案重复拼接**:EcoPaste AGENTS.md "message 不加 'xxx failed: {err}' 前缀,动作上下文由前端 toast label 拼接"。
- **直接用 localStorage/`console.*`**:hoppscotch eslint `no-restricted-globals`;EcoPaste biome `noConsole: error` 统一走 `@/utils/log`。
- **schema 版本校验**:clash-verge `cmds.ts:134` `if (view.schemaVersion !== 1) throw` —— 大 payload 加版本号。
- **`__TAURI_INTERNALS__` 直调**(hoppscotch `useAppInitialization.ts:19-21`)属于 hack,不推荐。
- hoppscotch `updater.client.ts:16` `// TODO: Type safety just like persistence.service.ts` —— `event.payload as UpdateEvent` 强转是自认的债务。

## 汇总表

| 档位 | 约定 |
|---|---|
| **公认(官方明确 / ≥4)** | `invoke<T>()` 显式返回泛型;参数 key camelCase + Rust `#[serde(rename_all="camelCase")]`;命令名字符串字面量写在封装函数内(不用枚举/specta);TS 类型手写镜像;vite `clearScreen:false / strictPort 1420 / watch.ignored src-tauri`;tsconfig `strict + moduleResolution bundler + noUnusedLocals`;Rust `cargo fmt --check` + `clippy -D warnings` 进 CI;`pnpm` + `packageManager` 锁定 |
| **多数(2-3 项目)** | 前端有独立 invoke 封装层且禁止组件裸调;函数名 = 命令名 camelCase;事件名 `domain://action`;事件 payload 类型表/常量表集中;结构化错误 `{kind/code, message/detail}` + 封装层统一 toast 后 rethrow;`@commitlint/config-conventional`(EcoPaste、hoppscotch);`@/*` 别名;pre-commit 跑 lint-staged,重检查(clippy/typecheck)放 pre-push 或 CI;CI 按 paths-filter 拆前端/后端 job |
| **分歧/不确定** | 封装目录名(`services/` vs `commands/` vs `kernel/`);单大文件 vs 按领域拆;命令名常量表(仅 EcoPaste);Lint 工具(ESLint vs Biome 全包 vs Biome 仅 format);Prettier 选项(单/双引号,trailingComma);typecheck 脚本名(`typecheck`/`tsc`/`lint:ts`);官方错误为纯字符串 vs 应用为结构化对象;commit scope 用法与中文均无证据(全部英文无 scope);release 流程(covector / release-it / 自研脚本)三家三样;Vue 侧 listen 清理无通用 composable(仅 1 个项目手写 `onUnmounted`) |