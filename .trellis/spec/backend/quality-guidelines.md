# 后端质量规范

> 门禁命令与 `.github/workflows/ci.yml` 的 `rust` job 一致(Windows + Ubuntu 双平台)。

---

## 1. 门禁命令(在 `src-tauri/` 下)

```bash
cargo fmt --check                              # 默认 rustfmt 配置,无 rustfmt.toml
cargo clippy --all-targets -- -D warnings      # 警告即错误;--all-targets 覆盖测试代码
cargo test
```

- `fmt --check` + `clippy -D warnings` 均进 CI,本地与 CI 执行同一组命令。
- `#[allow(clippy::xxx)]` 必须带一行中文原因,且作用域最小(标在项上,不标在模块上)。
- 本地跑 clippy 前若 `../dist` 不存在,`mkdir -p ../dist`。

## 2. 测试

- 只用**内联** `#[cfg(test)] mod tests { use super::*; … }`,不建 `tests/` 目录。
- 测试函数名 snake_case 描述行为:`centers_horizontally_and_anchors_top_at_quarter`(`launcher.rs`)、`invalid_input_message_has_category_prefix`(`error.rs`);`expect` 消息用中文说明期望(`error.rs` 的 `"AppError 应能序列化为字符串"`)。
- 必测:领域层纯逻辑;`AppError` 每个变体的 `to_string()` 文案(锁定前端契约);输入校验的拒绝路径。
- 不测:需要 `AppHandle` / `State` / 窗口的命令;把逻辑下沉后测领域层。
- 异步逻辑用 `#[tokio::test]`(`[dev-dependencies] tokio = { features = ["macros", "rt"] }` 已加并注释);数据库逻辑用 `sqlite::memory:` 内存库,连接数固定 1 且不回收(见 `persistence.md` §6)。
- 涉及文件系统的测试用 `std::env::temp_dir()` + 唯一子目录(现例 `store.rs::get_captured_round_trips_text_files_and_image` 用 `z-tools-clipboard-store-<pid>`),测试结束清理;不写到仓库目录。
- 不测需要真实系统剪贴板 / 消息循环的代码(`backend::read_snapshot` / `write`、`clipboard/windows.rs`);把阈值判定、哈希、缩略图尺寸等抽成纯函数测(`backend.rs` 的 `text_snapshot` / `image_too_large` / `thumb_size` / `pixel_hash` / `image_snapshot`)。

## 3. 注释与文档

- 每个 `.rs` 文件开头 `//!` 模块文档:一句话职责 + 边界(什么不放这里)。`lib.rs`、`commands.rs`、`error.rs`、`launcher.rs`、`tray.rs` 都是样板。
- 每个 `pub` 项 `///` 文档注释:做什么、参数含义、何时返回哪个错误。
- 行内 `//` 注释解释**隐藏约束**:平台时序、workaround、为什么不能删(`main.rs` 的 `windows_subsystem`、`Cargo.toml` 的 `_lib`)。不写复述代码的注释。
- 全部中文;标识符英文。
- 经验参考值(非硬标准):函数超过 ~50 行或嵌套超过 3 层就考虑拆。

## 4. 代码卫生

- `use` 分三组:std / 第三方 / `crate::`,组间空行(rustfmt 默认不重排组,自己维护)。
- 不 `use xxx::*` 通配导入(可用 clippy `wildcard_imports = deny` 机制化);`commands/` 的 `pub mod` 显式列出。
- serde / thiserror 用全路径写法 `serde::Serialize`、`thiserror::Error`(与 `error.rs` 一致),不单独 `use serde::Serialize;`;建议派生顺序 `Debug, Clone, serde::Serialize, serde::Deserialize`,再接 `#[serde(...)]`。
- 字符串格式化用内联变量 `format!("你好,{name}")`,不用位置参数。
- 常量用 `const`,不用 `static`(除非需要地址稳定或内部可变)。
- `unsafe` 块每处上方一行 `// 安全:…` 中文注释说明为什么前置条件成立(句柄来自哪里、回调签名为何匹配、谁保证生命周期),且块只包住那一次 FFI 调用;样板 `launcher/windows.rs`(`SetWindowSubclass` / `RemoveWindowSubclass` / `DefSubclassProc` / `GetForegroundWindow` / `GetClassNameW` / `IsWindow` / `SetForegroundWindow`)与 `clipboard/windows.rs`(消息窗口一套 + `SendInput`)。没有理由可写的 `unsafe` 就不该存在。原生句柄跨线程 / 存状态时转 `isize`,不让 `HWND` 本身离开平台文件。
- `Cargo.lock` 提交进仓库;改名 crate 后 `cargo update --workspace --offline` 刷新。

## 5. 反模式速查

| 反模式 | 本仓库要求 |
|---|---|
| 命令里连续 `unwrap()` | `?` + `AppError` |
| `Result<T, String>` + `map_err(to_string)` | `Result<T, AppError>` |
| 无 await 的 `async fn` | 同步 `fn` |
| async 里 `std::thread::sleep` / 忙等 | `tokio::time::sleep` |
| async 里 `std::sync::Mutex::lock().unwrap()` 跨 await | `tokio::sync::Mutex` 或缩小锁范围 |
| `println!` 调试 | `log::debug!` |
| `lib.rs` 膨胀到几百行且含业务结构体 | 只放装配 |
| 单命令文件几百行 | 拆领域层 |
| capabilities 通配 | 最小权限 |
| `tokio::spawn` 与 `async_runtime::spawn` 混用 | 只用 `tauri::async_runtime` |
| 全局 `LazyLock<Arc<Mutex>>` 代替托管状态 | `app.manage` |
| setup 里 panic(会跨 FFI 直接 abort) | 返回 `Err` |
| 英文 / 中英混杂错误文案 | 中文 |
| `sqlx::query!` 系编译期宏 | 运行时 `query_as::<_, XxxRow>` / `query_scalar` / `query` |
| 手写 `row.try_get("列名")` 散落各处 | `#[derive(sqlx::FromRow)] struct XxxRow` 一处声明列名,再 `XxxRow -> DTO`(`persistence.md`) |
| 改已提交的迁移文件 | 新增 `000N_xxx.sql` |
| `OFFSET` 分页 | keyset 游标 `(copied_at, id) < (?, ?)` |
| `#[allow(dead_code)]` 盖住非 Windows 下的未使用告警 | `pub mod` + 模块文档写理由 |
| `app_data_dir()` | `app_local_data_dir()` |

## 6. 提交前自检

- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过。
- [ ] 新命令已在 `generate_handler!` 注册,前端 `src/lib/api/<domain>.ts` 有对应封装。
- [ ] 新增依赖后 `cargo tree -i windows` 仍只有一个版本。
- [ ] 新 `AppError` 变体有 `///`、中文文案、`to_string()` 测试。
- [ ] 新依赖(非通用)/ 配置项有中文注释。
- [ ] 没有 `unwrap` / `expect` / `println!` / `Result<_, String>` / 空 `async`。
- [ ] 新 `.rs` 文件有 `//!` 模块文档。
