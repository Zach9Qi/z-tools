# 跨层思考指南

> **目的**:动手实现之前,先把数据在各层之间怎么流动想清楚。

本仓库的「层」是明确的,一次 IPC 调用会穿过下面这条链:

```
src/components/*.vue
  → src/composables/useXxx.ts        (按需创建;组件级响应式逻辑)
    → src/lib/api.ts                  (唯一 invoke 封装层)
      → src/lib/runtime.ts            (isTauriRuntime:浏览器预览时走降级分支)
        ═══ IPC 边界(JSON 序列化)═══
      → src-tauri/src/commands/<domain>.rs   (薄命令:参数校验 + 转发)
        → 领域模块                    (业务逻辑)
          → src-tauri/src/error.rs    (AppError,序列化为中文字符串)
    src-tauri/src/lib.rs              (装配:插件、setup、generate_handler!)
```

事件方向相反:Rust 领域模块 `emit` → IPC 边界 → `src/lib/events.ts` 类型表 → composable `listen` → 组件。

---

## 问题所在

**大多数 bug 出在层与层的边界上,而不是某一层内部。**

本仓库最常见的跨层 bug:

- 前端 `invoke("greet", { name })` 的参数 key 与 Rust 形参对不上,运行时才报「missing required key」;
- Rust 结构体改了字段,`src/types/<domain>.ts` 手写镜像没同步,前端读到 `undefined` 且无报错;
- 新增了 Rust 命令却忘了在 `lib.rs` 的 `generate_handler!` 里注册,前端调用直接失败;
- 浏览器预览(`bun run dev`)的降级分支返回形状与真实命令不一致,两种运行环境表现不同;
- 同一份校验在 TS 与 Rust 两侧各写一份,错误文案不一致。

---

## 实现跨层功能之前

### 第一步:画出数据流

按下面的顺序把数据的每一跳写出来:

```
用户输入 → 组件状态 → api.ts 参数对象 → JSON → Rust 形参 → 校验 → 业务逻辑 → Result<T, AppError> → JSON → Promise 结果 / reject → 组件展示
```

对每个箭头问三件事:

- 数据此时是什么格式?(TS 类型、JSON 形状、Rust 类型)
- 这里可能出什么错?(序列化失败、字段缺失、运行时不是 Tauri)
- 谁负责校验?(答案应当只有一个)

### 第二步:识别边界

| 边界 | 常见问题 |
|------|----------|
| 组件 ↔ composable | 状态放错层;组件直接 `import` `@tauri-apps/api` 绕过封装 |
| composable ↔ `src/lib/api.ts` | 降级分支与真实分支返回类型不一致 |
| `api.ts` ↔ Rust 命令(IPC) | 参数 key 大小写、缺字段、返回类型泛型写错、命令未注册 |
| Rust 命令 ↔ 领域模块 | 命令层混入业务逻辑;领域模块直接依赖 `tauri::AppHandle` 导致无法单测 |
| 领域模块 ↔ `error.rs` | 新增错误场景没有对应 `AppError` 变体,用 `String` 或 `anyhow` 直接冒出去 |
| Rust `emit` ↔ `src/lib/events.ts` | 事件名字面量两侧不一致;payload 类型两侧字段名不一致 |

### 第三步:写清契约

对每个边界明确:

- 精确的输入格式(TS 侧 camelCase key,Rust 侧 snake_case 形参或 `#[serde(rename_all = "camelCase")]` 结构体);
- 精确的输出格式(Rust 返回类型 ↔ `invoke<T>` 的 `T` ↔ `src/types/<domain>.ts`);
- 会发生哪些错误(`AppError` 哪些变体;前端拿到的是**中文字符串**,不是对象)。

以现有的 `greet` 为例,契约是:

```ts
// src/lib/api.ts
invoke<string>("greet", { name });                    // key: name(camelCase)
```

```rust
// src-tauri/src/commands/greet.rs
#[tauri::command]
pub fn greet(name: &str) -> Result<String, AppError>  // 形参: name(snake_case,自动映射)
```

`Err(AppError::InvalidInput("名字不能为空"))` 经 `error.rs` 的 `Serialize` 实现变为字符串 `"参数错误: 名字不能为空"`,
前端 `catch (error)` 后 `String(error)` 即可展示(`HelloWorld.vue` 就是这么做的)。

---

## IPC 契约变更清单

新增 / 改名命令、新增事件时要同步改动的位置,**唯一权威版本**在 [ipc-contract.md](./ipc-contract.md) 第 3、4 节,这里不再复制一份(否则就是下文「两套机制产出同一份结果」的陷阱)。

补充两条测试层面的提醒:

- Rust 单测覆盖「非法输入 → 对应 `AppError` 变体 + 中文文案」与正常路径;新 payload 结构体可用 `serde_json::to_value` 断言字段名为 camelCase。
- `api.ts` 的降级分支与真实分支返回类型一致由 `vue-tsc` 保证;Vitest 只负责断言降级分支返回值的形状。

---

## 常见跨层错误

### 错误一:隐含的格式假设

**反面**:前端传 `{ user_name }`,以为 Rust 那边会自动认;或者 Rust 返回 `chrono::DateTime`,前端假设它是 `Date`。

**正面**:参数 key 一律 camelCase 交给 Tauri 映射;时间用 ISO 8601 字符串或毫秒时间戳过 IPC,在 `api.ts` 或 composable 里显式转换。

### 错误二:校验分散

**反面**:`api.ts` 里判空一次、Rust 命令里再判一次,两处文案各写一句。

**正面**:校验只在 Rust 命令层做一次(前端是不可信输入源);前端只做交互层面的约束(禁用按钮),错误文案由 `AppError` 统一产出。

### 错误三:抽象泄漏

**反面**:组件直接 `import { invoke }`;领域模块直接持有 `tauri::AppHandle` 拼装窗口逻辑;composable 读 `window.__TAURI_INTERNALS__`。

**正面**:每层只认识相邻的一层。组件只调 composable / `api.ts`;`api.ts` 只认识 `runtime.ts` 与 `@tauri-apps/api`;领域模块通过参数接收依赖,保持可单测。

### 错误四:每个消费方各自解析同一份 payload

**反面**:多个组件各自 `event.payload as { key?: string }`,每个组件拥有一份私有的事件契约。下次改字段时改了一处漏一处。

**正面**:`src/lib/events.ts` 是 payload 类型的唯一拥有者,消费方通过带类型的 composable 拿到数据,渲染代码只做格式化,不重新定义契约。

### 错误五:两种运行环境行为分叉

**反面**:`api.ts` 的降级分支返回 `""`,真实分支返回 `Settings` 对象,浏览器预览时组件崩掉。

**正面**:降级分支返回同类型的合理默认值,并在注释里写明「仅供浏览器预览」;能复用 `src/types/<domain>.ts` 里的类型就不要手写字面量形状。

---

## 跨层功能检查清单

实现之前:

- [ ] 画出了完整的数据流,包括浏览器预览那条分支
- [ ] 列出了会跨越的每个边界(见上表)
- [ ] 每个边界的格式已明确(TS 类型 ↔ JSON ↔ Rust 类型)
- [ ] 校验位置已决定(Rust 命令层),没有第二处

实现之后:

- [ ] 用边缘输入测过(空字符串、纯空白、超长、非法字符)
- [ ] 每个边界的错误路径都验证过:Rust 返回 `Err` 时前端拿到的是中文字符串并正常展示
- [ ] 数据往返不丢字段:Rust → JSON → TS 镜像类型逐字段核对过
- [ ] 消费方引用共享类型 / composable,没有本地强转 payload
- [ ] `generate_handler!`、`api.ts`、`events.ts`、`src/types/` 四处已按变更清单同步
- [ ] `bun run build`(vue-tsc)与 `cargo clippy --all-targets -- -D warnings` 均通过

---

## 什么时候写流程文档

以下情况在任务目录(`.trellis/tasks/<task>/`)或对应 spec 里补一份数据流说明:

- 功能穿过 3 层以上(组件 → composable → api → 命令 → 领域模块);
- 涉及事件与命令双向通信,或使用 `Channel` 传输有序数据流;
- 数据格式复杂(嵌套结构体、枚举经 serde 序列化);
- 这条链路以前出过 bug。

---

## AI 交叉评审假阳性

跨层改动的 AI 评审误报率偏高,因为评审者往往只看到一侧代码。处理评审结论时:

- [ ] 评审称「用户输入可能是恶意的」→ 核实数据源:是前端传来的(确实不可信),还是 Rust 侧自己构造的 payload / 打包进应用的配置(可信)?
- [ ] 评审标「缺少校验」→ 校验是否已在 Rust 命令层完成?在 TS 侧再加一份是本指南反对的做法
- [ ] 评审说「行为发生变化」→ 先读代码注释,本仓库注释写明了大量有意为之的设计(例如降级分支、`canSubmit` 双重兜底)
- [ ] 评审在测试里发现「bug」→ 把被测功能在脑中删掉,测试是否仍能通过?能通过才是空洞测试,否则评审读错了

常见误报模式:

1. **信任边界混淆**:把内部数据当成不可信外部输入;
2. **忽略设计注释**:把注释已说明的有意行为标为 bug;
3. **变量误读**:没有追踪到变量的真实定义(例如把 `name` 形参与 `name` 响应式引用混为一谈)。

**核实规则**:每条 CRITICAL / WARNING 级别的结论都要对照真实代码确认后再排优先级;按大约三分之一的误报率预留精力。
