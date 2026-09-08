# 前端质量规范

> 提交前必须通过的门禁与代码卫生规则。`format:check / lint / test / build` 四条与 `.github/workflows/ci.yml` 的 `frontend` job 一致;`format` 是本地写入步骤,先跑它再跑四条门禁。

---

## 1. 门禁命令

```bash
bun run format        # Prettier --write src/ scripts/(printWidth 100、LF、Tailwind 类名排序);.prettierignore 额外排除 tauri.conf.json5 防编辑器保存时误格式化
bun run format:check  # Prettier --check,范围同上;CI 跑的是这条,格式漂移即失败
bun run lint          # oxlint:plugins typescript/unicorn/oxc/import/vue;correctness 全为 error;import/no-cycle = error
bun run test          # Vitest,include src/**/*.test.ts
bun run build         # vue-tsc -b(全量类型检查)+ vite build
```

`format` 之后的四条全绿才算完成。oxlint 极快(毫秒级),不要为了省时间跳过。

## 2. 测试

- 测试文件与被测源码**同目录**,命名 `<name>.test.ts`(`vitest.config.ts` 注释与 `include` 规则)。
- `vitest.config.ts` 独立于 `vite.config.ts`,没有 Vue 插件与 DOM 环境:当前只收集 `src/**/*.test.ts`,测的是**纯函数**(`src/lib/`)。`scripts/` 不在 include 内。要测组件需另开任务引入 `@vue/test-utils` + `happy-dom`,不要在业务任务里顺手加。
- 测试名用中文描述行为:`it("空名字返回降级文案", …)`。
- 每个 `lib/` 里的工具函数都应有测试;写测试时问一句「删掉被测功能,这个测试还过吗?」过的话就是同义反复测试。
- 本仓库把 `test` 放进 CI 正是为了保证前端单测不缺席,不要让 `src/**/*.test.ts` 长期为零。

## 3. 注释与可读性

- 注释中文,写「为什么」:每个 `ref` 一行 `/** */`(代表什么、何时变、与谁互斥);每个非显然的配置项一行说明(参考 `vite.config.ts`、`HelloWorld.vue`)。
- 函数级 JSDoc 说明降级行为与失败方式(`src/lib/api.ts`)。
- 不写复述代码的注释(`// 设置 loading 为 true`)。
- 经验参考值(非硬标准):文件超过约 200 行或组件同时管理 ≥5 个互相关联的 `ref`,考虑拆 composable / 子组件。

## 4. 日志

- 只允许 `console.error` / `console.warn`,且带中文前缀:`console.error("问候失败:", error)`。
- 不留 `console.log` 调试语句(本仓库未加 lint 规则强制,靠评审,但要求相同)。

## 5. 可访问性

- 装饰图标 `aria-hidden="true"`;可点击元素用 `<button>` 而不是 `<div @click>`。
- 表单控件有 `placeholder` 或 `<label>`;仅图标按钮要有 `aria-label`。
- 焦点可见:统一用 `outline-hidden focus-visible:ring-3 focus-visible:ring-ring/50`(表单控件再加 `focus-visible:border-ring`),`outline-hidden` 后必须补焦点样式(用 `outline-hidden` 不用 `outline-none`,高对比模式下才不丢焦点);`index.css` 基础层的 `outline-color` 只是兜底,不是免写 ring 的理由。
- reduced-motion 由 `index.css` 基础层 `@media (prefers-reduced-motion: reduce)` 全局保证,组件不写 `motion-reduce:` 变体;动效只用 `transition-colors` / `animate-spin` 等默认工具类。
- 文字与底色对比度满足 WCAG AA(4.5:1):正文用 `text-foreground` / `text-card-foreground`,错误用 `text-destructive`;`text-muted-foreground` 只用于辅助说明,不承载关键信息。

## 6. 依赖

- 新增依赖前先看现有栈能否解决;新依赖要在 PR 描述里写用途。
- 图标集、构建工具放 `devDependencies`;只有运行时需要的进 `dependencies`(当前仅 `@tauri-apps/api`、`vue`、`tailwindcss`)。
- 锁文件只有 `bun.lock`;不要生成 `package-lock.json` / `pnpm-lock.yaml`。

## 7. 禁止

- `console.log`、`debugger`。
- `eslint-disable` / `oxlint-disable` 无原因注释。
- 跳过任一门禁提交。
- 在 `.oxlintrc.json` `ignorePatterns` 里加产品代码目录。
