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
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
```

形态固定为:`thiserror` 枚举 + `#[from]` 转换 + 手写 `Serialize` 输出 `to_string()`。官方文档把这种写法列为 `Result<T, String>` 的 idiomatic 替代。

## 2. 规则

- **文案面向用户,中文**。`#[error("…")]` 的内容就是前端看到的字符串,要写成完整可展示的句子;前缀「参数错误 / 输入输出错误 / 系统错误」表明类别。
- **每个变体上方一行 `///`** 说明何时产生、谁负责处理。
- 外部错误一律 `#[from]`,让 `?` 直接工作;不要在调用点 `map_err(|e| e.to_string())`(丢掉类型信息且文案不统一)。
- 新增变体的判据:前端需要看到**不同的文案类别**,或者有新的外部错误类型要 `#[from]`。不要为每个函数造一个变体。
- 命令层校验失败统一 `AppError::InvalidInput("具体原因".into())`,原因不带「参数错误」前缀(枚举文案已带)。
- 不要在文案里再拼动作(「保存设置失败: …」);动作上下文前端组件知道,重复拼接会出现「保存失败: 保存设置失败: …」。
- 序列化契约有测试锁定:`greet.rs` 的 `assert_eq!(err.to_string(), "参数错误: 名字不能为空")`。新增变体时补一条同样的断言。

## 3. 何时引入 anyhow

当前依赖里没有 `anyhow`,领域层很薄时不需要。当领域层出现多层调用、需要给错误加上下文时,采用以下模式:

- 领域层内部用 `anyhow::Result` + `.context("中文上下文")` 传播;
- 边界(命令层)通过 `AppError::Other(#[from] anyhow::Error)` 变体统一转换,文案 `#[error("{0:#}")]` 用 `{:#}` 展开上下文链。

引入时在 `Cargo.toml` 注释写明「仅领域层内部使用,不得出现在命令签名里」。

## 4. 何时改成结构化错误

前端现在只展示文案,字符串足够。以下情况才升级为 `{ kind, message }` 或 `{ code, detail }` 之类的结构体:

- 前端需要按错误类别分支(如「参数错误」高亮输入框、「系统错误」弹重试)。
- 需要 i18n:前端按 `code` 查文案。

升级方式是改 `impl Serialize` 输出结构体(官方文档「ErrorKind」示例),`AppError` 枚举本身不变;同时更新 `src/lib/api.ts` 的错误处理与 `src/types/` 的镜像类型,并加序列化测试锁定 wire 格式。

## 5. panic 与 unwrap

- 命令、事件回调、领域层:不 `unwrap()` / `expect()`;用 `?` 或 `if let Err(e) … log::warn!`。
- 允许 `expect("中文原因")` 的地方只有 `lib.rs` 的 `run()` 末尾(`.expect("启动应用失败")`)和真正不可能失败的静态初始化。
- `setup` 闭包里的 panic 会跨 FFI 边界直接 abort(tao 从 ObjC / C 回调同步调用),所以 setup 内一律返回 `Err(Box<dyn Error>)` 而不是 panic。
- Mutex 中毒(`lock()` 返回 `Err`)在本仓库视为 bug,可用 `unwrap_or_else(|e| e.into_inner())` 恢复并 `log::error!`,不要静默 `unwrap`。

## 6. 日志与错误的关系

- 返回给前端的错误**不必**再 `log::error!` 一遍(前端会展示);只有被吞掉、不再向上传播的错误才记日志(如 emit 失败)。
- 日志文案中文,级别:被吞掉但不影响功能 → `warn`;影响功能 → `error`。

## 7. 禁止

- `Result<T, String>`、`Box<dyn Error>` 作为命令返回类型。
- `map_err(|e| e.to_string())`、`format!("… {e}")` 手拼错误字符串。
- 英文错误文案。
- 在文案里加动作前缀。
- 命令 / 回调里 `unwrap` / `expect` / `panic!` / `unimplemented!`。
