# 后端模块分层(src-tauri/)

> Rust 侧代码如何组织。新建模块前先看这里。

---

## 当前布局(真实存在)

```text
src-tauri/
├── Cargo.toml              # 非通用依赖上方一行中文注释说明用途与选型
├── tauri.conf.json5        # 每个配置项一行中文注释(config-json5 feature)
├── build.rs                # tauri_build::build(),不放逻辑
├── capabilities/
│   └── default.json        # 主窗口权限,仅 core:default
└── src/
    ├── main.rs             # 只有 windows_subsystem 属性 + tauri_vue_starter_lib::run()
    ├── lib.rs              # 组合根:插件注册、setup、generate_handler!;不放业务逻辑
    ├── error.rs            # 全局统一 AppError
    ├── commands.rs         # 命令层模块根:pub mod <domain>;
    └── commands/
        └── greet.rs        # 一个领域一个文件;内含 #[cfg(test)] mod tests
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
| 平台专属 | `src/<domain>/{windows,macos,linux}.rs`,`#[cfg(target_os = "…")] mod` | 只在某个 OS 存在的实现 | 在通用代码里散落 `#[cfg]` 分支 |

规则:

- 命令层「薄但不空」:允许校验 + 编排 + 整形,业务规则必须在领域层,这样 `#[cfg(test)]` 可以不依赖 `AppHandle` 测到业务。
- 小领域先写成 `commands/<domain>.rs` 一个文件;业务逻辑一出现就拆到 `src/<domain>.rs`,不要在 `commands/` 里长出几百行。
- `lib.rs` 只增长 `plugin(...)`、`manage(...)`、`generate_handler![...]` 三类行;setup 里的初始化逻辑抽到 `fn setup_xxx(app: &AppHandle) -> Result<(), AppError>`(经验值:超过十来行就抽)。桌面端专属装配按 `lib.rs` 注释拆 `#[cfg(desktop)] fn setup_desktop`。
- 横切模块变多后收拢到 `src/core/`(如 `core/{error,paths}`),此前 `error.rs` 直接放 `src/`。

## 新增命令的固定动作

1. 在 `src/commands/<domain>.rs` 写 `pub fn` / `pub async fn`,标 `#[tauri::command]`,文档注释说明参数与失败情形。
2. 若是新领域文件,在 `src/commands.rs` 加 `pub mod <domain>;`。
3. 在 `lib.rs` 的 `generate_handler![...]` 追加 `commands::<domain>::<name>`(漏注册前端 invoke 直接报错,`lib.rs` 注释)。
4. 前端 `src/lib/api.ts` 加同名 camelCase 封装(见 `../guides/ipc-contract.md`)。

## 命名

| 对象 | 规则 | 例子 |
|---|---|---|
| 模块 / 文件 | snake_case,名词 | `commands/greet.rs`、`settings.rs` |
| 命令函数 | snake_case,动词开头;前端函数名是它的 camelCase | `greet`、`get_settings`、`update_settings` |
| 状态结构体 | `XxxState` / `XxxStore`,`app.manage()` 托管 | `SettingsStore` |
| 事件常量 | `SCREAMING_SNAKE`,值 `domain://action` | `pub const SETTINGS_UPDATED: &str = "settings://updated";` |
| 错误变体 | 名词或名词短语 | `AppError::InvalidInput` |

## 禁止

- `main.rs` 里出现除 `run()` 以外的逻辑。
- 在 `lib.rs` 定义业务结构体或命令。
- `commands/` 下的文件互相 `use`(共享逻辑下沉到领域层)。
- 新建 `mod.rs`。
- 平台代码用大段 `#[cfg]` 内联在通用函数里(拆文件)。
