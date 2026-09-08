# 代码复用思考指南

> **目的**:动手写新代码之前先停一下——这段逻辑是不是已经存在了?

本仓库是 Tauri 2 + Vue 3 + TS(`src/`)加 Rust 后端(`src-tauri/`)的双栈项目。
重复代码在这里格外危险:同一段逻辑经常在 TS 与 Rust 两侧各有一份,任何一侧漏改都不会有编译错误。

---

## 问题所在

**重复代码是「行为不一致」类 bug 的头号来源。**

复制粘贴或重写一段已有逻辑之后:

- bug 修复不会传播到副本;
- 副本各自演化,行为逐渐分叉;
- 读代码的人搞不清哪一份才是权威实现。

本仓库里典型的分叉点:

- `src/lib/api.ts` 的浏览器降级文案与 Rust 命令返回值口径不一致;
- 同一个 Rust 结构体在多个 `.vue` 里各写一份 `interface` 镜像;
- 多个命令各自 `trim()` 并判空同一类用户输入,错误文案却各不相同。

---

## 写新代码之前

### 第一步:先搜索

```bash
# 以下命令在 Git Bash(Windows)或任何 POSIX shell 下执行
# 找同名或相近的函数 / 类型
grep -rn "greet" src/ src-tauri/src/

# 找同一段逻辑的关键词(例如错误文案、事件名、命令名)
grep -rn "名字不能为空" src/ src-tauri/src/
grep -rn "settings://" src/ src-tauri/src/
```

前后端一起搜。IPC 两侧的命令名、事件名、错误文案本质上是同一份契约,只搜一侧会漏掉另一半。

### 第二步:回答这几个问题

| 问题 | 若答案为「是」 |
|------|----------------|
| 已经有相似的函数 / composable / 命令? | 直接用,或在原处扩展 |
| 这个模式别处已经用过? | 沿用现有写法,不另起一套 |
| 这段逻辑将来会有第二个调用方? | 一开始就放到正确的共享位置 |
| 我正在从别的文件复制代码? | **停下**——抽到共享模块再引用 |

「正确的共享位置」在本仓库里是确定的:

| 内容 | 前端 | 后端 |
|------|------|------|
| IPC 调用 | `src/lib/api.ts`(唯一 `invoke` 封装层) | `src-tauri/src/commands/<domain>.rs` |
| 运行时判断 | `src/lib/runtime.ts`(`isTauriRuntime`) | —— |
| 事件名与 payload 类型 | `src/lib/events.ts` | 领域模块内的事件常量 |
| Rust 结构体镜像类型 | `src/types/<domain>.ts` | 领域模块 |
| 可复用的响应式逻辑 | `src/composables/useXxx.ts` | —— |
| 错误类型与文案 | 由后端序列化字符串直接展示 | `src-tauri/src/error.rs`(`AppError`) |

---

## 常见重复模式

### 模式一:复制粘贴函数

**反面**:把 `HelloWorld.vue` 里「调用 → 置 loading → catch 记错误 → finally 复位」那段 `submit` 逻辑原样抄进第二个组件。

**正面**:第二个组件出现时,抽成 `src/composables/useAsyncAction.ts` 之类的 composable,把 `loading` / `errorMessage` / 调用封装进去,两个组件都引用它。

### 模式二:相似组件

**反面**:新建一个与现有组件 80% 相同、只换了标题和按钮文案的 `.vue`。

**正面**:给现有组件加 props / slot / 变体,而不是复制一份。

### 模式三:重复的常量

**反面**:事件名 `"settings://updated"` 字面量散落在多个 `.vue` 和 Rust 文件里;或者命令名字符串在 `api.ts` 之外的地方再次出现。

**正面**:前端事件名只在 `src/lib/events.ts` 的 `EVENTS` 常量表里出现一次;Rust 侧对应一个 `pub const`。命令名字面量只允许出现在 `src/lib/api.ts` 的封装函数内部和 Rust 的 `#[tauri::command]` 函数名上。

### 模式四:重复的 payload 字段提取

**反面**:多个组件各自把 Tauri 事件 payload 或 `invoke` 返回值强转成本地类型:

```ts
// ComponentA.vue
const key = (event.payload as { key?: string }).key;

// ComponentB.vue
const key = (event.payload as { key: string; source?: string }).key;
```

哪怕只有两行,这也是重复的契约逻辑:每个消费方都拥有一份「合法 payload 长什么样」的私有定义。
Rust 侧一改字段名,前端读到 `undefined` 且没有任何报错,两个组件还可能一个改了一个没改。

**正面**:类型映射在 `src/lib/events.ts` 里只定义一次,消费方通过 composable 拿到已带类型的 payload:

```ts
// src/lib/events.ts
export const EVENTS = { SETTINGS_UPDATED: "settings://updated" } as const;
export interface EventPayloads {
  [EVENTS.SETTINGS_UPDATED]: { key: string };
}

// 组件里
useTauriEvent(EVENTS.SETTINGS_UPDATED, (payload) => {
  // payload 已是 { key: string },不需要 as
});
```

`invoke` 返回值同理:泛型参数写在 `src/lib/api.ts` 里(`invoke<Settings>("get_settings")`),
类型定义在 `src/types/<domain>.ts`,组件只消费函数返回值,不做二次断言。

**规则**:同一个未定型的 payload 字段在两处被读取,就要在加第三处之前先建共享类型 / 归一化函数。

### 模式五:两侧各写一份校验

**反面**:Rust 命令已经对 `name.trim().is_empty()` 返回 `AppError::InvalidInput`,前端又在 `api.ts` 里复制一份同样的判空并抛出另一句中文。

**正面**:校验以 Rust 命令层为唯一权威(前端是不可信输入源);前端只做 UI 层面的「禁用按钮」等交互约束(如 `HelloWorld.vue` 的 `canSubmit`),不重复产出错误文案。

---

## 什么时候该抽象

**应该抽象**:

- 同样的代码出现了 3 次以上;
- 逻辑复杂到足以藏 bug(异步状态机、竞态处理、unlisten 清理);
- 它属于 IPC 契约的一部分(命令封装、事件类型、结构体镜像),哪怕当前只有一个调用方。

**不要抽象**:

- 只用一次且不属于契约;
- 一行就写完的琐碎逻辑;
- 抽象本身比重复更难读。

---

## 批量修改之后

对多个文件做了同类改动之后:

1. **复查**:是否覆盖了所有实例?前端和后端都查了吗?
2. **搜索**:用 `grep` 按旧名字、旧文案再搜一遍两侧代码;
3. **反思**:这批改动是不是在提醒你该抽象了?

### 按枢轴值分发的逻辑要写成穷尽结构

当状态由 `kind` / `status` / `action` 这类枢轴值驱动时,用一个穷尽的 `switch`(TS)或 `match`(Rust)集中处理,不要把 `if/else` 散落在多处。

```ts
// 反面:各处各写一段,新增一种 kind 时不知道要改几处
if (kind === "opened") { ... } else if (kind === "closed") { ... }

// 正面:一个 switch 拥有整张转移表;配合 never 断言让漏分支变成编译错误
switch (event.kind) {
  case "opened": ...; return;
  case "closed": ...; return;
  default:
    // 新增 kind 而没加 case 时,这行编译报错
    return event.kind satisfies never;
}
```

Rust 侧的 `match` 天然穷尽,但要避免用 `_ =>` 兜底吞掉新变体——给 `AppError` 加变体时尤其注意。

---

## 陷阱:两套机制产出同一份结果

**问题**:两个不同的机制必须产出同样的东西,但只有一处会自动同步,另一处靠手工维护。本仓库最典型的例子:

- Rust 命令函数 ↔ `lib.rs` 里的 `generate_handler!` 列表:新增命令忘记注册,前端 `invoke` 直接报错;
- Rust 结构体 ↔ `src/types/<domain>.ts` 手写镜像:Rust 改字段,TS 侧不会报错;
- Rust 命令的真实返回 ↔ `api.ts` 的浏览器降级分支:后端改了返回形状,降级分支还是旧形状。

**症状**:Tauri 窗口里正常,浏览器预览里坏掉;或者反过来。

**预防**:

- 能消除不对称就消除:例如降级分支复用同一个 TS 类型,而不是手写字面量;
- 消除不了就补回归测试:Rust 单测锁住返回形状;`api.ts` 两条分支的类型一致由 `vue-tsc` 保证,Vitest 断言降级分支的返回值形状;
- 改目录结构或重命名时,`grep` 所有引用旧名字的路径,包括注释与文档。

---

## 陷阱:AI 交叉评审的假阳性

用 AI 做交叉评审时,「重复代码」类的报告尤其容易误报。收到以下结论时先核实再动手:

| 评审结论 | 核实方式 |
|----------|----------|
| 「前端缺少参数校验」 | 校验是否已在 Rust 命令层完成?前端重复校验反而是本指南反对的重复 |
| 「这两个函数应该合并」 | 它们是否真的属于同一契约?相似形状 ≠ 相同职责 |
| 「常量重复定义」 | TS 的 `EVENTS` 与 Rust 的 `pub const` 各一份是**设计**,跨语言无法共享 |
| 「这段行为变了」 | 先读代码注释——本仓库注释写明了大量有意为之的设计取舍 |
| 「测试有 bug」 | 把被测功能在脑中删掉,测试是否仍能通过?能通过才是空洞测试 |

常见误报模式:

1. **信任边界混淆**:把内部数据(打包进应用的配置、Rust 侧构造的 payload)当成不可信输入要求校验;
2. **忽略设计注释**:把注释里已说明的有意行为标为 bug;
3. **变量误读**:没有追踪到变量真正的定义处就下结论。

**核实规则**:每条 CRITICAL / WARNING 级别的结论都要对照真实代码确认后再排优先级;按大约三分之一的误报率预留精力。

---

## 提交前检查清单

- [ ] 前后端两侧都搜过,确认没有已存在的等价实现
- [ ] 没有本应共享却被复制粘贴的逻辑
- [ ] 没有在共享类型之外重复做 `event.payload as X` / 返回值强转
- [ ] 命令名只出现在 `src/lib/api.ts` 与 Rust 命令函数上;事件名只出现在 `src/lib/events.ts` 与 Rust 常量上
- [ ] 输入校验只在 Rust 命令层做一次,前端没有复制一份错误文案
- [ ] 同类模式采用同一结构(composable 命名、错误处理、loading 状态)
- [ ] 按枢轴值分发的逻辑集中在一个穷尽 `switch` / `match` 里
- [ ] 新增命令已在 `generate_handler!` 注册,新增结构体已同步 TS 镜像
