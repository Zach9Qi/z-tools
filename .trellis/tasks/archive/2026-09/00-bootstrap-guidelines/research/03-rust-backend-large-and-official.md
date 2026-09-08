# Tauri 2 Rust 后端编码约定调研报告

**样本**:clash-verge-rev(CVR)、EcoPaste(ECO)、hoppscotch-desktop(HOP)、官方 plugins-workspace(PW:fs/store/dialog/log/shell)、create-tauri-app 脚手架(CTA)。
路径前缀:`CVR=clash-verge-rev/src-tauri/src`,`ECO=EcoPaste/src-tauri/src`,`HOP=hoppscotch/packages/hoppscotch-desktop/src-tauri/src`,`PW=plugins-workspace/plugins`,`CTA=create-tauri-app/templates/_base_/src-tauri`。

---

## 1. 模块分层

- **main.rs 极薄,lib.rs 为组合根**:5/5。`main.rs` 只做 `windows_subsystem` + 调 `lib::run()`(CTA `src/main.rs.lte`;ECO `main.rs`;HOP `main.rs` 额外先初始化 tracing;CVR `main.rs` 额外建 tokio runtime 并 `tauri::async_runtime::set`)。`lib.rs` 注册插件/命令/setup/run 事件循环。
- **命令按领域拆文件 + 顶层 mod glob re-export**:CVR `cmd/mod.rs`(`pub use app::*; ...`)、ECO `commands/mod.rs`(注释明确说明 glob 是因 `__cmd__*` 隐藏项需要)。HOP 规模小,命令直接散在 `updater.rs`/`path.rs` 等领域文件里。官方插件统一叫 `commands.rs`(PW `dialog/src/commands.rs`, `fs/src/commands.rs`)。
- **`mod.rs` vs `xxx.rs + xxx/`**:CVR、ECO 都用 `mod.rs`(`CVR/core/mod.rs`, `ECO/clipboard/mod.rs`);官方插件是扁平单文件,无子目录(shell 的 `process/mod.rs` 也用 mod.rs)。→ **mod.rs 风格是多数**。
- **分层名称与边界**:
  - CVR:`cmd/`(IPC 层)→ `feat/`(用例编排,如 `feat/clash.rs` 的 `restart_clash_core`)→ `core/`(长生命周期基础设施:`handle`, `tray`, `logger`, `manager`, `service`)+ `config/`(持久化模型)+ `enhance/`(纯领域逻辑)+ `utils/` + `process/`(AsyncHandler)+ `constants.rs`。
  - ECO:`commands/` → 领域模块(`clipboard/`, `settings/`, `db/`, `window/`)+ `core/`(error/paths 等横切)。其 `.trellis/spec/backend/architecture.md` 明确:"Keep commands thin but not empty… Business rules should live below commands/"。
  - HOP:扁平领域文件,无分层(仅 1 个项目,小型)。
- **结论**:「命令层 → 用例/领域层 → 基础设施」三段式是 2 个大项目的共识(CVR、ECO),命名不统一(`feat` vs 领域名)。

## 2. 命令写法

- **返回类型**:分歧。
  - 官方插件:`Result<T>` = `std::result::Result<T, Error>` 别名(PW `dialog/src/error.rs`, `store/src/error.rs`, `shell/src/lib.rs` `type Result<T>`);fs 额外有 `CommandResult<T>`/`CommandError`(`fs/src/commands.rs:28-72`)。
  - CVR:`pub type CmdResult<T = ()> = Result<T, CommandFailure>`(`cmd/mod.rs:5`),结构体 `{code?, detail, operation?}`。
  - ECO:`crate::core::Result<T>` = `Result<T, AppError>`(`core/error.rs`)。
  - HOP:混用 `Result<T, String>`(`updater.rs:71`, `logger.rs:59`)与 `HoppError`。CTA:直接返回 `String`。
- **async 比例**:CVR、ECO 几乎全部 `pub async fn`(即使内部同步,如 ECO `commands/settings.rs` `get_settings`);官方 fs 同步/异步各半(`fs/src/commands.rs` grep:`create`/`mkdir`/`stat` 同步,`read`/`write`/`copy_file` 异步)。
- **参数**:官方插件用 `State<'_, T>` + `Window<R>`/`Webview<R>`,泛型 `<R: Runtime>`(PW `dialog/src/commands.rs:118`);ECO 取 `AppHandle` 再 `app.state::<T>()`(`commands/settings.rs`);CVR 全局单例,命令几乎无 Tauri 参数(`cmd/verge.rs`)。
- **命令薄**:CVR 是(`cmd/verge.rs` 一行转发 `feat::`);ECO "薄但非空"(`commands/settings.rs::update_settings` 含副作用编排);官方 fs 命令较厚(含 scope 校验)。
- **`generate_handler!` 位置**:全部在 `lib.rs` 的 `Builder::invoke_handler`(CVR 抽成 `app_init::generate_handlers()` 函数,`lib.rs:117`)。
- **命令名↔前端**:ECO 要求同步 `src/constants/commands.ts` 并在 `src/commands/index.ts` 包一层(`.trellis/spec/backend/commands-and-events.md`)。CVR/HOP 未见显式约定。

## 3. 错误处理

- **官方口径(PW 4/4 插件)**:`thiserror` 枚举 `Error`,`#[non_exhaustive]`,`#[error(transparent)] X(#[from] x::Error)`,手写 `impl Serialize` → `serialize_str(self.to_string())`,`pub type Result<T> = std::result::Result<T, Error>`(`dialog/src/error.rs`, `fs/src/error.rs`, `store/src/error.rs`, `shell/src/error.rs`)。fs 的 `CommandError` 还包 `Anyhow(#[from] anyhow::Error)` 并用 `{err:#}` 展开链(`fs/src/commands.rs:60-68`)。
- **thiserror 定义边界 + anyhow 内部传播**:ECO(`AppError::Other(#[from] anyhow::Error)` + `Context` 使用,`window/state.rs:31`)、fs 插件、CVR(内部全 `anyhow::Result`,边界 `StringifyErr`/`WithErrorCode` trait 转换,`cmd/mod.rs:150-180`)。→ 3 个项目。
- **Serialize 形态**:官方=纯字符串;ECO=`{kind, message}`(`core/error.rs`);CVR=`{code?, detail, operation?}` 且有测试锁定 wire contract(`cmd/mod.rs:184`)。HOP 无 Serialize impl,命令层直接 `map_err(|e| e.to_string())`。
- **宏**:CVR 没有 `wrap_err!`,用 trait 扩展方法;`logging!`/`logging_error!` 宏来自 `crates/clash-verge-logging`。其他项目无。

## 4. 状态管理

- **`app.manage()`**:官方插件 `app.manage(dialog)`(`dialog/src/lib.rs:203`)、`app.manage(RwLock<Platform>)`(CVR 的 sysinfo 插件);ECO manage `WindowStateStore`/`WindowLifecycleManager`/`DatabaseState`(`lib.rs:190-213`)。CVR 主 crate **几乎不用 manage**,改用全局单例。
- **锁选型**:分歧。
  - std::sync::Mutex/RwLock:官方 store(`Arc<RwLock<HashMap>>`, `store/src/lib.rs:41`)、shell(`Arc<Mutex<HashMap>>`)、ECO 多数(`window/state.rs`, `clipboard/app_store.rs`,并处理 poisoned)。
  - tokio::sync::Mutex:ECO `db/state.rs`(跨 await 持有 pool)、HOP `updater.rs:57`、CVR `config/config.rs:23`。
  - parking_lot:仅 CVR(`core/notification.rs:3`, `core/logger.rs:17`,并 `mutex_atomic`/`await_holding_lock = deny`)。
- **全局 static**:ECO 用 `std::sync::LazyLock`/`OnceLock`(`backup/mod.rs:55`, `clipboard/detect.rs:14`);HOP `LazyLock`+`OnceLock`(`updater.rs:56`, `lib.rs:22`);CVR `once_cell::Lazy` + `std::sync::OnceLock`(`singleton!` 宏,`utils/singleton.rs`)。→ **std LazyLock/OnceLock 为多数,once_cell 仅 CVR(历史包袱)**。
- **AppHandle 全局持有**:CVR `pub static APP_HANDLE: OnceCell<AppHandle>`(`lib.rs:27`)+ `Handle::app_handle()`;ECO 仅在 Windows 钩子里 `static APP_HANDLE: OnceLock`(`keyboard/windows.rs:21`),其余传参。官方插件绝不全局持有,存进 managed state(`desktop::Dialog(AppHandle)`)。

## 5. 事件

- **emit 封装**:CVR 集中 `NotificationSystem::send_event`(`core/notification.rs:285`),`FrontendEvent` enum → `match` 得 `(&'static str, Value)`;并强制 `run_on_main_thread` 派发(注释:macOS WebKit 死锁)。ECO 每处 `app.emit(CONST, &payload)` + `log::warn` 兜底(`commands/settings.rs:113`)。HOP 裸 `app.emit("updater-event", UpdateEvent::…)`。
- **事件名管理**:ECO `const X_EVENT: &str = "domain://action"`(`settings://updated`, `clipboard://updated`)靠近 emit 点,spec 要求镜像到 `src/constants/events.ts`;CVR 字面量集中在 `serialize_event` 一处,前缀 `verge://`。→ **`domain://action` 命名 2 个项目**。
- **payload derive**:`#[derive(Debug, Clone, Serialize)] #[serde(rename_all = "camelCase")]`(HOP `updater.rs:9`, CVR `PendingFailure`, 官方 store `ChangePayload<'a>` 借用字段避免 clone)。HOP `UpdateEvent` 用 `#[serde(tag = "type")]` tagged enum;官方 shell `#[serde(tag = "event", content = "payload")]`。
- **Channel<T>**:官方 fs watcher(`fs/src/watcher.rs:44`)、shell spawn(`shell/src/commands.rs:242`)用于流式回传;三个应用均未用。

## 6. 异步

- **tokio features**:CVR 精确列 `rt-multi-thread, macros, time, sync`(workspace Cargo.toml);ECO 仅 `time`(dev 加 `macros, rt`);HOP 无 feature。→ **不依赖 `full`**。
- **spawn**:统一走 `tauri::async_runtime::spawn/spawn_blocking`(ECO `commands/clipboard.rs:143` 缩略图解码;HOP `lib.rs:93`;CVR 包成 `AsyncHandler`,`process/async_handler.rs`)。CPU/IO 阻塞进 `spawn_blocking` 3/3。
- **block_on**:仅在 `setup()` 内(ECO `lib.rs:205`, HOP `lib.rs:210`)与 CVR 启动/退出事件。命令内未见。

## 7. 日志

- 分歧:ECO `tauri-plugin-log` + `log`(LogDir 文件;Stdout/Webview 仅 debug,`lib.rs:28-48`,注释解释 Windows release stdout 阻塞);HOP `tracing` + `tracing-appender` + `file-rotate`(5 个 10MB,`logger.rs`);CVR `flexi_logger` + `log` + 自定义 `logging!(level, Type::X, …)` 带 target 枚举,可配置大小/数量滚动(`core/logger.rs`)。官方 log 插件基于 `fern`,默认 `Stdout + LogDir`,40KB 轮转(`log/src/lib.rs:51-57`)。
- **共识**:3/3 落文件 + 滚动;级别 debug/release 区分。

## 8. 序列化

- `rename_all = "camelCase"`:ECO 全面(`settings/model.rs` 配合 `#[serde(default)]` 保证向前兼容,模块注释说明);HOP 全面;官方插件 IPC 结构体全面(`dialog/src/commands.rs:37`, `store/src/lib.rs:31`)。CVR **例外**:配置结构体 snake_case 直传(`config/verge.rs`),仅 yaml 相关 `kebab-case`。→ 4/5。
- `#[serde(default)]` 在 Deserialize 入参上普遍;`skip_serializing_if = "Option::is_none"` 见 CVR `CommandFailure`、`IVerge`。
- **TS 类型生成**:5 个样本**均未用** ts-rs/tauri-specta;官方仅 haptics/geolocation 有 optional `specta` feature。→ 手写 TS 镜像是现状。

## 9. 插件与权限

- **capabilities 拆分**:按平台/用途分文件——CVR `desktop.json`/`desktop-windows.json`/`migrated.json`;ECO `default.json`(注释"平台特定放独立文件")+ `macos-permissions.json`;HOP `default.json`+`desktop.json`;CTA 单 `default.json`。
- **最小化**:ECO/HOP/CTA 使用 `xxx:default` + 个别 `allow-*`,HOP fs 精确到命令 + `fs:scope` 限 `$APPCONFIG/$APPDATA`。CVR `desktop.json` 有重复项(`updater:default` 两次)且 `http` 放开 `https://*/*`——不算最小。
- **官方插件目录结构**(PW dialog 为完整范本):`lib.rs`(`//!` crate 文档、`init()` Builder、`XxxExt` trait、`pub use error::{Error, Result}`)、`commands.rs`(`#[command] pub(crate) async fn` + IPC 入参 struct)、`error.rs`、`models.rs`(共享 Serialize 类型)、`desktop.rs`/`mobile.rs`(同名 `init()` 与 `Dialog<R>`,`#[cfg(desktop)] use desktop::*`)、`build.rs` 用 `tauri_plugin::Builder::new(COMMANDS)` 生成 `permissions/autogenerated/`。CVR 自定义插件 `crates/tauri-plugin-clash-verge-sysinfo` 为简化版(仅 `lib.rs`+`commands.rs`)。

## 10. Cargo.toml

- **profile.release**:CTA/官方 = `panic="abort", codegen-units=1, lto=true, strip=true`(CTA `opt-level=3`;PW `opt-level="s"`)。CVR 反向:`panic="unwind"`(catch_unwind 降级启动依赖)、`lto="thin"`、`debug=1`、`strip="none"`(便于崩溃符号);ECO/HOP 未定义 profile。
- **profile.dev**:仅 CVR(`codegen-units=64`, `incremental`),另有 `fast-release`/`debug-release` 自定义 profile。
- **edition**:CTA/ECO/HOP/PW 2021;CVR 2024 + `rust-version="1.95"`。`rust-toolchain.toml` 固定版本:CVR 1.98、ECO 1.96(含 rustfmt/clippy 组件)。
- **lints**:仅 CVR `[workspace.lints.clippy]`(50+ 条,含 `await_holding_lock/panic/unimplemented/wildcard_imports/unused_async = deny`, `unwrap_used/expect_used = warn`)+ `.clippy.toml`(`cognitive-complexity-threshold=25`)+ `rustfmt.toml`(`max_width=120`)+ `.cargo/config.toml` alias `clippy-all = "clippy --all-targets --all-features -- -D warnings"`。ECO/HOP 无 lints 配置。
- **workspace 拆 crate**:CVR 拆 7 个 `crates/*`(logging/draft/i18n/limiter/signal/plugin);PW 天然 workspace;ECO/HOP 单 crate。

## 11. 测试

- **内联 `#[cfg(test)] mod tests` 是唯一形态**:5/5,无 `tests/` 目录(CVR `lib.rs:521`, `cmd/mod.rs:184`, `core/notification.rs:136`;ECO 40+ 处,如 `clipboard/detect.rs:139`;PW `fs/src/commands.rs:1807`, `shell/src/process/mod.rs:510`)。
- 覆盖层次:纯逻辑/领域层(CVR `enhance/`, `core/runstate`, ECO `clipboard/`)+ 错误序列化契约(CVR)+ 启动决策函数(CVR `handle_singleton_startup`)。命令层本身基本无测试。CVR 有 `criterion` dev-dep。

## 12. 平台差异

- 官方:`#[cfg(desktop)]`/`#[cfg(mobile)]` 切模块 + `desktop.rs`/`mobile.rs` 暴露相同 API(`dialog/src/lib.rs:32-48`;fs 更细:`android.rs`/`ios.rs`)。
- 应用侧:全部用 `#[cfg(target_os = "…")]`,不用 `desktop`/`mobile`(CVR grep 0 命中)。ECO 每个平台模块内 `macos.rs`/`windows.rs` 分文件(`window/`, `keystroke/`, `drag_out/`),`lib.rs` 里 `#[cfg(target_os = "windows")] mod keyboard;`;CVR `utils/linux/`、`core/win_uwp.rs`。Cargo 侧 `[target.'cfg(...)'.dependencies]` 4/4。

## 13. 注释与文档

- 官方插件:`//!` crate 文档 + 每个 pub 项 `///`(含 doctest 示例),英文,SPDX 头。
- ECO:`//!` 模块头 + 大量 `///` 与 `//` 解释**隐藏约束**(NSPanel 时序、stdout 阻塞、catch_unwind 原因),中文;spec 明文:"Do not restate obvious assignments"。
- CVR:`///` 密度高,中英混杂(日志中文、doc 英文),偏向解释 invariant("must not …" 30+ 处)。HOP:英文,较少。

## 14. 反模式 / 踩坑(有代码证据)

1. **emit 死锁**:CVR `core/notification.rs:305` —— "Emitting from a runtime worker can deadlock on macOS when WebKit's protocol handler waits for Tauri's webview lock",解法 `run_on_main_thread`。
2. **setup panic 跨 FFI 边界 abort**:CVR `lib.rs:264-`(`catch_unwind` + 降级启动)、ECO `lib.rs:298`("由 tao 从 ObjC 经 extern C 同步调用,panic 无法 unwind 直接 abort")。
3. **非 Send 句柄跨 await**:ECO spec `architecture.md` "Async and Threading Constraints"——剪贴板句柄在同步块内用完再 await。
4. **Mutex 跨 await**:CVR 以 `clippy::await_holding_lock = "deny"` 机制化;`unwrap_used/expect_used = warn` + `panic = deny`。
5. **Windows release stdout 阻塞**:ECO `lib.rs:28` 注释,生产不输出 Stdout。
6. **大 lib.rs**:CVR `lib.rs` 500+ 行(含 event_handlers 内嵌 mod),ECO `lib.rs` 370 行——两者都把 generate_handler 列表放 lib.rs,属可接受的"组合根膨胀"。
7. **HOP 的 TODO 自证**:`updater.rs:54` "TODO: See if it's possible to let Tauri handle this state management"——全局 `LazyLock<Arc<Mutex>>` 被作者自己标为待改进,佐证 `manage()` 才是正道。
8. CONTRIBUTING.md(CVR)仅有 `cargo clippy-all`/`cargo fmt` 要求,无书面教训;教训全在代码注释里。

---

## 汇总表

| 档位 | 结论 |
|---|---|
| **公认(≥4 或官方明确)** | main.rs 仅调 `lib::run()`,lib.rs 为组合根;`generate_handler!` 在 lib.rs;thiserror 枚举 `Error` + `#[from]` + 手写 `Serialize` + `type Result<T>` 别名;`#[serde(rename_all="camelCase")]` 用于 IPC 结构;`#[serde(default)]` 入参;`tauri::async_runtime::spawn_blocking` 处理阻塞;内联 `#[cfg(test)]`,无 `tests/`;capabilities 按平台/用途拆 JSON,以 `xxx:default` 为基;`[target.'cfg()'.dependencies]` 平台依赖;release `codegen-units=1 + lto`(CTA/官方还含 `panic=abort, strip`);tokio 不用 `full`;不用 ts-rs/specta |
| **多数** | 命令按领域拆到 `cmd|commands/*.rs` 并 glob re-export;三段式(命令→用例→基础设施);`mod.rs` 目录风格;内部 anyhow 传播、边界转 thiserror;全局用 std `LazyLock/OnceLock`;`app.manage()` 托管状态而非全局 AppHandle;事件名 `domain://action` 常量置于 emit 附近;日志落文件+滚动、debug/release 分级;`//!`+`///` 记录隐藏约束;应用侧用 `target_os` 而非 `desktop/mobile`;`rust-toolchain.toml` 固定版本 |
| **分歧/不确定** | 命令错误 wire 形态(纯字符串 vs `{kind,message}` vs `{code,detail}`);锁选型(std / tokio / parking_lot);日志栈(tauri-plugin-log / tracing / flexi_logger);`[lints]`/rustfmt/clippy.toml 仅 CVR 有;edition 2024 仅 CVR;`panic=unwind`+保留符号(CVR)vs `abort`+strip(官方);命令是否全 async;workspace 拆 crate 仅 CVR;全局 `APP_HANDLE` static 仅 CVR 主用 |