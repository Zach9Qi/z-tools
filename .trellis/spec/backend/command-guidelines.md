# 命令规范(#[tauri::command])

> 参考实现:`src-tauri/src/commands/launcher.rs`(不可失败命令的样板:`hide_launcher` 返回 `()`、`get_toggle_shortcut` 返回 `&'static str`,两者都只转发到 `crate::launcher`);`src-tauri/src/commands/clipboard.rs`(可失败 async 命令的样板:6 个 `pub async fn … -> Result<T, AppError>`,`State<'_, ClipboardStore>` 注入,`limit` 校验与 `query` trim 在命令层,其余全部转发 `crate::clipboard`)。命令是前端唯一能调用的 Rust 入口,也是**信任边界**。

---

## 1. 签名

```rust
// 现实样板(commands/clipboard.rs):可失败 async 命令。State 注入 + Result<T, AppError>;校验在最前,然后转发
/// 按 kind × favoriteOnly × query 叠加筛选并游标分页,`ORDER BY copied_at DESC, id DESC`。
///
/// `limit` 不在 1..=200 内返回参数错误;关键字先 trim,只有空白时视为不过滤。
#[tauri::command]
pub async fn list_clipboard_items(
    store: State<'_, ClipboardStore>,
    mut query: ListQuery,
) -> Result<Vec<ClipboardItem>, AppError> {
    validate_limit(query.limit)?;
    query.query = query.query.trim().to_owned();
    store.list(&query).await
}

#[tauri::command]
pub async fn set_clipboard_item_favorite(
    store: State<'_, ClipboardStore>,
    id: i64,
    favorite: bool,
) -> Result<(), AppError> {
    store.set_favorite(id, favorite).await
}

// 现实样板(commands/launcher.rs):不可失败就直接返回 T;窗口 API 的失败已在领域层记日志,前端无需也无法处理
#[tauri::command]
pub fn hide_launcher<R: Runtime>(app: AppHandle<R>) { crate::launcher::hide(&app); }

#[tauri::command]
pub fn get_toggle_shortcut() -> &'static str { crate::launcher::DEFAULT_TOGGLE_SHORTCUT }
```

- 必须 `pub`(在独立模块里定义命令的官方要求),命令名不受模块作用域影响,全局唯一。
- **可失败就返回 `Result<T, AppError>`**;不写 `Result<T, String>`(官方文档称其「不 idiomatic」;与之伴生的满地 `map_err(|e| e.to_string())` 会丢掉类型信息且文案不统一)。不可能失败的命令直接返回 `T`。
- **没有 `.await` 就写同步 `fn`**,不要为了「看起来统一」全写 `async fn`(可用 `clippy::unused_async = deny` 机制化禁止空 async)。需要 IO / 网络 / 长耗时时才 `async fn`。
- async 命令不能用借用参数(`&str` 在 async 签名里受 tauri#2533 限制):改用 `String`。`State<'_, T>` 在 async 命令里**可以用**,前提是返回类型为 `Result`(`commands/clipboard.rs` 全部如此);返回裸 `T` 的 async 命令才需要换成 `AppHandle` 后 `app.state::<T>()`。
- 需要平台差异的命令(`paste_clipboard_item`),差异放在领域函数的 `#[cfg]` 两份实现上,命令签名与 `generate_handler!` 不带 cfg;不支持的平台返回 `AppError::Unsupported`,不静默 no-op。
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

- 校验放在命令层最前面,失败返回 `AppError::InvalidInput("中文原因")`;字符串输入先 `trim()` 再判空。现例:`list_clipboard_items` 的 `validate_limit`(范围常量 `LIMIT_RANGE: RangeInclusive<u32> = 1..=200`,文案「每页条数必须在 1..=200 之间」)与 `query.query.trim()`;存储层信任 `limit` 已校验。
- **命令层不一定要 emit**:前端自己发起的变更(删除 / 收藏)它已知结果,命令返回 `Ok` 即可;只有前端无法自知的变化(监听器录入)才在领域层 emit(见 `state-events-async.md` §2)。上面示意代码里的第 4 步适用于「多窗口 / 多消费者」场景。
- 窗口操作类命令(`hide_launcher`)不返回 `Result`:领域层 `launcher.rs` 已把所有 Tauri 窗口 API 的失败 `log::warn!` 并继续,前端拿到错误也做不了什么(窗口没隐藏只是留在屏幕上);不为了〈看起来统一〉包一层 `Ok(())`。
- 命令体不写业务规则(见 `directory-structure.md` 三段式);经验值超过 ~30 行就该考虑拆。
- 不在命令里 `block_on`(仅允许 setup 阶段使用);不在 async 命令里 `std::thread::sleep` / 忙等。
- CPU 密集或阻塞 IO 用 `tauri::async_runtime::spawn_blocking`,不要占住异步线程。
- 不 `unwrap()` / `expect()`;`?` 配合 `AppError` 的 `#[from]`。

## 4. 注册

- 所有命令在 `lib.rs` 的 `generate_handler![commands::launcher::hide_launcher, commands::launcher::get_toggle_shortcut, …]` 集中注册,一行一个,按领域分组。
- 桌面 / 移动专属命令用 `#[cfg(desktop)]` 标在命令函数上,并在注册处同样 cfg(规模小时直接写在 `lib.rs`;cfg 分支多了再把 handler 列表抽成函数)。

## 5. 测试

- 带校验的命令在文件底部 `#[cfg(test)] mod tests`,测「校验被拒 → 正确变体 + 中文文案」和「正常路径」两类(现例 `commands/clipboard.rs` 的 `limit_out_of_range_is_rejected_with_chinese_message` / `limit_in_range_is_accepted`,校验抽成私有 `fn validate_limit` 才能不带 `State` 测);`AppError` 的序列化契约由 `error.rs` 的 `*_message_has_category_prefix` 锁定。
- 需要 `AppHandle` / `State` / 窗口的命令不做单测;把逻辑下沉到领域层让其可测。现例:`launcher.rs` 把定位算法抽成纯函数 `anchor_position(work, window_size)`,三条测试(居中、副屏偏移、窄工作区贴左缘)不碰任何 Tauri 类型;真正调窗口 API 的 `position_anchored` 不测。
- async 领域逻辑用 `#[tokio::test]`(`[dev-dependencies] tokio = { features = ["macros", "rt"] }` 已加);数据库逻辑用 `sqlite::memory:` 内存库,见 `persistence.md` §6。

## 6. 禁止

- `Result<T, String>`、`map_err(|e| e.to_string())`。
- 无 `.await` 的 `async fn`。
- 全局 `static APP_HANDLE`。
- `rename_all = "snake_case"`。
- 命令里 `unwrap` / `expect` / `panic!` / `block_on` / `std::thread::sleep`。
- 把 `println!` 当日志(见 `quality-guidelines.md`)。
