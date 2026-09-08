# 填充项目开发规范(Bootstrap Guidelines)

> 本任务由 `trellis init` 自动创建。原模板为英文通用说明,已按本项目实际范围重写。

## 目标

把 `.trellis/spec/` 从空模板变成**本仓库真实约定 + 开源社区事实标准**的中文编码规范,让后续每个 AI 会话(`trellis-implement` / `trellis-check` 子代理)写出与本项目风格一致、人类易维护的代码。

## 范围

### 语言策略(全项目)

- 交流、代码注释、文档、commit 描述一律**中文**。
- 代码标识符(变量、函数、类型、文件名、命令名、事件名)一律**英文**。
- 该策略写入共享指南并在 frontend / backend 两层 index 中引用。

### spec 分层

当前只有 `frontend/` 一层,与本仓库前后端双栈的事实不符。目标结构:

| 目录 | 覆盖内容 | 主要证据来源 |
|------|----------|--------------|
| `spec/frontend/` | Vue 3 + TS + Tailwind 4:目录分层、组件、composable、状态、类型、IPC 调用封装、样式令牌、质量门禁 | `src/`、`vite.config.ts`、`.oxlintrc.json`、`.prettierrc`、`vitest.config.ts` |
| `spec/backend/` | Rust + Tauri 2:模块分层、命令写法、错误处理、状态与事件、日志、配置与权限、质量门禁 | `src-tauri/src/`、`Cargo.toml`、`tauri.conf.json5`、`capabilities/` |
| `spec/guides/` | 跨层共享:语言策略与提交约定、IPC 契约(命令名 / 参数 key / 错误 / 事件)、代码复用与跨层思考指南(中文化) | 官方 Tauri 文档、README、`scripts/release.ts`、CI workflow |

### 调研来源

本地浅克隆(`%TEMP%/tauri-research/`)后由只读 scout 分析,结论按「公认 / 多数 / 分歧」分档:

- Tauri 2 + Vue 3:HuLa、BiliTools、PakePlus、JiwuChat(Nuxt)、AIaW(Quasar)、hoppscotch-desktop
- Rust 侧参考(前端非 Vue,仅看 src-tauri):clash-verge-rev、EcoPaste
- 官方口径:tauri-apps/plugins-workspace(fs / store / dialog / log / shell)、create-tauri-app 模板、v2.tauri.app 文档

### 不做的事

- 不改产品源码(`src/`、`src-tauri/`)。若调研发现本仓库与公认做法有出入,写入 spec 的「待讨论」小节,不擅自改代码。
- 不写入无证据支撑或社区分歧明显的做法;分歧项要么不写,要么明确标注「本项目选择 X,原因 …」。
- 不写平台专属(某个 AI 宿主)的操作说明。

## 验收标准

- [x] `spec/frontend/`、`spec/backend/`、`spec/guides/` 三个目录均为中文,无 "(To be filled)"、"placeholder"、英文模板残留。
- [x] 每个 `index.md` 含「开发前检查清单」与「质量检查」两节,且索引表与目录下实际文件一一对应。
- [x] 每条重要规则都能指向本仓库真实文件路径(或官方文档 / 被调研项目路径作为佐证)。
- [x] 有明确的「禁止模式」小节,并说明原因。
- [x] `python ./.trellis/scripts/get_context.py --mode packages` 输出的 Spec layers 包含 `frontend` 与 `backend`。
- [x] `grep -R "To be filled\|TODO: fill\|placeholder" .trellis/spec` 无结果。
- [x] 与 README「工程规范与架构设计」一节口径一致,不自相矛盾。

## 状态

- [x] 前端 spec 填充完成
- [x] 后端 spec 填充完成
- [x] 共享指南中文化并补充 IPC 契约
- [x] 验收命令全部通过

## 调研结论存档

`research/01~04-*.md`:四份 scout 报告(前端约定 / Rust 侧 Vue 项目 / Rust 侧大型与官方 / IPC 契约与工程化),spec 中「N/3」「N/5」计数的原始出处。

## 调研过程中发现的产品问题(超出本任务范围,待用户决定)

- 仓库零测试文件,`bun run test` 退出码 1 → CI 前端 job 当前必红。建议补 `src/lib/runtime.test.ts`。
- CI 不跑 `prettier --check`,格式漂移不会被门禁拦住。
- `Cargo.toml` 中 `serde` / `serde_json` / `tauri-build` 无注释(spec 已将规则收窄为「非通用依赖必须注释」)。

## 完成后

```bash
python ./.trellis/scripts/task.py finish
python ./.trellis/scripts/task.py archive 00-bootstrap-guidelines
```
