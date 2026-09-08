<div align="center">

# ⚡ Tauri 2 + Vue 3 Starter

开箱即用的现代化跨平台桌面应用开发脚手架与 GitHub 模板仓库。  
集成 **Tauri 2** + **Vue 3** + **TypeScript** + **Tailwind CSS v4**，预设严苛的工程化规范、双向降级浏览器预览、多平台 CI 质量门禁与一键自动化矩阵打包发版流水线。

[![Tauri 2](https://img.shields.io/badge/Tauri-v2-24C8D8?style=flat-square&logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![Vue 3](https://img.shields.io/badge/Vue-v3-4FC08D?style=flat-square&logo=vuedotjs&logoColor=white)](https://vuejs.org/)
[![Tailwind CSS v4](https://img.shields.io/badge/Tailwind_CSS-v4-06B6D4?style=flat-square&logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.x-3178C6?style=flat-square&logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Bun](https://img.shields.io/badge/Bun-v1.x-FBF0DF?style=flat-square&logo=bun&logoColor=black)](https://bun.sh/)
[![Rust 2024](https://img.shields.io/badge/Rust-Edition_2024_(MSRV_1.85)-DEA584?style=flat-square&logo=rust&logoColor=black)](https://www.rust-lang.org/)
[![GitHub Actions CI](https://img.shields.io/badge/CI-Passing-brightgreen?style=flat-square&logo=githubactions&logoColor=white)](.github/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue?style=flat-square)](https://opensource.org/licenses/MIT)

[✨ 使用此模板新建项目](#-基于模板新建项目) · [🚀 快速开始](#-快速开始) · [📋 定制清单](#-新项目必改清单) · [📐 架构规范](#-工程规范与架构设计) · [📦 自动化发版](#-自动化发版与-cicd)

</div>

---

## 🌟 核心特性 (Features)

不同于基础的 Hello World，本脚手架定位为**直接面向生产环境的工程化起步模板**，为你抹平跨平台桌面端开发初期的大量重复配置：

- ⚡ **极致现代化前端栈**：Vue 3.5 (`<script setup>`) + Vite 6 + TypeScript 严格模式，基于 **Bun** 驱动包管理与脚本执行，冷启动与构建瞬时完成。
- 🎨 **Tailwind CSS v4 原生设计令牌**：采用全新的 CSS-first 架构，内置三层设计令牌体系（原始层 → 语义层 → 工具类），利用 `light-dark()` 原生 CSS 函数实现**跟随系统的深浅色模式无缝切换**，杜绝页面闪烁。
- 🌐 **双向降级与极速纯浏览器调试**：支持 `bun run dev` 纯网页预览。内置 `runtime.ts` 运行时环境感知与 IPC 自动降级 Mock，**调 UI 无需编译 Rust 后端**；需要联调系统底层能力时再执行 `bun run tauri dev`。
- 🦀 **健壮解耦的 Rust 后端架构**：
  - `lib.rs`（应用状态与插件装配）与 `main.rs`（系统入口/隐藏控制台）规范分离，便于单元测试与集成测试复用；
  - 领域化命令拆分（`src-tauri/src/commands/`），命令层遵循**薄代理**与边界输入校验；
  - 基于 `thiserror` 的强类型全局统一错误枚举 `AppError`，序列化直出可读中文，前端捕获即可直接展示；
  - 结构化日志集成（`tauri-plugin-log` + `log` 门面宏），Debug 细粒度追踪，Release 静默安全。
- 🧩 **按需零运行时图标**：集成 `unplugin-icons` 与 Lucide 图标集，编译期按需提取并内联为纯 SVG 组件，零网络开销、尺寸随 `size-*` 缩放、颜色随 `currentColor` 自适应。
- 🛡️ **严格的多层代码质量门禁**：
  - 前端：Prettier 格式检查（`--check` 已进 CI，含 Tailwind 类名自动排序插件）+ Oxlint（毫秒级极速静态分析）+ `vue-tsc -b` 全量类型检查 + Vitest 单元测试；
  - 后端：`cargo fmt` + `cargo clippy --all-targets -- -D warnings`（零警告容忍）+ `cargo test`。
- 🚀 **一条命令自动化发版 (`scripts/release.ts`)**：内置发版助手，自动执行工作区洁净度检查、分支一致性校验、tag 防重、三处版本号同步（`package.json` / `Cargo.toml` / `tauri.conf.json5`）并自动刷新 `Cargo.lock`。
- 🤖 **工业级 GitHub Actions 流水线**：
  - **CI 门禁 (`ci.yml`)**：PR 与 Push 自动触发，前端全量验证与 Rust 双平台（Windows + Ubuntu 24.04）并行编译检查；
  - **多平台矩阵打包 (`release.yml`)**：推送 tag 自动触发跨平台矩阵构建，并行产出 **Windows** (`.msi` / `-setup.exe`)、**macOS Apple Silicon** (`.dmg`)、**macOS Intel** (`.dmg` 交叉编译)、**Linux** (`.deb` / `.rpm` / `.AppImage`)，一次性原子创建附带全套安装包的 GitHub Release。

---

## 🧰 技术栈概览

| 模块 | 技术选型 | 版本 / 说明 |
|---|---|---|
| **桌面运行时** | [Tauri 2](https://v2.tauri.app/) | 现代化轻量跨平台运行时，内存占用低、体积小 |
| **系统后端** | [Rust](https://www.rust-lang.org/) | Edition 2024 (MSRV 1.85)，`thiserror` 统一错误，`tauri-plugin-log` 日志 |
| **前端框架** | [Vue 3](https://vuejs.org/) | Composition API + `<script setup>` + TypeScript |
| **样式与主题** | [Tailwind CSS v4](https://tailwindcss.com/) | `@tailwindcss/vite`，三层设计令牌 + `light-dark()` 自动主题 |
| **构建与打包** | [Vite](https://vite.dev/) + [vue-tsc](https://github.com/vuejs/language-tools) | 极速热重载 + 全量类型门禁构建 |
| **图标方案** | [unplugin-icons](https://github.com/unplugin/unplugin-icons) + Lucide | 编译期按需内联 SVG 组件 (`~icons/lucide/*`) |
| **测试框架** | [Vitest](https://vitest.dev/) + `cargo test` | 前端组件/纯逻辑单测 + Rust 领域命令单测 |
| **代码质量** | [Oxlint](https://oxc.rs/) + Prettier + Clippy | 毫秒级 Lint，样式属性自动重排，Rust 静态诊断 |
| **包管理器** | [Bun](https://bun.sh/) | 锁定 `bun.lock`，极速脚本执行与依赖安装 |
| **持续集成** | [GitHub Actions](https://github.com/features/actions) | 全自动多系统 CI 门禁 + 四平台矩阵打包发版 |

---

## 🚀 快速开始

### 前置准备

在本地运行或构建前，请确保安装以下基础环境：

1. **[Bun](https://bun.sh/)**：推荐使用 Bun 进行依赖管理与脚本执行。
2. **[Rust](https://rustup.rs/)**：进入本仓库时，`rust-toolchain.toml` 会自动切换至 stable（>= 1.85）并补齐 `rustfmt` 与 `clippy`。
3. **系统依赖**：请参阅 Tauri 官方文档 [Prerequisites](https://v2.tauri.app/start/prerequisites/) 配置目标系统的 C++ 编译工具链与 WebView2 (Windows) / WebKitGTK (Linux) 依赖。

### 常用运行命令

```bash
# 1. 安装项目依赖（使用 bun.lock 锁定依赖版本）
bun install

# 2. 启动桌面端完整调试（首次启动会自动编译 Rust 后端，请耐心等待）
bun run tauri dev

# 3. 仅浏览器预览前端（无需编译 Rust，IPC 自动 mock 降级，极速调试 UI 与样式）
bun run dev
```

> [!TIP]
> **关于双向降级**：通过 `bun run dev` 启动纯浏览器模式时，页面会调用 `src/lib/runtime.ts` 感知到不在 WebView 中，`src/lib/api.ts` 会自动返回带有 `(浏览器预览)` 标识的模拟数据。这允许前端工程师在没有安装 Rust 环境的设备上快速完成界面开发。

---

## 📋 基于模板新建项目

本仓库是标准的 **GitHub Template**。请按照以下步骤将其转化为你的专属生产项目：

### 第一步：创建新仓库

点击本仓库右上角的 **`Use this template`** 按钮（或选择 **`Create a new repository`**），填写你的新仓库名称并克隆到本地。

### 第二步：新项目必改清单

为防止占位符遗留影响打包与上线，请依次替换以下文件中的标识（代码中均包含 `【新项目必改】` 提示）：

| 目标文件 | 需替换字段 | 示例 / 说明 |
|---|---|---|
| `package.json` | `name` | 改为你的项目小写连字符名称，如 `my-app` |
| `src-tauri/Cargo.toml` | `[package]` 段的 `name`、`description`、`authors` | 如 `name = "my-app"`，`authors = ["Your Name <you@example.com>"]` |
| `src-tauri/Cargo.toml` | `[lib]` 段的 `name` | crate 标识符（下划线风格），如 `my_app_lib` |
| `src-tauri/src/main.rs` | 引入的 lib crate 名称 | 与上一步保持一致，如 `my_app_lib::run()` |
| `src-tauri/tauri.conf.json5` | `productName` | 应用展示名称，如 `"My App"` |
| `src-tauri/tauri.conf.json5` | `identifier` | **必须修改**！应用唯一标识（反向域名），如 `"com.company.myapp"`，切勿留 `com.example` |
| `src-tauri/tauri.conf.json5` | `app.windows[0].title` | 窗口默认标题，如 `"My App"` |
| `index.html` | `<title>` | 浏览器标签页标题 |
| `src-tauri/icons/` | 应用全套图标 | 见下方一键生成图标说明 |

### 第三步：生成应用图标

准备一张 `1024x1024` 分辨率的 PNG 图标（例如 `app-icon.png`），运行 Tauri 内置工具即可自动生成全平台图标集：

```bash
bun run tauri icon path/to/app-icon.png
```

生成的 `32x32.png` 同时可复制到 `public/favicon.png` 作为网页预览图标。

### 第四步：刷新锁文件并提交

修改 package 和 crate 名称后，必须同步更新 lockfile：

```bash
# 刷新 bun.lock 与 Cargo.lock
bun install && (cd src-tauri && cargo update --workspace --offline)

# 验证代码检查与构建
bun run format && bun run lint && bun run build

# 提交初始化代码
git add -A
git commit -m "chore: initialize project from tauri-vue-starter template"
```

---

## 📂 项目结构

```text
.
├── .github/
│   └── workflows/
│       ├── ci.yml              # 持续集成：前端 Lint/测试/构建 + Rust 双系统门禁
│       └── release.yml         # 自动化发版：tag 触发 Windows/macOS/Linux 四平台矩阵打包
├── .vscode/                    # 编辑器推荐扩展与工作区配置 (Volar, Tailwind, rust-analyzer)
├── public/                     # 静态资源 (favicon 等)
├── scripts/
│   ├── release.ts              # 一键发版脚本：安全检查 → 同步版本 → commit → tag → push
│   └── version.ts              # 跨文件版本号读写、一致性校验与 Cargo.lock 刷新逻辑
├── src/                        # 前端应用源码 (Vue 3 + TS)
│   ├── components/             # 业务组件库 (HelloWorld.vue 等)
│   ├── lib/
│   │   ├── api.ts              # 统一 IPC 调用入口（附带错误处理与降级响应）
│   │   └── runtime.ts          # 运行时环境探测（判断是否处于 Tauri WebView）
│   ├── App.vue                 # 根组件（承担系统深浅色与全局布局策略）
│   ├── index.css               # Tailwind CSS v4 样式入口与三层设计令牌配置
│   ├── main.ts                 # 前端应用挂载入口
│   └── vite-env.d.ts           # Vite 环境变量与 unplugin-icons 类型声明
├── src-tauri/                  # Rust 桌面端源码
│   ├── capabilities/           # Tauri 2 窗口与插件能力权限配置 (default.json)
│   ├── icons/                  # 多平台应用图标资源
│   ├── src/
│   │   ├── commands/           # 按领域模块划分的 Tauri 命令实现 (greet.rs 等)
│   │   ├── commands.rs         # 命令模块索引
│   │   ├── error.rs            # 全局统一 AppError 枚举与面向前端的用户友好中文序列化
│   │   ├── lib.rs              # 运行时装配：Builder 初始化、插件注册、setup 与命令挂载
│   │   └── main.rs             # 可执行文件入口：静默启动、控制台隐藏与 lib::run 调用
│   ├── build.rs                # Tauri 构建脚本
│   ├── Cargo.toml              # Rust 项目清单与依赖管理
│   └── tauri.conf.json5        # Tauri 2 运行时配置文件（支持丰富注释的 JSON5）
├── rust-toolchain.toml         # 锁定 Rust 编译工具链版本与组件
├── .oxlintrc.json              # Oxlint 静态分析规则配置
├── .prettierrc                 # Prettier 格式化配置（含 Tailwind 属性重排）
├── package.json                # 项目依赖清单与 NPM 运行指令
├── tsconfig.json               # TypeScript 复合工程配置
├── vite.config.ts              # Vite 构建与开发服务器配置
└── vitest.config.ts            # Vitest 独立测试环境配置
```

---

## 📐 工程规范与架构设计

### 1. 前端通信分层 (IPC Architecture)

- **禁止组件直接调用 `invoke`**：所有前后端 IPC 通信必须收敛在 `src/lib/api.ts` 中。
- **运行时环境降级**：`api.ts` 借助 `runtime.ts` 的 `isTauriRuntime()` 检测是否存在 `__TAURI_INTERNALS__`。在浏览器开发环境中自动走 Mock 分支，保证页面可用，防止调用崩溃。
- **类型一致性**：可失败的 Rust 命令返回 `Result<T, AppError>`，前端捕获的 `error` 即为格式化好的中文字符串，直接绑定在视图提示中即可。

### 2. Tailwind CSS v4 三层设计令牌

项目使用全新的 Tailwind CSS v4 配置，在 `src/index.css` 实现了优雅的三层令牌抽象：

1. **原始层 (`:root`)**：全项目唯一允许出现具体色值和尺寸的地方。颜色使用 `light-dark()` 绑定系统的深浅色模式。
2. **语义层 (`@theme inline`)**：将原始变量映射为语义变量（如 `--color-background`、`--color-foreground`、`--color-accent`）。
3. **消费层 (Vue 组件)**：组件直接书写工具类（如 `bg-background`、`text-muted-foreground`、`bg-accent`），换肤或调节品牌色只需修改 `:root`。

### 3. Rust 后端薄命令与错误分层

- **薄命令 (Thin Commands)**：`src-tauri/src/commands/` 下的命令只负责不可信参数校验与服务转发，业务逻辑解耦到独立函数或内部 crate。
- **统一错误 (`AppError`)**：使用 `thiserror` 派生标准错误枚举，并统一实现 `serde::Serialize` 输出字符串。前端无需理解 Rust 底层错误码，直接消费展示文本。
- **日志门面**：统一使用 `log::info!`、`log::debug!` 宏，严禁使用 `println!` 打印调试信息。开发构建自动开启 Debug 级输出，Release 构建自动收缩至 Info 级。

---

## 🛠️ 常用开发指令速查

| 指令 | 作用 | 适用场景 |
|---|---|---|
| `bun run dev` | 启动浏览器预览前端 | 界面、组件、样式极速微调（免编译 Rust） |
| `bun run tauri dev` | 启动桌面端完整调试 | 联调原生能力、窗口、IPC、插件 |
| `bun run build` | 全量类型检查 + 前端产物打包 | 本地构建测试或 CI 门禁检查 |
| `bun run format` | 执行 Prettier 格式化 | 格式化 TS/Vue/CSS，自动对齐 Tailwind 类名 |
| `bun run format:check` | 仅检查 Prettier 格式不改文件 | 与 CI 门禁一致，提交前确认无格式漂移 |
| `bun run lint` | 运行 Oxlint 静态诊断 | 秒级检测代码潜藏隐患与模块循环引用 |
| `bun run test` | 执行 Vitest 单元测试 | 验证前端纯逻辑与工具函数 |
| `bun run version:check` | 检查三处版本号一致性 | 检查 `package.json`、`Cargo.toml`、`tauri.conf.json5` |
| `bun run release <版本>` | 一键触发版本发布流程 | 自动校验、同步版本号、打 Tag 并推送 |

---

## 📦 自动化发版与 CI/CD

本模板内置了开箱即用的自动化版本管理与全平台持续部署（CI/CD）方案。

### 一键发版工作流

无需手动修改多处版本号并手打 tag，只需执行一条指令：

```bash
# 方式一：直接指定版本号
bun run release 0.2.0

# 方式二：按 semver 规则自动递增 (patch / minor / major)
bun run release patch

# 方式三：预发布版本（Release 会自动打上 prerelease 标记）
bun run release 0.2.0-beta.1
```

发版脚本 (`scripts/release.ts`) 会自动执行严密的安全检查：
1. **工作区洁净度检查**：确保没有未提交的改动或未跟踪的文件；
2. **分支与远程同步检查**：确保位于 `main` 分支，且本地已完全同步远端 `origin/main`；
3. **Tag 防重检查**：验证本地与 GitHub 远端均不存在相同的版本 Tag；
4. **同步版本号**：原子化同步 `package.json`、`src-tauri/Cargo.toml` 和 `src-tauri/tauri.conf.json5`（保留 JSON5 注释），并离线刷新 `Cargo.lock`；
5. **Git Commit & Tag & Push**：生成格式化的发布提交 (`chore(release): vX.Y.Z`)，创建附注 Tag 并推送到 GitHub 远端。

> [!NOTE]
> 支持通过 `--dry-run` 预览发布计划而不真正修改文件，或通过 `--no-push` 仅在本地完成 commit + tag。

### 自动化跨平台打包矩阵 (`release.yml`)

当 `v*` tag 被推送到 GitHub 后，GitHub Actions 会自动接管跨平台流水线：

```text
[ Tag v* ] ──> 1. verify (校验版本号一致性 + 前端 Lint/测试)
                   │
                   └──> 2. build (四平台矩阵并行编译打包)
                            ├── Windows x64        (.msi, -setup.exe)
                            ├── macOS aarch64      (.dmg, Apple Silicon)
                            ├── macOS x64          (.dmg, Intel 交叉编译)
                            └── Linux x64          (.deb, .rpm, .AppImage)
                                     │
                                     └──> 3. publish (所有平台就绪后，一次性创建 GitHub Release)
```

- **原子化发布保障**：若任意一个平台的构建发生失败，`publish` 步骤将不会触发，绝不留下缺斤少两的半成品 Release。
- **发布产物汇总**：

| 平台 | 安装包格式 | 架构 / 构建基准 |
|---|---|---|
| **Windows** | `.msi`, `-setup.exe` (NSIS) | x64 (基于 `windows-latest`) |
| **macOS** | `.dmg` | Apple Silicon (`aarch64-apple-darwin`) |
| **macOS** | `.dmg` | Intel (`x86_64-apple-darwin`，ARM Runner 交叉编译) |
| **Linux** | `.deb`, `.rpm`, `.AppImage` | x64 (基于 `ubuntu-24.04`) |

> [!IMPORTANT]
> **代码签名与发布安全**：  
> 默认生成的安装包未配置商业签名证书。Windows 首次运行会弹出 SmartScreen 拦截，macOS 需在「系统设置 → 隐私与安全性」中手动放行。正式商业发布前，请参阅 Tauri 官方文档将代码签名凭据配置到 GitHub Secrets 中。

---

## 📄 开源许可证

本项目基于 [MIT License](https://opensource.org/licenses/MIT) 开源，欢迎自由复用、定制与二次分发。
