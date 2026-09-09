<div align="center">

# ⚡ z-tools

常驻托盘、键盘优先的桌面效率启动器。  
一键唤出面板，把日常小工具收进同一个入口——首个内置工具是**剪贴板历史**。

[![Tauri 2](https://img.shields.io/badge/Tauri-v2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Vue 3](https://img.shields.io/badge/Vue-v3-4FC08D?style=flat-square&logo=vuedotjs&logoColor=white)](https://vuejs.org/)
[![Tailwind CSS v4](https://img.shields.io/badge/Tailwind_CSS-v4-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![Rust 2024](https://img.shields.io/badge/Rust-Edition_2024_(MSRV_1.85)-DEA584?style=flat-square&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![Bun](https://img.shields.io/badge/Bun-v1.x-FBF0DF?style=flat-square&logo=bun&logoColor=black)](https://bun.sh/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](LICENSE)

[✨ 功能](#-功能) · [📥 安装](#-安装) · [🚀 开发](#-开发) · [📐 架构](#-架构与工程规范) · [📦 发版](#-发版与-cicd)

</div>

---

## 🌟 功能

### 启动器面板

- **托盘常驻，不占任务栏**：启动即隐藏到系统托盘，托盘左键开合面板，右键菜单「打开启动器 / 退出」。
- **全局快捷键唤出**：默认 <kbd>Alt</kbd> + <kbd>Enter</kbd>，在任意前台应用下呼出 / 收起面板（定义于 `src-tauri/src/launcher.rs` 的 `DEFAULT_TOGGLE_SHORTCUT`）。
- **无边框透明面板**：宽 800px，高度随内容自动伸缩；每次唤出按当前显示器工作区重新定位（水平居中、顶边在 1/4 处），始终置顶。
- **失焦即隐藏**：点到别处自动收起；<kbd>Alt</kbd>+<kbd>F4</kbd> / 关闭请求只隐藏不退出；Windows 下拦截 <kbd>Alt</kbd> 弹出的系统菜单。
- **工具搜索**：主页输入关键词（中英文 / 拼音缩写）过滤工具，键盘导航进入；页脚实时显示当前页可用键位，无需记忆。

### 剪贴板历史（首个内置工具）

- **自动记录**：监听系统剪贴板变化，捕获 **文本**（≤ 1 MiB）、**图片**（≤ 20 MiB，落盘 PNG 原图 + 256px 缩略图）、**文件列表**（路径）。
- **去重与保留**：blake3 内容哈希去重，重复复制只刷新时间；最多保留 **500** 条非收藏记录，超出自动淘汰最旧项（含图片文件）；收藏项不受限制。
- **一键粘贴回去**：选中条目确认或点击 → 写回剪贴板 → 切回唤出前的前台窗口 → 模拟 <kbd>Ctrl</kbd>+<kbd>V</kbd>；目标窗口不可达时退化为仅复制。
- **分类与检索**：全部 / 文本 / 图片 / 文件四个标签页；文本按内容、文件按文件名模糊搜索；收藏过滤；滚动到底自动加载更多。
- **管理**：收藏 / 取消收藏、删除（同时清理图片文件）、展开查看完整内容。

> [!NOTE]
> 剪贴板**监听与模拟粘贴**目前只在 **Windows** 上实现；macOS / Linux 可编译运行启动器，但剪贴板工具不会录入历史，粘贴命令返回「不支持」。

## 📥 安装

从 [GitHub Releases](https://github.com/Zach9Qi/z-tools/releases) 下载对应平台安装包：

| 平台 | 安装包 | 说明 |
|---|---|---|
| **Windows x64** | `.msi` / `-setup.exe` | 完整功能（推荐） |
| **macOS** | `.dmg`（Apple Silicon / Intel） | 仅启动器壳，剪贴板工具不可用 |
| **Linux x64** | `.deb` / `.rpm` / `.AppImage` | 仅启动器壳，剪贴板工具不可用 |

> [!IMPORTANT]
> 安装包未做代码签名：Windows 首次运行会被 SmartScreen 拦截，选择「仍要运行」即可；macOS 需在「系统设置 → 隐私与安全性」中手动放行。

### 数据与日志目录

所有落盘数据统一放在应用本地数据目录（Windows 为 `%LOCALAPPDATA%\com.zachq.z-tools\`），不写 Roaming：

```text
com.zachq.z-tools/
├── clipboard/
│   ├── history.db      # 剪贴板历史（SQLite）
│   └── images/         # 图片原图与缩略图（<hash>.png / <hash>.thumb.png）
├── logs/               # 运行日志（开发 Debug / 发布 Info）
└── EBWebView/          # WebView2 缓存
```

**完全卸载 / 重置**：卸载程序后删除上述目录即可。

## 🚀 开发

### 前置准备

1. [Bun](https://bun.sh/) ≥ 1.x —— 依赖管理与脚本执行。
2. [Rust](https://rustup.rs/) —— 进入仓库后 `rust-toolchain.toml` 自动切换到 stable（≥ 1.85）并补齐 `rustfmt` / `clippy`。
3. 系统依赖 —— 参见 Tauri 官方 [Prerequisites](https://v2.tauri.app/start/prerequisites/)（Windows 需 C++ 构建工具 + WebView2，Linux 需 WebKitGTK）。

### 常用指令

```bash
bun install            # 安装依赖
bun run tauri dev      # 桌面端完整调试（首次编译 Rust 较慢）
bun run dev            # 仅浏览器预览前端，IPC 自动 mock，免编译 Rust
```

| 指令 | 作用 |
|---|---|
| `bun run build` | `vue-tsc -b` 全量类型检查 + Vite 打包 |
| `bun run format` / `format:check` | Prettier 格式化 / 检查（含 Tailwind 类名排序） |
| `bun run lint` | Oxlint 静态诊断 |
| `bun run test` | Vitest 前端单测 |
| `bun run version:check` | 校验 `package.json` / `Cargo.toml` / `tauri.conf.json5` 三处版本一致 |
| `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` | Rust 侧质量门禁（在 `src-tauri/` 下执行） |

> [!TIP]
> **浏览器预览模式**：`bun run dev` 下 `src/lib/runtime.ts` 检测到不在 Tauri WebView 中，`src/lib/api/` 的命令封装会降级为 no-op 或假数据（剪贴板列表带「(浏览器预览)」标记），窗口尺寸同步与事件订阅直接跳过。适合无 Rust 环境下调 UI；托盘、全局快捷键、剪贴板监听等原生能力需 `bun run tauri dev` 联调。

## 📂 项目结构

```text
.
├── .github/workflows/
│   ├── ci.yml                    # CI 门禁：前端 Lint/测试/构建 + Rust（Windows / Ubuntu）
│   └── release.yml               # tag 触发四平台矩阵打包并发布 GitHub Release
├── scripts/
│   ├── release.ts                # 一键发版：安全检查 → 同步版本 → commit → tag → push
│   └── version.ts                # 三处版本号读写、一致性校验、Cargo.lock 刷新
├── src/                          # 前端（Vue 3 + TS）
│   ├── components/
│   │   ├── common/               # 通用组件（KeyboardKey）
│   │   └── launcher/             # 启动器壳：面板、搜索栏、结果网格、磁贴、页脚
│   ├── composables/              # useKeymap / useRowNavigation / useAutoHeight / useTauriEvent
│   ├── stores/keymap.ts          # Pinia：当前页快捷键登记表（页脚提示来源）
│   ├── lib/
│   │   ├── api/                  # 唯一 IPC 入口（launcher.ts / clipboard.ts），带浏览器降级
│   │   ├── events.ts             # Rust → 前端事件名与 payload 类型（与 Rust 侧一一对应）
│   │   ├── runtime.ts            # 是否运行在 Tauri WebView
│   │   ├── window.ts             # 窗口尺寸同步（唯一 @tauri-apps/api/window 入口）
│   │   └── launcher/             # 纯函数：搜索分区、方向键导航、键位标签 + 单测
│   ├── tools/
│   │   ├── registry.ts           # 工具登记表（新增工具在此注册）
│   │   ├── icons.ts              # 工具图标映射
│   │   └── clipboard/            # 剪贴板历史工具：页面、行 / 详情 / 标签组件、数据 composable
│   ├── types/                    # 跨模块类型（tool.ts 工具契约、clipboard.ts）
│   ├── App.vue · main.ts · index.css
├── src-tauri/                    # Rust 后端
│   ├── capabilities/default.json # 窗口能力：core:default + allow-set-size
│   ├── migrations/               # sqlx 迁移脚本（编译期内嵌，校验和锁定）
│   ├── src/
│   │   ├── lib.rs                # 装配：插件、setup_desktop（快捷键 / 托盘 / 窗口事件 / 剪贴板监听）、命令挂载
│   │   ├── main.rs               # 可执行入口（Windows 发布版隐藏控制台）
│   │   ├── launcher.rs           # 面板显示 / 隐藏 / 定位 / 失焦策略 / 默认唤出键
│   │   ├── launcher/windows.rs   # Windows：拦截 Alt 系统菜单
│   │   ├── tray.rs               # 托盘图标与菜单
│   │   ├── clipboard.rs          # 剪贴板领域：类型、限额、监听装配、粘贴回写
│   │   ├── clipboard/store.rs    # SQLite 存储层（sqlx，去重 / 淘汰 / 分页）
│   │   ├── clipboard/backend.rs  # arboard 跨平台读写
│   │   ├── clipboard/windows.rs  # Windows：WM_CLIPBOARDUPDATE 监听、前台窗口切换、SendInput
│   │   ├── commands/             # 薄命令层：launcher.rs / clipboard.rs
│   │   └── error.rs              # 统一 AppError，序列化为中文提示
│   ├── Cargo.toml
│   └── tauri.conf.json5          # Tauri 配置（JSON5，逐项中文注释）
└── rust-toolchain.toml · package.json · vite.config.ts · vitest.config.ts · .oxlintrc.json · .prettierrc
```

## 📐 架构与工程规范

### 前后端通信分层

- **组件禁止直接 `invoke`**：IPC 统一收敛在 `src/lib/api/<domain>.ts`，由 `src/lib/api/index.ts` 汇出。
- **Tauri API 三个入口**：`@tauri-apps/api/core` 只在 `lib/api/**`；`@tauri-apps/api/window` 只在 `lib/window.ts`（仅 setSize）；`@tauri-apps/api/event` 只在 `composables/useTauriEvent.ts`。窗口显示 / 隐藏由 Rust 控制，前端隐藏走 `hideLauncher()` 命令。
- **错误直出**：可失败命令返回 `Result<T, AppError>`，前端捕获到的 `error` 已是可读中文字符串，直接展示。

### 工具扩展契约

每个工具是 `src/tools/<name>/` 下一个自包含模块，实现 `src/types/tool.ts` 的契约（id、名称、关键词、图标、页面组件），在 `src/tools/registry.ts` 注册即可出现在主页搜索中。工具页通过 `useKeymap` 登记快捷键，页脚提示自动同步；需要后端能力时在 `src-tauri/src/commands/<name>.rs` 加薄命令并在 `lib.rs` 挂载。

### Rust 后端

- **薄命令**：`commands/` 只做参数校验与转发，业务逻辑在领域模块（`launcher.rs` / `clipboard.rs`）。
- **平台代码隔离**：Windows 专属实现放在 `<domain>/windows.rs` 并以 `#[cfg(windows)]` 编译；非 Windows 平台不写假实现，缺失能力明确返回 `AppError::Unsupported`。
- **持久化约定**：只用 `app_local_data_dir()`，不用 Roaming；只用运行时 `sqlx::query*`，不用 `query!` 宏（避免编译期依赖数据库）。
- **日志**：`log::*` 门面宏 + `tauri-plugin-log`，禁止 `println!`。

### Tailwind CSS v4 三层令牌

`src/index.css`：原始层 `:root`（唯一允许写具体色值处，`light-dark()` 跟随系统深浅色）→ 语义层 `@theme inline`（`--color-background`、`--color-accent` …）→ 组件直接用 `bg-background` / `text-muted-foreground` 等工具类；不写 `dark:` 变体。

## 📦 发版与 CI/CD

### 一键发版

```bash
bun run release 0.2.0            # 指定版本
bun run release patch            # patch / minor / major 自动递增
bun run release 0.2.0-beta.1     # 预发布（Release 自动标 prerelease）
bun run release patch --dry-run  # 只预览计划
bun run release patch --no-push  # 本地 commit + tag，不推送
```

`scripts/release.ts` 依次执行：工作区洁净检查 → 位于 `main` 且与 `origin/main` 同步 → tag 不重复 → 同步三处版本号并刷新 `Cargo.lock` → `chore(release): vX.Y.Z` 提交 → 附注 tag → `git push --follow-tags`。

### 流水线

- **`ci.yml`**：push / PR 到 `main` 触发。前端 Prettier + Oxlint + `vue-tsc` + Vitest；Rust 在 `windows-latest` 与 `ubuntu-24.04` 并行 fmt / clippy / test。
- **`release.yml`**：推送 `v*` tag 触发。

```text
[ Tag v* ] ──> verify（版本一致性 + 前端门禁）
                  └──> build（并行）
                        ├── Windows x64     .msi / -setup.exe
                        ├── macOS aarch64   .dmg
                        ├── macOS x64       .dmg（ARM Runner 交叉编译）
                        └── Linux x64       .deb / .rpm / .AppImage
                              └──> publish（全部成功后一次性创建 GitHub Release）
```

任一平台失败则不发布，不会留下缺平台的半成品 Release。

## 🗺️ 已知限制与路线

- 剪贴板监听 / 粘贴仅 Windows；macOS / Linux 待实现。
- 模拟粘贴对以管理员权限运行的目标窗口无效（UIPI 限制）。
- 唤出快捷键暂不可配置，无设置页、无开机自启。
- 未支持：HTML / RTF 富文本、纯文本粘贴、多选、数字键快速粘贴、OCR。
- 数据库迁移失败或文件损坏时应用拒绝启动，需手动删除 `clipboard/history.db`。

## 📄 许可证

[MIT](LICENSE)
