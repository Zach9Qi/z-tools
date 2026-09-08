# 类型安全

> 编译器配置见 `tsconfig.app.json`;`bun run build` 里的 `vue-tsc -b` 是全量类型门禁,类型错误直接导致 CI 失败。

---

## 1. 编译器基线(已启用,不得放松)

`tsconfig.app.json`:`strict`、`noUnusedLocals`、`noUnusedParameters`、`noFallthroughCasesInSwitch`、`moduleResolution: "bundler"`、`isolatedModules`、`noEmit`。

- 禁止 `// @ts-ignore`;`// @ts-expect-error` 仅限第三方类型缺陷,且同一行写中文原因。
- 未用变量直接删,不要加 `_` 前缀留着。

## 2. 类型放哪

| 场景 | 位置 |
|---|---|
| 只在一个文件用 | 就近定义在该文件顶部,不导出 |
| 组件 props / emits | `defineProps<{...}>()` 内联;复杂时提到同文件的 `interface Props` |
| ≥2 个模块共用、或镜像 Rust 结构体 | `src/types/<domain>.ts`,按领域一个文件 |
| 全局环境声明(`~icons/*`、`import.meta.env`) | `src/vite-env.d.ts` |
| 事件 payload 映射 | `src/lib/events.ts`(见 `ipc-guidelines.md`) |

采用「集中目录 + 就近声明」混合;不要建 `src/typings/` 之类的第二个类型目录。

## 3. `interface` 还是 `type`

业界无统一做法,本项目不强制。约定只有:

- 同一文件内保持一致。
- 镜像 Rust 结构体的对象形状用 `interface`,便于文件头注释与 Rust 一一对应;联合、别名、映射类型用 `type`。

## 4. IPC 边界的类型

- `invoke<T>()` 泛型必写;`T` 是 `src/types/` 里的镜像类型或原始类型。
- Rust 的 `Option<T>` → TS `T | null`(serde 序列化 `None` 为 `null`,不是 `undefined`);Rust `Vec<T>` → `T[]`;Rust `u64` 等大整数若可能超过 2^53 要在 Rust 侧转字符串,TS 用 `string`。
- 枚举:Rust `#[serde(rename_all = "camelCase")] enum Kind { Text, Image }` → TS `type Kind = "text" | "image"`,不用 TS `enum`。
- 带数据的枚举:Rust `#[serde(tag = "type")]` → TS 判别联合 `{ type: "progress"; percent: number } | { type: "done" }`。
- 事件 payload 与命令返回值都不用 `as X` 强转;类型来自 `events.ts` 的映射或 `invoke<T>` 泛型。

## 5. 运行时校验

- 来自 Rust 的数据是可信内部数据,**不**在前端二次校验(信任边界在 Rust 命令层,见 `../backend/command-guidelines.md`)。
- 用户输入的业务校验(判空、长度、格式)也在 Rust 命令层,前端不重复;`api.ts` 只对“调用方编程错误”抛 `TypeError`(见 `ipc-guidelines.md`)。
- 不引入 zod / valibot 之类运行时校验库,除非有外部 HTTP 数据源(当前没有)。

## 6. 禁止

- `any`(用 `unknown` + 收窄)。
- 非空断言 `!`(用可选链 / 提前 return)。
- `as unknown as X` 双重断言。
- TS `enum`(用字符串字面量联合;与 serde 输出天然对齐)。
- 在 `.vue` 里 `declare global`。
