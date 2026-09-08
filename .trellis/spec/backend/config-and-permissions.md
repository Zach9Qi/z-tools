# 配置、权限与依赖

> 涉及 `tauri.conf.json5`、`capabilities/`、`Cargo.toml`、日志与平台差异。改这些文件前先读。

---

## 1. `tauri.conf.json5`

- 用 JSON5 是为了**每个配置项都能带中文注释**(`Cargo.toml` 里 `config-json5` feature 的注释);新增任何配置项必须在其上方写一行说明。
- `version` 只能由 `bun run release` 修改(三处同步,见 `../guides/project-conventions.md`);`.prettierignore` 排除了该文件,因为 Prettier 会去掉键引号导致版本脚本正则失配。
- `【新项目必改】` 标记的项(此文件的 `identifier`,`Cargo.toml` 的 `authors`)是模板占位;`productName`、窗口 `title` 等其余占位见 README「新项目必改清单」。
- 窗口 `label` 是代码与 capabilities 的关联键;新增窗口时同时新增或更新 capability 文件。
- `security.csp` 当前为 `null`(开发方便);上线前收紧为官方基线 `default-src 'self'; connect-src ipc: http://ipc.localhost`,并把这条注释留在原处。
- `build.devUrl` 端口 1420 与 `vite.config.ts` 的 `strictPort` 一致,改一处必须改另一处。

## 2. capabilities(权限最小化)

- 当前只有 `capabilities/default.json`:`windows: ["main"]`,`permissions: ["core:default"]`。
- 新增插件时:先加 `<plugin>:default`,再按需追加具体 `allow-*`;**不要** `fs: "**"` / `http: "http://**"` 全放;fs scope 限定到 `$APPDATA` 等具体目录。
- 权限按**窗口**与**平台**拆文件:`default.json`(所有窗口通用)、`desktop.json`(`platforms: ["macOS","windows","linux"]`)、`<label>.json`(某个窗口专属)。当前单文件起步,出现平台专属权限或新窗口时再拆。
- `windows` 字段写明确 label,不用 `"*"`。
- 每个 capability 文件的 `description` 用中文写清「给谁、为什么」。

## 3. `Cargo.toml`

- 非通用依赖上方一行中文注释说明用途与选型原因(现有:`thiserror` 统一错误、`log` + `tauri-plugin-log` 日志门面、`config-json5`);`serde` / `serde_json` 这类人人都懂的基础依赖可省略。
- `edition = "2024"`,`rust-version = "1.85"`;提升 MSRV 要同步 README 与 `Cargo.toml` 里 `rust-version` 的注释。
- 平台专属依赖放 `[target.'cfg(target_os = "…")'.dependencies]`,不要无条件引入 windows / cocoa crate。
- `[lib] name` 保留 `_lib` 后缀与三种 `crate-type`(`Cargo.toml` 注释解释 Windows 冲突与用途)。
- `tokio` 若引入,精确列 features。
- 尚未配置、需要时另开任务讨论:
  - `[profile.release]`:一种取向是 `panic = "abort"`、`codegen-units = 1`、`lto = true`、`strip = true`(体积小、启动快);另一种是保留 unwind 与符号方便崩溃分析。两种都有理由,按发布需求选。
  - `[lints.clippy]` 与 `clippy.toml`:可用 `await_holding_lock` / `unwrap_used` / `unused_async` 等 lint 机制化本规范中的禁止项;本仓库目前靠 `-D warnings` + 评审。

## 4. 日志

- 业务代码只用 `log::{error, warn, info, debug, trace}!` 宏,**禁止 `println!` / `eprintln!` / `dbg!`**(README 与 `Cargo.toml` 注释)。
- 级别在 `lib.rs` 由 `cfg!(debug_assertions)` 决定:debug 构建 `Debug`,发布 `Info`。
- 日志文案中文,带上下文(哪个命令 / 哪个文件),不带敏感信息(token、完整路径中的用户名)。
- 当前 `tauri-plugin-log` 使用默认 target(stdout + LogDir);Windows release 下 stdout 可能阻塞,若发布后出现卡顿,把 release 目标改为仅 `LogDir` 并保留该注释。
- 第三方 crate 噪音用 `.level_for("tauri", LevelFilter::Warn)` 压低,不要整体降级。

## 5. 平台差异

- 应用侧用 `#[cfg(target_os = "windows" | "macos" | "linux")]`;`#[cfg(desktop)]` / `#[cfg(mobile)]` 只用于 Builder 装配(`lib.rs` 注释的 `setup_desktop` 建议)与 `mobile_entry_point`。
- 平台实现拆文件 `src/<domain>/{windows,macos,linux}.rs`,在 `<domain>.rs` 里 `#[cfg(target_os = "windows")] mod windows;` 并暴露统一函数签名。
- 不支持的平台要么编译期排除,要么返回 `AppError` 明确文案,不静默 no-op。
- `main.rs` 的 `windows_subsystem = "windows"` 属性不可删(否则 Windows 发布版弹控制台)。

## 6. 构建

- `build.rs` 只有 `tauri_build::build()`。
- CI 在跑 Rust 门禁前 `mkdir -p ../dist` 满足 `frontendDist` 检查(`ci.yml` 注释);本地 `cargo test` 若报 dist 不存在,同样处理,不要改 `frontendDist`。

## 7. 禁止

- 无注释的配置项 / 非通用依赖。
- 手改三处版本号中的任何一处。
- capabilities 里 `windows: ["*"]`、通配 scope。
- `println!` 系列。
- 在通用代码里内联大段 `#[cfg(target_os)]` 分支。
