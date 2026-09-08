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
- 测试函数名 snake_case 描述行为:`empty_name_is_rejected`、`greets_trimmed_name`;`expect` 消息用中文说明期望(`greet.rs`)。
- 必测:领域层纯逻辑;`AppError` 每个变体的 `to_string()` 文案(锁定前端契约);输入校验的拒绝路径。
- 不测:需要 `AppHandle` / `State` / 窗口的命令;把逻辑下沉后测领域层。
- 异步逻辑用 `#[tokio::test]`,首次引入时在 `[dev-dependencies]` 加 `tokio = { features = ["macros", "rt"] }` 并注释。
- 涉及文件系统的测试用 `std::env::temp_dir()` + 唯一子目录,测试结束清理;不写到仓库目录。

## 3. 注释与文档

- 每个 `.rs` 文件开头 `//!` 模块文档:一句话职责 + 边界(什么不放这里)。`lib.rs`、`commands.rs`、`error.rs`、`greet.rs` 都是样板。
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

## 6. 提交前自检

- [ ] `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` 通过。
- [ ] 新命令已在 `generate_handler!` 注册,前端 `api.ts` 有对应封装。
- [ ] 新 `AppError` 变体有 `///`、中文文案、`to_string()` 测试。
- [ ] 新依赖(非通用)/ 配置项有中文注释。
- [ ] 没有 `unwrap` / `expect` / `println!` / `Result<_, String>` / 空 `async`。
- [ ] 新 `.rs` 文件有 `//!` 模块文档。
