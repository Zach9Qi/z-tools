# 后端开发规范(Rust + Tauri 2)

> `src-tauri/` 目录的编码约定。写代码前读「开发前检查清单」,写完对照「质量检查」。
> 全项目通用约定见 `../guides/project-conventions.md`;前后端 IPC 契约见 `../guides/ipc-contract.md`;前端侧见 `../frontend/index.md`。

---

## 规范索引

| 文件 | 内容 | 什么时候读 |
|------|------|-----------|
| [directory-structure.md](./directory-structure.md) | lib / main 职责、`commands.rs + commands/` 风格、三段式分层、新增命令的固定动作、命名 | 新建模块 / 命令之前 |
| [command-guidelines.md](./command-guidelines.md) | `#[tauri::command]` 签名、参数序列化、命令体顺序、注册、测试 | 写或改命令 |
| [error-handling.md](./error-handling.md) | `AppError` 形态、文案规则、何时引入 anyhow / 结构化错误、panic 策略 | 任何返回 `Result` 的代码 |
| [state-events-async.md](./state-events-async.md) | `app.manage` 托管状态与锁选型、事件命名与 payload、异步运行时约束 | 引入共享状态、事件、后台任务 |
| [config-and-permissions.md](./config-and-permissions.md) | `tauri.conf.json5`(含 assetProtocol)、**落盘目录约定**(统一 `app_local_data_dir()`)、capabilities 最小权限、`Cargo.toml`、日志、平台差异(含非 Windows 编译路径)、启动器窗口约定(show / hide 时序、失焦、托盘、快捷键) | 改配置、加插件 / 依赖、写平台代码、改窗口行为、任何要落盘的东西 |
| [persistence.md](./persistence.md) | sqlx + SQLite:依赖 feature、迁移文件不可变、打开失败即启动失败、去重 / keyset 分页 / LIKE 转义写法、内存库测试清单 | 新增表 / 迁移 / 任何 SQL |
| [quality-guidelines.md](./quality-guidelines.md) | fmt / clippy / test 门禁、内联测试、`//!` 与 `///` 文档、反模式速查 | 提交前 |

## 开发前检查清单

1. 读 `directory-structure.md`,确定代码属于命令层 / 领域层 / 横切层,放对目录;不建 `mod.rs`。
2. 写命令 → 读 `command-guidelines.md`:`Result<T, AppError>`、无 await 不 async、参数 camelCase 自动映射、命令体「校验 → 取状态 → 调领域层 → 整形 → emit」。
3. 新错误 → 读 `error-handling.md`:加变体 + `///` + 中文文案 + `to_string()` 测试。
4. 共享状态 / 事件 / 后台任务 → 读 `state-events-async.md`。
5. 加插件 / 依赖 / 改配置 → 读 `config-and-permissions.md`,权限最小化,配置项与依赖带中文注释。
6. 写 SQL / 迁移 → 读 `persistence.md`:运行时 `sqlx::query`、迁移文件只增不改、keyset 游标、内存库测试。
7. 参照 `src-tauri/src/commands/launcher.rs`(不可失败薄命令)、`commands/clipboard.rs`(可失败 async 命令 + `State`)、`launcher.rs`(领域层 + 纯函数测试)、`clipboard.rs` + `clipboard/store.rs`(领域编排 + 存储层 + 内存库测试)、`tray.rs`(事件 → 领域函数映射)、`error.rs`、`lib.rs`(`setup_desktop` 顺序注释)的注释密度与写法。
8. 同步前端:命令 → `src/lib/api/<domain>.ts`(经 `api/index.ts` 汇出);事件 → `src/lib/events.ts`;结构体 → `src/types/`(见 `../guides/ipc-contract.md`)。

## 质量检查

- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过(在 `src-tauri/` 下)。
- [ ] 命令返回 `Result<T, AppError>` 或不可失败的 `T`;没有 `Result<_, String>`。
- [ ] 没有 `unwrap` / `expect` / `panic!` / `println!` / `block_on`(setup 外)/ 空 `async fn`。
- [ ] 新命令已注册到 `generate_handler!`,前端封装同步。
- [ ] IPC 结构体带 `#[serde(rename_all = "camelCase")]`;带字段的 tagged 枚举还要 `rename_all_fields = "camelCase"`。
- [ ] 落盘一律 `app_local_data_dir()` / `$APPLOCALDATA`,没有 `app_data_dir()` / `$APPDATA`。
- [ ] 没有 `sqlx::query!` 系宏;已提交的迁移文件未被改动。
- [ ] 事件名是 `pub const`,格式 `domain://action`,前端常量同步;`emit` 失败只 `log::warn!`。
- [ ] 新 `.rs` 文件有 `//!`,每个 `pub` 项有 `///`,全部中文。
- [ ] 新依赖 / 配置 / 权限有中文注释,权限最小。

## 关键决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 目录模块风格 | `xxx.rs + xxx/`,不用 `mod.rs` | 本仓库按 Rust 2018 风格起步,避免编辑器里多个同名 `mod.rs` 标签无法区分 |
| 命令返回类型 | `Result<T, AppError>` | 官方文档称 `Result<T, String>` 不 idiomatic;统一错误类型可保留类型信息、集中管理文案 |
| 错误序列化 | 中文字符串 | 前端目前只展示文案,字符串足够;结构化对象留作升级路径 |
| 命令 async | 有 await 才 async | 空 `async fn` 只增加运行时开销并掩盖真实语义;可用 clippy `unused_async` 机制化禁止 |
| 状态持有 | `app.manage` + 参数注入 | 官方文档推荐方式;避免全局 `APP_HANDLE` 带来的初始化时序与测试隔离问题 |
| 事件名 | `domain://action` 常量 | 按领域分组便于检索;常量避免字面量散落导致前后端不一致 |
| 日志 | `log` 门面 + `tauri-plugin-log` | 与 Tauri 生态集成最直接;不再引入 tracing / flexi_logger 等第二套体系 |
| TS 类型生成 | 不用 ts-rs / specta,手写镜像 | 当前 IPC 面很小,手写成本低于引入代码生成链 |
| `[profile.release]` / `[lints]` | 暂不配置 | abort + strip(体积小)与 unwind + 保留符号(便于崩溃分析)取向相反,需要时按发布需求另开任务 |
| edition | 2024,MSRV 1.85 | 本仓库已定 |
| 窗口显示 / 隐藏 | 由 Rust 控制(快捷键 / 托盘 / 失焦 / `hide_launcher` 命令),前端不给 `allow-hide` | 隐藏要顺带 `set_ignore_cursor_events(true)` 并 emit 事件,前端直调 `hide()` 会绕过这两步留下透明挡点区 |
| 无 payload 事件 | emit `()`,前端类型 `null` | 空结构体没有信息增量,只多一个名字要维护 |
| 唤出快捷键 | 单一常量 `launcher::DEFAULT_TOGGLE_SHORTCUT`,经 `get_toggle_shortcut` 命令下发前端渲染键帽 | 预留可配置:到时只改命令实现读设置;字面量不在代码 / 文档各处散落 |
| 托盘右键 | 只弹菜单,不动面板 | 用户右键可能只是想退出,面板不应因此消失;失焦回调已因光标在托盘上豁免 |
| 平台钩子 cfg 写法 | `#[cfg(windows)]` 短写,不写 `target_os = "windows"` | 两者等价,短写更易读;非 Windows 不编译平台文件也不提供 stub |
| 落盘目录 | 全部 `app_local_data_dir()`(`%LOCALAPPDATA%\<identifier>\`),禁用 `app_data_dir()`(Roaming) | WebView2 / 日志本来就在 LOCALAPPDATA;用户删一个目录即可完整清理;剪贴板数据不该漫游 |
| 持久化 | sqlx 0.8 + SQLite,运行时 `sqlx::query`,`migrate!` 内嵌迁移 | 用户指定 sqlx;不用 `query!` 宏避免编译期依赖 DATABASE_URL;详见 `persistence.md` |
| 剪贴板读写 / 监听 | 读写用 `arboard`(跨平台);监听与粘贴模拟按平台自写(`clipboard/windows.rs`) | arboard 三平台一致且不引入第二份 `windows` crate;监听是它不提供的能力,各平台几十行薄实现 |
| 事件由谁 emit | `clipboard://changed` 只由领域层 `record()` 发,命令层不发 | 删除 / 收藏的发起者就是前端自己,命令返回 Ok 它就知道结果;再 emit 会造成本地已改 + 重拉双重刷新 |
| 损坏库 | open 失败直接让 setup 失败,应用报错退出,不自动改名 / 重建 | 用户决定:静默丢历史比启动失败更糟,让用户看到错误自行处理 |
