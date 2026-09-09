# 错误处理

> 参考实现:`src-tauri/src/error.rs`。全仓库只有一个面向前端的错误类型 `AppError`。

---

## 1. 形态

```rust
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 前端传入的参数不合法(前端是不可信输入源,校验放在命令层边界)
    #[error("参数错误: {0}")]
    InvalidInput(String),

    /// 文件系统等输入输出错误
    #[error("输入输出错误: {0}")]
    Io(#[from] std::io::Error),

    /// Tauri 运行时错误(窗口、事件等)
    #[error("系统错误: {0}")]
    Tauri(#[from] tauri::Error),

    /// SQLite(sqlx)读写失败:库损坏、磁盘满、迁移失败等,由存储层 `?` 透传到命令层
    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    /// 剪贴板读写失败(arboard 报错 / 图片编解码失败 / Win32 监听窗口创建失败),内容为底层错误的 `to_string()`
    #[error("剪贴板错误: {0}")]
    Clipboard(String),

    /// 当前操作系统没有实现该能力(如非 Windows 上的剪贴板粘贴),前端直接展示,不重试
    #[error("当前平台暂不支持: {0}")]
    Unsupported(String),
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
```

形态固定为:`thiserror` 枚举 + `#[from]` 转换 + 手写 `Serialize` 输出 `to_string()`。官方文档把这种写法列为 `Result<T, String>` 的 idiomatic 替代。

三个后加变体的归类依据(新增变体时照此判断):

| 变体 | 为什么是新变体 | 谁产生 |
|---|---|---|
| `Database(#[from] sqlx::Error)` | 新的外部错误类型需要 `?` 直接工作 | `clipboard/store.rs` 所有 `.await?`;`kind_of` 读到非法 `kind` 时手造 `sqlx::Error::Decode` |
| `Clipboard(String)` | 前端需要「剪贴板错误」这个文案类别;底层来源多种(arboard / image / Win32)且都不值得各 `#[from]` 一个变体,统一 `to_string()` | `backend.rs::clipboard_error`、`clipboard/windows.rs` 的窗口创建 / `SendInput` |
| `Unsupported(String)` | 「平台未实现」不是参数错也不是系统错,前端不应提示重试 | `clipboard::paste` 的 `#[cfg(not(windows))]` 版本,内容是能力名(「剪贴板粘贴」) |

反例:不为「记录不存在」造 `NotFound` 变体——它属于「前端传了不存在的 id」,用 `InvalidInput("记录不存在")`(`store.rs` 常量 `NOT_FOUND`)。

## 2. 规则

- **文案面向用户,中文**。`#[error("…")]` 的内容就是前端看到的字符串,要写成完整可展示的句子;前缀「参数错误 / 输入输出错误 / 系统错误」表明类别。
- **每个变体上方一行 `///`** 说明何时产生、谁负责处理。
- 外部错误一律 `#[from]`,让 `?` 直接工作;不要在调用点 `map_err(|e| e.to_string())`(丢掉类型信息且文案不统一)。
- 新增变体的判据:前端需要看到**不同的文案类别**,或者有新的外部错误类型要 `#[from]`。不要为每个函数造一个变体。
- 命令层校验失败统一 `AppError::InvalidInput("具体原因".into())`,原因不带「参数错误」前缀(枚举文案已带)。
- 不要在文案里再拼动作(「保存设置失败: …」);动作上下文前端组件知道,重复拼接会出现「保存失败: 保存设置失败: …」。
- 序列化契约有测试锁定:`error.rs` 内联测试 `invalid_input_message_has_category_prefix` 同时断言 `to_string()` 文案与 `serde_json::to_string` 的 wire 格式(`"\"参数错误: 名字不能为空\""`);后加的三个变体各有一条:`clipboard_message_has_category_prefix`(含 wire 格式)、`unsupported_message_has_category_prefix`、`database_message_has_category_prefix`(用 `sqlx::Error::RowNotFound.into()` 验 `#[from]` + `starts_with("数据库错误: ")`)。新增变体时在同一个 `mod tests` 补一条同样的断言。
- `AppError` 在 `lib.rs` 以 `pub mod error` 导出,作为 crate 的错误契约(供命令与集成测试使用,也是后来 `pub mod clipboard` 的先例)。首批可失败命令是 `commands/clipboard.rs`,全部 `Result<T, AppError>`;启动器的窗口命令仍不可失败(领域层吞错误记日志)。不要另造领域专属错误类型。
- 存储层读到「不可能」的数据(如 `kind` 列不在 CHECK 三值内)时,手造 `AppError::Database(sqlx::Error::Decode("中文原因".into()))`,不用 `InvalidInput`(不是前端的错)、不 panic。可容忍的损坏(`files` 列 JSON 非法)则降级 + `log::warn!`,不让整页列表失败。

## 3. 何时引入 anyhow

当前依赖里没有 `anyhow`,领域层很薄时不需要。当领域层出现多层调用、需要给错误加上下文时,采用以下模式:

- 领域层内部用 `anyhow::Result` + `.context("中文上下文")` 传播;
- 边界(命令层)通过 `AppError::Other(#[from] anyhow::Error)` 变体统一转换,文案 `#[error("{0:#}")]` 用 `{:#}` 展开上下文链。

引入时在 `Cargo.toml` 注释写明「仅领域层内部使用,不得出现在命令签名里」。

## 4. 何时改成结构化错误

前端现在只展示文案,字符串足够。以下情况才升级为 `{ kind, message }` 或 `{ code, detail }` 之类的结构体:

- 前端需要按错误类别分支(如「参数错误」高亮输入框、「系统错误」弹重试)。
- 需要 i18n:前端按 `code` 查文案。

升级方式是改 `impl Serialize` 输出结构体(官方文档「ErrorKind」示例),`AppError` 枚举本身不变;同时更新 `src/lib/api/**` 的错误处理与 `src/types/` 的镜像类型,并加序列化测试锁定 wire 格式。

## 5. panic 与 unwrap

- 命令、事件回调、领域层:不 `unwrap()` / `expect()`;用 `?` 或 `if let Err(e) … log::warn!`。
- 允许 `expect("中文原因")` 的地方只有 `lib.rs` 的 `run()` 末尾(`.expect("启动应用失败")`)和真正不可能失败的静态初始化。
- `setup` 闭包里的 panic 会跨 FFI 边界直接 abort(tao 从 ObjC / C 回调同步调用),所以 setup 内一律返回 `Err(Box<dyn Error>)` 而不是 panic。
- Mutex 中毒(`lock()` 返回 `Err`)在本仓库视为 bug,可用 `unwrap_or_else(|e| e.into_inner())` 恢复并 `log::error!`,不要静默 `unwrap`。

## 6. 日志与错误的关系

- 返回给前端的错误**不必**再 `log::error!` 一遍(前端会展示);只有被吞掉、不再向上传播的错误才记日志(如 emit 失败、监听器 `record()` 内的失败、删图片文件失败)。
- 命令返回 `Ok` 但部分步骤降级时(`paste()` 激活原窗口失败只复制不粘贴、`SendInput` 失败),降级原因记 `debug` / `warn`,不往上报错:对用户而言主目的(内容已在剪贴板)已达成。
- 日志文案中文,级别:被吞掉但不影响功能 → `warn`;影响功能 → `error`。

## 7. 禁止

- `Result<T, String>`、`Box<dyn Error>` 作为命令返回类型。
- `map_err(|e| e.to_string())`、`format!("… {e}")` 手拼错误字符串。
- 英文错误文案。
- 在文案里加动作前缀。
- 命令 / 回调里 `unwrap` / `expect` / `panic!` / `unimplemented!`。
