# 前端开发规范(Vue 3 + TypeScript + Tailwind CSS 4)

> `src/` 目录的编码约定。写代码前读「开发前检查清单」,写完对照「质量检查」。
> 全项目通用约定(语言、提交、版本)见 `../guides/project-conventions.md`;前后端 IPC 契约见 `../guides/ipc-contract.md`。

---

## 规范索引

| 文件 | 内容 | 什么时候读 |
|------|------|-----------|
| [directory-structure.md](./directory-structure.md) | `src/` 布局、按需扩展的目录、依赖方向、文件命名 | 新建任何文件之前 |
| [component-guidelines.md](./component-guidelines.md) | SFC 形态、props / emits、组件内状态套路、模板、图标 | 写或改 `.vue` |
| [composable-guidelines.md](./composable-guidelines.md) | `useXxx` 的命名、返回形态、生命周期清理 | 抽取可复用响应式逻辑 |
| [state-management.md](./state-management.md) | 状态放哪、Pinia setup store 写法、Rust 作为唯一真相 | 状态要跨组件共享时 |
| [ipc-guidelines.md](./ipc-guidelines.md) | `api.ts` 封装、参数 / 返回 / 错误契约、浏览器降级、事件与类型镜像 | 任何与 Rust 通信的改动 |
| [type-safety.md](./type-safety.md) | tsconfig 基线、类型放哪、IPC 边界类型映射 | 定义或修改类型 |
| [styling-guidelines.md](./styling-guidelines.md) | 三层设计令牌(shadcn v4 命名)、深浅色、交互 / 焦点范式、表面层级、z-index 档位、桌面端约定、组件变体写法 | 写样式、加颜色、写按钮 / 弹层 / 表单控件 |
| [quality-guidelines.md](./quality-guidelines.md) | 门禁命令、测试、注释、日志、可访问性、依赖 | 提交前 |

## 开发前检查清单

1. 读 `directory-structure.md`,确认新文件的目录与命名;不要新建同义目录。
2. 涉及 IPC → 读 `ipc-guidelines.md` + `../guides/ipc-contract.md`,确认 `invoke` 只在 `src/lib/api.ts`(或 `src/lib/api/**/*.ts`),并有浏览器降级分支。
3. 涉及样式 → 读 `styling-guidelines.md`,只用语义令牌工具类。
4. 涉及共享状态 → 读 `state-management.md`,先判断是否真的需要 store。
5. 参照 `src/components/HelloWorld.vue` 与 `src/lib/api.ts` 的注释密度和写法,保持一致。
6. 所有注释、文案、日志前缀用中文;标识符用英文。

## 质量检查

- [ ] `bun run format && bun run format:check && bun run lint && bun run test && bun run build` 全部通过。
- [ ] `.vue` / composable / store 中没有 `import { invoke }` / `import { listen }`(事件 composable 除外)。
- [ ] 每个 `invoke<T>()` 有泛型;每个 `api.ts` 函数有非 Tauri 分支。
- [ ] 没有 `any`、`!` 非空断言、TS `enum`、`console.log`。
- [ ] 组件内没有字面色值、`bg-zinc-*`、`dark:` 变体。
- [ ] 主按钮用 `bg-primary text-primary-foreground`,不用 `accent`(`accent` 只做 hover / 选中叠加底)。
- [ ] 实底按钮 hover 用 `hover:bg-primary/90` 等实底变色,不用 `hover:opacity-*`。
- [ ] 没有裸 `z-*` 数字或 `z-[...]` 任意值,层级一律 `z-(--z-*)`。
- [ ] `listen` / 定时器都有 `onUnmounted` 清理。
- [ ] 新增的 `lib/` 纯函数有同目录 `*.test.ts`。
- [ ] 新的 `ref` / 配置项都有一行中文注释说明「为什么」。

## 关键决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 样式方案 | Tailwind CSS 4 三层令牌 | 本仓库已实现完整令牌体系;CSS-first 配置与 `light-dark()` 原生深浅色,不换 |
| 令牌词汇表 | 对齐 shadcn/ui v4 命名(`primary` / `card` / `popover` / `destructive` / `input` …),机制保留 `light-dark()` | Tailwind 生态事实标准,未来接 shadcn-vue / Reka UI 零成本;不照搬 `.dark` 类,不引入 cva / tailwind-variants |
| 组合式函数目录 | `composables/` | Vue 官方术语 |
| props 默认值 | 解构默认值,不用 `withDefaults` | Vue 3.5 官方推荐 |
| SFC 块顺序 | script → template → style | 业界无统一做法,按本仓库现有文件统一 |
| 命令名管理 | 字面量写在封装函数内 | 与 Tauri 官方用法一致;命令名只在封装函数内出现一次,无需常量表 |
| 错误契约 | Rust 序列化为中文字符串,前端直接展示 | 最简单可用;结构化 `{kind,message}` 待需要时再引入 |
| 事件名 | `domain://action` | 领域前缀避免事件名冲突,便于按领域 grep |
| 类型共享 | 手写 TS 镜像 | 不引入 specta / ts-rs,少一层构建链依赖;镜像文件头注明 Rust 路径便于同步 |
| 前端持久化 | 暂不规定 | 业界无统一做法,本项目暂不规定 |
| 页面目录名 | 暂不规定 | `views/` 与 `pages/` 业界无统一做法,引入路由时再定 |
