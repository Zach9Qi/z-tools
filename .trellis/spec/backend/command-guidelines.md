# 命令规范(#[tauri::command])

> 参考实现:`src-tauri/src/commands/greet.rs`。命令是前端唯一能调用的 Rust 入口,也是**信任边界**。

---

## 1. 签名

```rust
/// 向指定名字问好。
///
/// `name` 为用户输入,去除首尾空白后为空则返回参数错误,由前端直接展示文案。
#[tauri::command]
pub fn greet(name: &str) -> Result<String, AppError> { … }
```

- 必须 `pub`(在独立模块里定义命令的官方要求),命令名不受模块作用域影响,全局唯一。
- **可失败就返回 `Result<T, AppError>`**;不写 `Result<T, String>`(官方文档称其「不 idiomatic」;与之伴生的满地 `map_err(|e| e.to_string())` 会丢掉类型信息且文案不统一)。不可能失败的命令直接返回 `T`。
- **没有 `.await` 就写同步 `fn`**,不要为了「看起来统一」全写 `async fn`(可用 `clippy::unused_async = deny` 机制化禁止空 async)。需要 IO / 网络 / 长耗时时才 `async fn`。
- async 命令不能用借用参数(`&str`、`State<'_, T>` 在 async 签名里受 tauri#2533 限制):改用 `String`,或把 `State` 换成 `AppHandle` 后 `app.state::<T>()`,或让返回类型为 `Result` 以绕过。
- 参数按需注入:`State<'_, T>`(托管状态)、`AppHandle`(需要 emit / 取路径 / 开窗口)、`WebviewWindow`(只对当前窗口操作)。不在命令里用全局 static 拿 `AppHandle`。

## 2. 参数与序列化

- 前端传 camelCase 对象 key,Tauri 自动映射到 Rust snake_case 形参;**不要**用 `#[tauri::command(rename_all = "snake_case")]`(官方默认即 camelCase)。
- IPC 结构体统一 `rename_all = "camelCase"`;**入参**结构体额外加 `default`,让老前端少传字段时不报错(用于向前兼容);出参不加。

```rust
/// 前端传入的设置补丁(入参)
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsPatch { … }

/// 返回给前端的完整设置(出参)
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings { … }
```
- `Option<T>` 出参默认序列化为 `null`;只有在字段确实想「不存在」而非 `null` 时才 `skip_serializing_if = "Option::is_none"`。
- 大整数(`u64` 时间戳、文件大小)超过 2^53 会在 JS 侧丢精度,序列化为字符串或改用 `f64` / `u32` 并注释原因。
- 返回大块二进制(文件内容、图片)用 `tauri::ipc::Response` 而不是 `Vec<u8>` JSON(官方文档「Returning Array Buffers」)。
- 多个可选参数收成一个 `options: XxxOptions` 结构体,与前端 `{ options }` 对应。

## 3. 命令体

顺序固定:**校验 → 取状态 → 调领域层 → 整形 → emit**。

```rust
use tauri::{Emitter, Manager}; // emit / state 分别来自这两个 trait,不引入会编译失败

#[tauri::command]
pub async fn update_settings(
    app: tauri::AppHandle,
    patch: SettingsPatch,
) -> Result<Settings, AppError> {
    // 1. 校验:前端是不可信输入源,边界上拒绝非法值
    patch.validate()?;
    // 2. 取状态
    let store = app.state::<SettingsStore>();
    // 3. 领域层做真正的合并与持久化
    let settings = store.update(patch).await?;
    // 4. 成功后通知前端;emit 失败只记日志,不让命令因此失败
    if let Err(e) = app.emit(SETTINGS_UPDATED, &settings) {
        log::warn!("发送 {SETTINGS_UPDATED} 事件失败: {e}");
    }
    Ok(settings)
}
```

- 校验放在命令层最前面,失败返回 `AppError::InvalidInput("中文原因")`;用 `trim()` 处理字符串输入(`greet.rs`)。
- 命令体不写业务规则(见 `directory-structure.md` 三段式);经验值超过 ~30 行就该考虑拆。
- 不在命令里 `block_on`(仅允许 setup 阶段使用);不在 async 命令里 `std::thread::sleep` / 忙等。
- CPU 密集或阻塞 IO 用 `tauri::async_runtime::spawn_blocking`,不要占住异步线程。
- 不 `unwrap()` / `expect()`;`?` 配合 `AppError` 的 `#[from]`。

## 4. 注册

- 所有命令在 `lib.rs` 的 `generate_handler![commands::greet::greet, …]` 集中注册,一行一个,按领域分组。
- 桌面 / 移动专属命令用 `#[cfg(desktop)]` 标在命令函数上,并在注册处同样 cfg(规模小时直接写在 `lib.rs`;cfg 分支多了再把 handler 列表抽成函数)。

## 5. 测试

- 命令文件底部 `#[cfg(test)] mod tests`,测「校验被拒 → 正确变体 + 中文文案」和「正常路径」两类(`greet.rs` 的 `empty_name_is_rejected` / `greets_trimmed_name`)。
- 需要 `AppHandle` / `State` 的命令不做单测;把逻辑下沉到领域层让其可测(测试都在纯逻辑层)。
- async 领域逻辑用 `#[tokio::test]`(需 `[dev-dependencies] tokio = { features = ["macros", "rt"] }`,当前未加,首次需要时添加并注释)。

## 6. 禁止

- `Result<T, String>`、`map_err(|e| e.to_string())`。
- 无 `.await` 的 `async fn`。
- 全局 `static APP_HANDLE`。
- `rename_all = "snake_case"`。
- 命令里 `unwrap` / `expect` / `panic!` / `block_on` / `std::thread::sleep`。
- 把 `println!` 当日志(见 `quality-guidelines.md`)。
