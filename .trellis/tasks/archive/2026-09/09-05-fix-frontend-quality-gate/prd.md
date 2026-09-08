# 修复前端质量门禁

## 背景

填充 spec 时发现两处门禁缺口(记录于 `.trellis/spec/guides/project-conventions.md`「待讨论」):

1. 仓库没有任何 `*.test.ts`,Vitest 在零测试文件时以退出码 1 结束 → `bun run test` 失败,`.github/workflows/ci.yml` 的前端 job 当前必红。
2. CI 不检查格式(`bun run format` 是 `prettier --write`,只在本地跑),格式漂移不会被门禁拦住。

## 需求

### 1. 首个前端单测

- 新增 `src/lib/runtime.test.ts`,测试 `isTauriRuntime()`:
  - 默认(Vitest node 环境)`globalThis` 无 `__TAURI_INTERNALS__` → 返回 `false`;
  - 用 `vi.stubGlobal("__TAURI_INTERNALS__", {})` 注入后 → 返回 `true`;每个用例结束 `vi.unstubAllGlobals()`。
- 测试描述用中文;文件与被测源码同目录(`vitest.config.ts` 的 `include: src/**/*.test.ts`)。
- 不改 `vitest.config.ts`,不开 `passWithNoTests`。

### 2. CI 增加格式检查

- `package.json` 新增脚本 `"format:check": "prettier --check src/ scripts/"`,与 `format` 的作用范围一致。
- `.github/workflows/ci.yml` 前端 job 在 Lint 之前增加一步「格式检查(Prettier)」,`run: bun run format:check`;步骤名中文,与现有步骤风格一致。
- `.github/workflows/release.yml` 的 `verify` job 同样增加该步骤(它已经跑 lint / test)。
- README「常用开发指令速查」表补一行 `bun run format:check`;「核心特性 → 严格的多层代码质量门禁 → 前端」补上 Prettier 检查已进 CI。

### 3. spec 同步

- `.trellis/spec/guides/project-conventions.md`:门禁命令块加入 `bun run format:check`,「待讨论」删除已解决的两条(prettier --check、零测试文件)。
- `.trellis/spec/frontend/quality-guidelines.md`:门禁命令块加入 `format:check`;删除「现状:仓库目前没有任何 *.test.ts…」段落与「待讨论」小节;文件头说明改为四条门禁与 CI 一致。
- `.trellis/spec/frontend/index.md` 质量检查第一条去掉「注意:仓库现在零测试文件…」括注。

## 约束

- 不改 `src/lib/runtime.ts` 等产品逻辑;只新增测试文件、脚本、CI 步骤与文档。
- 遵循 `.trellis/spec/frontend/quality-guidelines.md`(测试同目录、中文描述)与 `guides/project-conventions.md`(中文注释、commit 格式)。
- 先运行 `bun run format` 确保新文件格式合规,再运行 `bun run format:check && bun run lint && bun run test && bun run build` 全绿。

## 验收标准

- [x] `bun run test` 退出码 0,输出 2 个用例通过。
- [x] `bun run format:check` 退出码 0。
- [x] `bun run lint && bun run build` 通过。
- [x] `ci.yml` 与 `release.yml` 含格式检查步骤,YAML 有效。
- [x] spec 与 README 中不再把「零测试文件 / CI 不查格式」描述为现状。
