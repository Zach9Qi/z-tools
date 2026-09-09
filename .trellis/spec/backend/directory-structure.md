# 后端模块分层(src-tauri/)

> Rust 侧代码如何组织。新建模块前先看这里。

---

## 当前布局(真实存在)

```text
src-tauri/
├── Cargo.toml              # 非通用依赖上方一行中文注释说明用途与选型;tauri feature tray-icon / protocol-asset、
│                           # tauri-plugin-global-shortcut、arboard / sqlx / image / blake3、dev tokio、
│                           # [target.'cfg(windows)'.dependencies] windows(feature 逐条注释用途)
├── tauri.conf.json5        # 每个配置项一行中文注释(config-json5 feature);主窗口是透明无边框启动器形态;assetProtocol scope
├── build.rs                # tauri_build::build(),不放逻辑
├── migrations/
│   └── 0001_clipboard.sql  # sqlx::migrate!("./migrations") 编译期内嵌;已提交的文件不可再改,schema 变更只新增 000N_xxx.sql
├── capabilities/
│   └── default.json        # 主窗口权限:core:default + core:window:allow-set-size(不给 allow-hide,隐藏走命令)
└── src/
    ├── main.rs             # 只有 windows_subsystem 属性 + tauri_vue_starter_lib::run()
    ├── lib.rs              # 组合根:插件注册、setup、generate_handler!、`#[cfg(desktop)] fn setup_desktop` / `fn setup_clipboard`;
    │                       # Builder 用 .build()?.run(|app, event| …) 在 RunEvent::Exit 停监听;不放业务逻辑
    ├── error.rs            # 全局统一 AppError;以 `pub mod error` 导出(见 error-handling.md §2)
    ├── launcher.rs         # 领域层:启动器窗口 show / hide / toggle / 失焦策略、事件常量、anchor_position 纯函数(含测试);
    │                       # `#[cfg(windows)]` PreviousForeground 托管状态 + remember_foreground / previous_foreground / activate_window
    ├── launcher/
    │   └── windows.rs      # 平台钩子:`#[cfg(windows)]` SetWindowSubclass 拦 SC_KEYMENU;current_foreground / is_taskbar / activate
    ├── clipboard.rs        # 领域层(pub mod):常量、ClipboardKind / ClipboardItem / ListQuery DTO、Captured、`pub use store::ClipboardStore`、
    │                       # domain_hash / searchable_text、record()(唯一 emit clipboard://changed 处)、paste() 编排、delete_item
    ├── clipboard/
    │   ├── store.rs        # 存储层(pub mod):ClipboardStore 定义 + 全部 impl(sqlx SQL、行 → DTO 整形、images/ 图片文件读写删);内存库 #[tokio::test]
    │   ├── backend.rs      # 跨平台读写(pub mod,arboard):read_snapshot / write、阈值 / 像素哈希 / 缩略图纯函数(含测试);无 cfg
    │   └── windows.rs      # 平台专属:`#[cfg(windows)]` 消息窗口监听 run_monitor / stop_monitor、SendInput send_paste、ClipboardWatcher 状态
    ├── tray.rs             # 托盘装配:菜单 / 点击事件 → launcher 领域函数
    ├── commands.rs         # 命令层模块根:pub mod <domain>;
    └── commands/
        ├── launcher.rs     # 一个领域一个文件;hide_launcher / get_toggle_shortcut,只转发到 launcher.rs(不可失败命令样板)
        └── clipboard.rs    # 5 个可失败 async 命令:list / get_text / paste / delete / set_favorite;只校验 limit、trim query、转发
```

- lib + bin 拆分的原因写在 `lib.rs` 模块文档里:让命令与错误类型能被单测复用。`[lib] name` 带 `_lib` 后缀是 Windows 上 lib/bin 同名冲突的规避(`Cargo.toml` 注释)。main.rs 保持极薄,lib.rs 作为组合根。
- 目录模块用 **`xxx.rs + xxx/` 风格**(`commands.rs` + `commands/`),不用 `mod.rs`。理由:编辑器里一堆 `mod.rs` 标签无法区分;既然仓库已按 2018 风格起步,不混用。

## 三段式分层(增量扩展)

采用「命令层 → 领域 / 用例层 → 基础设施」三段式。本仓库对应:

| 层 | 目录 | 职责 | 不做 |
|---|---|---|---|
| 命令层 | `src/commands/<domain>.rs` | `#[tauri::command]` 入口:校验不可信输入、取 `State` / `AppHandle`、调用领域层、把结果整形成前端需要的形状、成功后 emit 事件 | 业务规则、直接开文件/数据库连接、重复领域层的过滤逻辑 |
| 领域层 | `src/<domain>.rs`,长大后 `src/<domain>/` | 可被命令、托盘、定时任务、测试共同复用的业务逻辑;纯 Rust,不依赖 Tauri 类型(必要时接收 `AppHandle` 参数) | 处理 IPC 参数格式、拼前端文案以外的展示逻辑 |
| 横切 / 基础设施 | `src/error.rs`;将来 `src/core/`(路径、日志初始化等) | 错误类型、路径解析、通用工具 | 业务 |
| 基础设施:存储 | `src/<domain>/store.rs`(现例 `clipboard/store.rs`),类型 `XxxStore` 的定义与全部 `impl` 都在这里,`<domain>.rs` 只 `pub use` 并在 `lib.rs` `app.manage` | 该领域的全部持久化:SQL、行 → DTO 整形、随行落盘的文件(图片原图 / 缩略图);可用内存库 + 临时目录单测(见 `persistence.md`) | 发事件、碰剪贴板 / 系统 API、决定阻塞 IO 跑在哪个线程(交回领域编排函数) |
| 基础设施:系统适配(跨平台) | `src/<domain>/backend.rs`(现例 `clipboard/backend.rs`,arboard) | 三平台同一 API 的系统能力封装;纯函数部分单测 | 带 `#[cfg]`;持有非 `Send` 句柄跨 await |
| 平台专属 | `src/<domain>/{windows,macos,linux}.rs`,在 `<domain>.rs` 里 `#[cfg(windows)] mod windows;`(现例 `launcher/windows.rs`、`clipboard/windows.rs`) | 只在某个 OS 存在的实现(监听、按键模拟、窗口钩子),暴露统一签名供其他平台照抄 | 在通用代码里散落 `#[cfg]` 分支;把跨平台库能做的事写进平台文件 |

> `cfg(windows)` 与 `cfg(target_os = "windows")` 等价(`windows` / `unix` 是 rustc 内置的目标族简写),本仓库统一用短写法;macOS / Linux 没有族简写,届时写 `cfg(target_os = "macos")` / `cfg(target_os = "linux")`。

**跨平台分工原则**(剪贴板确立,后续领域沿用):跨平台库能覆盖的能力(arboard 读写剪贴板)放通用文件不带 cfg;库不提供、必须调平台 API 的能力(监听变化、模拟按键、前台窗口切换)按平台拆文件并暴露同名函数(`run_monitor` / `stop_monitor` / `send_paste`),父模块用 `#[cfg(windows)] pub use windows::{…};` 转出。新平台只补一份平台文件,数据层、命令层、前端不动。

规则:

- 命令层「薄但不空」:允许校验 + 编排 + 整形,业务规则必须在领域层,这样 `#[cfg(test)]` 可以不依赖 `AppHandle` 测到业务。
- 小领域先写成 `commands/<domain>.rs` 一个文件;业务逻辑一出现就拆到 `src/<domain>.rs`,不要在 `commands/` 里长出几百行。
- `lib.rs` 只增长 `plugin(...)`、`manage(...)`、`generate_handler![...]` 三类行;setup 里的初始化逻辑抽到 `fn setup_xxx(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>>`(经验值:超过十来行就抽;返回 `Box<dyn Error>` 是因为 setup 闭包的签名就是它,`?` 可以直接透传插件 / 托盘的各类错误)。桌面端专属装配已落地为 `#[cfg(desktop)] fn setup_desktop`(快捷键、`setup_clipboard`、窗口事件、平台钩子、托盘、`init_hidden`),新的桌面端初始化追加进去,不在 `setup` 闭包里内联;单个领域的装配(建目录、开库、manage、起后台任务)再抽成自己的 `setup_<domain>`(现例 `setup_clipboard`)。
- 领域模块的可见性默认私有(`mod launcher;`);只在两种情况下 `pub mod`:它是 crate 对外契约(`error`),或它的跨平台部分只被 `#[cfg(windows)]` 代码引用、非 Windows 会被 dead_code 拦下(`clipboard` 及其 `backend` / `store`)。理由写在该模块的 `//!` 文档里(见 `config-and-permissions.md` §6)。
- 领域层的编排函数(`record` / `paste` / `delete_item`)接收 `&ClipboardStore` 与 `&AppHandle` 参数,负责串 store 方法与其它副作用:事件广播、切前台、`spawn_blocking`。store 的 SQL 方法与文件方法分开(`trim` 返回文件名 → `remove_image_files` 删),是为了 SQL 能在内存库里单测而不碰磁盘;文件本身仍归 store 管。一个类型的 struct 与 `impl` 不拆到两个文件——之前 `ClipboardStore` 定义在 `clipboard.rs`、SQL 在 `store.rs`、删文件又回 `clipboard.rs`,读代码要三处跳,已收拢。
- 领域模块之间避免双向依赖:`tray.rs` 依赖 `launcher.rs`(调 show / toggle_from_tray),而失焦回调又要凭托盘 id 查托盘矩形,所以 `TRAY_ID` 常量放在 `launcher.rs` 由 `tray.rs` 引用,而不是反过来。
- 横切模块变多后收拢到 `src/core/`(如 `core/{error,paths}`),此前 `error.rs` 直接放 `src/`。

## 新增命令的固定动作

1. 在 `src/commands/<domain>.rs` 写 `pub fn` / `pub async fn`,标 `#[tauri::command]`,文档注释说明参数与失败情形。
2. 若是新领域文件,在 `src/commands.rs` 加 `pub mod <domain>;`。
3. 在 `lib.rs` 的 `generate_handler![...]` 追加 `commands::<domain>::<name>`(漏注册前端 invoke 直接报错,`lib.rs` 注释)。
4. 前端 `src/lib/api/<domain>.ts` 加同名 camelCase 封装并由 `api/index.ts` 汇出(见 `../guides/ipc-contract.md`)。

## 命名

| 对象 | 规则 | 例子 |
|---|---|---|
| 模块 / 文件 | snake_case,名词 | `commands/launcher.rs`、`launcher.rs`、`tray.rs` |
| 命令函数 | snake_case,动词开头;前端函数名是它的 camelCase | `hide_launcher`、`get_toggle_shortcut`、`update_settings` |
| 状态结构体 | `XxxState` / `XxxStore` / `XxxWatcher`,`app.manage()` 托管 | `ClipboardStore`、`ClipboardWatcher`、`PreviousForeground` |
| 事件常量 | `SCREAMING_SNAKE`,值 `domain://action` | `pub const CLIPBOARD_CHANGED: &str = "clipboard://changed";` |
| 错误变体 | 名词或名词短语 | `AppError::InvalidInput`、`AppError::Database`、`AppError::Unsupported` |
| 迁移文件 | `000N_<名>.sql`,四位序号递增 | `migrations/0001_clipboard.sql` |
| 内部快照 / DTO | 内部类型不派生 serde(`Captured`);IPC 出参 `XxxItem`、入参 `XxxQuery` / `XxxPatch` | `Captured`、`ClipboardItem`、`ListQuery` |

## 禁止

- `main.rs` 里出现除 `run()` 以外的逻辑。
- 在 `lib.rs` 定义业务结构体或命令。
- `commands/` 下的文件互相 `use`(共享逻辑下沉到领域层)。
- 新建 `mod.rs`。
- 平台代码用大段 `#[cfg]` 内联在通用函数里(拆文件)。
- 在 `store.rs` 里删文件 / emit 事件 / 调剪贴板(副作用留给 `<domain>.rs` 的编排函数)。
- 修改已提交的 `migrations/000N_*.sql`。
