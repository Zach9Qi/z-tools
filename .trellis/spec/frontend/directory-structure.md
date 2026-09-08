# 前端目录结构

> `src/` 下代码如何组织。写新文件前先看这里,别在 `src/` 根目录随手新建。

---

## 当前布局(真实存在)

```text
src/
├── components/
│   ├── common/            # 通用无业务组件(KeyboardKey.vue)
│   └── launcher/          # 启动器领域组件,8 个:LauncherPanel(壳 / 状态拥有者)、HomeSearchBar、
│                          # ToolSearchBar、SearchInput、ResultsPanel、ToolSection、ToolTile、LauncherFooter
├── composables/           # useKeymap(含 useKeymapListener)、useRowNavigation、useAutoHeight
├── stores/
│   └── keymap.ts          # 唯一的 Pinia store:快捷键登记表 + 页脚提示派生
├── lib/                   # 与 Vue 无关的纯 TS 模块
│   ├── api.ts             # 唯一允许 import `@tauri-apps/api/core` 的 invoke 封装层
│   ├── runtime.ts         # isTauriRuntime():是否运行在 Tauri WebView 内
│   ├── window.ts          # 唯一允许 import `@tauri-apps/api/window` 的窗口控制封装(hide / setSize)
│   └── launcher/          # 启动器纯函数:search / navigation / keyLabels,各配同目录 *.test.ts
├── types/
│   └── tool.ts            # 工具注册契约(ToolItem / ToolModule / isViewModule)
├── tools/                 # 工具模块(见 tool-module-guidelines.md)
│   ├── registry.ts        # 手工维护的模块数组;catalog / moduleOf 由此派生
│   ├── icons.ts           # lucide 图标名 → 组件的手工映射表
│   └── demo/              # 示例工具,真实工具进来后整目录删除
├── App.vue                # 根组件:只管布局,挂 LauncherPanel
├── index.css              # Tailwind 4 入口 + 三层设计令牌(见 styling-guidelines.md)
├── main.ts                # createApp(App).use(createPinia()).mount("#app"),不放业务逻辑
└── vite-env.d.ts          # Vite / unplugin-icons 类型声明
```

`@/` 别名指向 `src/`(`vite.config.ts`、`vitest.config.ts`、`tsconfig.app.json` 三处一致),跨目录 import 用 `@/...`,不用 `../../`。

## 增量扩展规则

以下目录**按需创建**(部分已存在);命名与职责一旦确定就不要再造同义目录(避免同时出现 `hooks/` 与 `composables/`、`utils/` 与 `helpers/`):

| 需要放的东西 | 目录 | 命名 | 依据 |
|---|---|---|---|
| 可复用组件 | `src/components/` | `PascalCase.vue`;同一领域组件变多(经验值 5~6 个)时按领域建子目录(`components/settings/`),通用无业务组件放 `components/common/` | 本仓库现有做法 |
| 组合式函数 | `src/composables/` | `useXxx.ts`,导出同名 `useXxx` | Vue 官方术语 |
| Pinia store | `src/stores/` | `xxx.ts` 导出 `useXxxStore` | Pinia 官方惯例 |
| IPC 封装 | `src/lib/api.ts`;命令多了拆成 `src/lib/api/<domain>.ts` 并由 `api/index.ts` 汇出 | 函数名 camelCase,与 Rust 命令 snake_case 一一对应 | 本仓库 `lib/api.ts`;README「禁止组件直接调用 invoke」 |
| 事件名常量 + payload 类型表 | `src/lib/events.ts` | 常量 `as const`,与 Rust 侧常量一一对应(命令名**不**建常量表,字面量写在封装函数内即可) | 事件名两侧必须一一对应,集中管理便于 grep;命令名字面量与 Tauri 官方用法一致 |
| 跨模块共享类型 | `src/types/` | 一个领域一个文件;仅当 ≥2 个模块使用才提升到这里 | 集中目录 + 就近声明混合,避免过早集中 |
| 纯工具函数 | `src/lib/`;同一领域的多个纯函数按领域建子目录(`lib/launcher/`),与拆分后的 `lib/api/` 同级 | 动词短语命名,单一职责,配同目录 `*.test.ts` | `vitest.config.ts` 的 `include: src/**/*.test.ts`;本仓库 `lib/launcher/` |
| 工具模块 | `src/tools/<id>/` | `index.ts` 导出该工具的 `ToolModule`;可含自己的 `components/` / `composables/` / `lib/`;在 `tools/registry.ts` 手工登记 | 工具 = 组件 + 逻辑的领域包,`components/` 放不下;详见 `tool-module-guidelines.md` |
| 窗口 API 封装 | `src/lib/window.ts` | 动词短语,如 `hideLauncher()`;非 Tauri 环境 no-op | 与 `lib/api.ts` 同一套「唯一入口 + 浏览器降级」规则(见 `ipc-guidelines.md` §1) |
| 静态资源 | `public/`(不经打包)、`src/assets/`(经打包) | — | Vite 约定 |

尚未决定、暂不规定的:页面目录名(`views/` 与 `pages/` 业界无统一做法),引入路由时再定并回写本文件。

## 依赖方向

```
tools/*  →  components / composables / types      (工具模块可复用启动器的通用组件与 composable)
   ↑
components/launcher  →  tools/registry            (启动器只经 registry 认识工具,不 import 具体工具)

components  →  composables  →  lib/api  →  @tauri-apps/api/core
     ↓              ↓   ↘            ↓
   stores        stores   ↘       lib/runtime
                           ↘
                  @tauri-apps/api/event(仅事件 composable)

lib/window.ts  →  @tauri-apps/api/window          (唯一入口;组件 / composable 只 import lib/window)
```

- 箭头只能从左到右 / 上到下;`lib/` 不得 import 组件或 store。`lib/launcher/` 与 `lib/api/` 同级,都是按领域拆分的子目录,同样不得反向依赖。
- `tools/*` 与 `components/launcher/*` 之间只有一条边:`registry.ts`。工具不 import 启动器内部组件(`LauncherPanel` 等),启动器不 import `tools/<id>/`。
- `.oxlintrc.json` 开了 `import/no-cycle`,循环依赖直接报错。
- 组件之间不互相 import 业务逻辑,共享逻辑下沉到 composable。

## 文件命名

| 类型 | 规则 | 示例 |
|---|---|---|
| Vue 组件 | PascalCase | `ToolTile.vue` |
| composable | `use` + PascalCase | `useTauriEvent.ts` |
| 其余 TS 模块 | camelCase 或 kebab-case,同目录内保持一致;`lib/` 现为 camelCase | `runtime.ts`、`api.ts` |
| 测试 | 被测文件同目录,`<name>.test.ts` | `runtime.test.ts`、`lib/launcher/search.test.ts` |

## 禁止

- 在 `.vue` 里 `import { invoke }`(唯一例外:`src/lib/api.ts` 或 `src/lib/api/**/*.ts`);在 `src/lib/window.ts` 之外 import `@tauri-apps/api/window`。
- 在 `src/` 根目录堆放非入口文件。
- 同一概念多个目录(`hooks/` + `composables/`、`utils/` + `helpers/`)。
