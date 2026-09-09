# 项目约定(语言、提交、版本与门禁)

> 全仓库通用,前端与后端两层 spec 都以此为前提。

---

## 1. 语言策略

| 对象 | 语言 | 证据 |
|------|------|------|
| 交流、PR / issue 描述、任务文档(`.trellis/`)| **中文** | 本文件、`README.md` |
| 代码注释(`//`、`///`、`//!`、`/** */`、`<!-- -->`)| **中文** | `src-tauri/src/error.rs`、`src/components/launcher/LauncherPanel.vue`、`vite.config.ts` |
| 面向用户的文案(错误信息、UI 文本、日志)| **中文** | `AppError::InvalidInput("名字不能为空")`、`.expect("启动应用失败")` |
| commit 描述 | **中文**(type / scope 用英文关键字) | 见下节 |
| 配置文件注释(`tauri.conf.json5`、`Cargo.toml`、`.oxlintrc.json`、workflow yml)| **中文** | `src-tauri/tauri.conf.json5`、`.github/workflows/ci.yml` |
| 代码标识符:变量、函数、类型、模块、文件名、Tauri 命令名、事件名、CSS 变量名 | **英文** | 全仓库 |

- 中文注释里的标点:与本仓库现有代码保持一致即可,不强制全角或半角;**不要**为了统一标点批量改动无关行。
- 注释写「为什么」而不是复述代码;每个非显而易见的常量、cfg、workaround 都要有一句中文说明其存在原因(参考 `src-tauri/Cargo.toml` 中 `_lib` 后缀、`vite.config.ts` 中 `clearScreen: false` 的注释)。

## 2. 提交约定

- 格式:`type(scope): 中文描述`,遵循 Conventional Commits 的 type 集合:`feat` / `fix` / `docs` / `style` / `refactor` / `perf` / `test` / `build` / `ci` / `chore` / `revert`。
- scope 可选,用英文小写,常用:`frontend`、`backend`、`ipc`、`ci`、`release`、`spec`、`deps`。
- 发版提交由脚本固定生成为 `chore(release): vX.Y.Z`(`scripts/release.ts`),不要手写。
- 描述用中文陈述句,不加句号,不加 emoji。示例:`feat(backend): 新增剪贴板读取命令`、`fix(frontend): 浏览器预览下唤出键键帽未渲染`。
- 一次提交只做一件事;husky + lint-staged + commitlint 本仓库**尚未接入**(见「待讨论」)。

## 3. 版本与发布

- 版本号有三处:`package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json5`,**只能**通过 `bun run release <x.y.z|patch|minor|major>` 同步修改(`scripts/release.ts` + `scripts/version.ts`),禁止手改任何一处。
- `bun run version:check` 校验三处一致;CI 的 `release.yml` 在打 tag 后再校验一次。
- tag 形如 `vX.Y.Z`,推送后由 `.github/workflows/release.yml` 做四平台矩阵打包并原子创建 Release。

## 4. 质量门禁(必须通过才算完成)

与 `.github/workflows/ci.yml` 对应(CI 跑 format:check / lint / test / build 与 Rust 三条;`format` 是本地写入步骤,CI 用 `format:check` 只检查不改文件),本地提交前跑全套:

```bash
# 前端
bun run format          # Prettier --write(含 Tailwind 类名排序);本地写入
bun run format:check    # Prettier --check,与 CI 一致,格式漂移即失败
bun run lint            # oxlint(含 import/no-cycle)
bun run test            # Vitest
bun run build           # vue-tsc -b 全量类型检查 + vite build

# 后端(在 src-tauri/ 下,CI 双平台 Windows + Ubuntu)
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

- clippy 警告视为错误,不允许用 `#[allow(...)]` 掩盖,除非注释写明原因。
- `.oxlintrc.json` 的 `ignorePatterns` 把 `.trellis/`、`.pi/` 等工作流脚手架排除在外;产品代码不得放进这些目录。

### 4.1 dev server 与端口 1420(AI 代理必读)

背景:`vite.config.ts` 设 `strictPort: true` + 端口 1420,`tauri.conf.json5` 的 `devUrl` 指向同一端口。曾出现代理用 `( bun run dev & )` 脱离会话启动 Vite 验证后未清理,留下常驻 node 进程占住 1420;也出现过代理见端口被占就 `taskkill` 掉用户手动开的 `tauri dev` 再自己重启。以下为硬约束:

- **`bun run tauri dev` 是用户手测专用命令,代理禁止运行。** 它会弹窗、注册全局快捷键、挂托盘,代理既看不见也操作不了。任务文档里写「`tauri dev` 手测」时,执行主体是用户:代理跑完第 4 节的自动化门禁后,**列出手测步骤提示用户**即可。
- **端口 1420 被占 = 用户的 dev server 正在运行。** 需要预览前端时直接访问 `http://localhost:1420` 复用;**禁止** `netstat` 找 PID 后 `taskkill` / `kill` 占用进程,端口冲突只能向用户说明并等用户决定。
- 代理确需自行起纯前端预览(`bun run dev`)时:用会话托管的后台任务方式启动(pi 的 `bash background=true`),验证完立刻 `task_stop`;**禁止** `( … &)`、`nohup`、`start /b` 等脱离会话的启动方式。结束前用 `netstat -ano | findstr :1420` 确认无残留。
- `bun run dev` 底下实际是 `bun → vite.exe(shim) → node vite.js`(Bun 尊重 `#!/usr/bin/env node` shebang),排查端口占用时看到 node 进程属正常,清理需连 bun / vite / node 三个一起。

## 5. 脚本与工具链

- 包管理器固定 **bun**(`bun.lock`,CI `--frozen-lockfile`);不要引入 npm / pnpm lock。
- Rust 工具链由 `rust-toolchain.toml` 锁定 stable + rustfmt + clippy;MSRV 1.85(edition 2024)。
- 工程脚本放 `scripts/*.ts`,用 bun 直接运行;只用 Node 内置模块,git 调用一律 `execFileSync` 传数组参数、不拼 shell 字符串(`scripts/release.ts` 文件头注释)。

## 6. 待讨论(业界常见做法、本仓库尚未采纳)

以下做法业界常见,本仓库暂未接入;需要时另开任务,不要在业务任务里顺手加:

- husky + lint-staged + commitlint(lint-staged 可一并跑 `cargo fmt` + `clippy`)。
- Rust 侧 `[profile.release]` 体积优化(`lto` / `codegen-units = 1` / `strip` / `panic = "abort"`)。
